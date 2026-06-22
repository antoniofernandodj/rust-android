# Gradle Build Integration

> **Seção:** Native Library Integration
> **Status:** ✅ preenchido

## Visão geral

O **Gradle** é o sistema de build do Android. Integrar o Rust significa fazer o Gradle (a)
disparar a compilação Rust no momento certo, (b) empacotar os `.so` resultantes no APK/AAB
nas ABIs corretas e (c) tratar isso como **dependência das tasks de build**, para que um
`.so` desatualizado nunca entre no pacote.

Há essencialmente três estratégias, do mais simples ao mais acoplado:

1. **`cargo-ndk` + `jniLibs.srcDirs`** — você (ou um script Make/Gradle) roda `cargo ndk`,
   que cospe os `.so` na pasta certa, e o Gradle só os empacota. Mais simples; ótimo para
   começar. Risco: o passo Rust e o build Gradle ficam frouxamente acoplados (você pode
   esquecer de rodar o Rust).
2. **Plugin `rust-android-gradle`** (Mozilla) — adiciona uma task `cargoBuild` ao Gradle que
   compila o Rust como parte do build, cuidando de targets, NDK e cópia. Mais integrado e
   reprodutível; é o caminho recomendado para projetos sérios.
3. **`externalNativeBuild` + CMake** — quando o Rust é um `staticlib` linkado dentro de um
   `.so` C/C++ maior. Mais complexo; use só nesse cenário híbrido (ver *Shared Object
   Creation → Static library bundling*).

O princípio que une as três: **o `.so` precisa estar atualizado e no lugar certo antes do
empacotamento**, e isso tem que ser garantido pelo grafo de tasks, não pela disciplina do
desenvolvedor.

## Subtópicos

### External native build config

- O bloco `externalNativeBuild` do `build.gradle` integra builds nativos (CMake/ndk-build)
  ao Gradle. Usa-se quando há um `CMakeLists.txt` que, por sua vez, invoca o Cargo ou
  consome um `staticlib` Rust.
- Para projetos Rust puros, costuma ser **desnecessário**: basta gerar os `.so` com
  `cargo-ndk`/plugin e apontar `jniLibs.srcDirs`. Só pague a complexidade do CMake se houver
  código C/C++ no meio.

### CMake build file integration

- Quando se usa CMake, o `CMakeLists.txt` pode chamar o Cargo via `add_custom_command` e
  linkar o `.a` Rust (`staticlib`) em um target nativo C/C++.
- Plugins como **`rust-android-gradle`** (Mozilla) encapsulam o caminho mais comum sem CMake,
  adicionando uma task `cargoBuild` que cuida de targets, NDK e cópia dos `.so`
  automaticamente:

```groovy
plugins { id "org.mozilla.rust-android-gradle.rust-android" }

cargo {
    module  = "../rust"          // pasta do crate
    libname = "core"             // gera libcore.so
    targets = ["arm64", "x86_64"]
    profile = "release"          // ou condicional ao buildType
}
```

- O plugin resolve o NDK a partir do SDK configurado, então você não duplica caminhos de
  toolchain (ver *Build Environment*).

### Cargo manifest inclusion

- O `Cargo.toml` do crate nativo precisa declarar `crate-type = ["cdylib"]` (ver *Shared
  Object Creation*) e ficar em um diretório conhecido pelo Gradle (ex.: `../rust`).
- Mantenha o crate Rust **fora** de `src/main` para separar fontes Rust de fontes Kotlin; o
  Gradle referencia o **output** (o `.so`), não o código-fonte Rust.
- Layout típico de um projeto:

```
meu-app/
├── app/                      # módulo Android (Kotlin/Compose)
│   ├── build.gradle.kts
│   └── src/main/
│       ├── java/             # ou kotlin/
│       └── jniLibs/<abi>/    # .so vão para cá
├── rust/                     # crate(s) Rust
│   ├── Cargo.toml            # crate-type = ["cdylib"]
│   └── src/lib.rs
└── settings.gradle.kts
```

### Build task dependency mapping

- O ponto crítico: a task que gera os `.so` deve rodar **antes** do empacotamento. Amarre-a
  às tasks `merge<Variant>JniLibFolders` / `preBuild`:

```groovy
tasks.whenTaskAdded { task ->
    if (task.name == 'mergeDebugJniLibFolders')   task.dependsOn 'cargoBuildDebug'
    if (task.name == 'mergeReleaseJniLibFolders') task.dependsOn 'cargoBuildRelease'
}
```

- Sem esse mapeamento, o Gradle pode empacotar um `.so` velho (build não-reprodutível) — um
  bug traiçoeiro porque "funciona" até alguém limpar o `target/` e perceber que o `.so`
  nunca era regerado.
- O mesmo princípio vale se você gera **bindings UniFFI** (ver *Automated Bindings*): a task
  de `uniffi-bindgen` deve rodar antes do `compileKotlin`, e os `.kt` gerados entram num
  `sourceSet`.

### Library search path definitions

- O Gradle procura `.so` em `android.sourceSets.main.jniLibs.srcDirs`. Aponte-o para onde
  o `cargo-ndk` despeja os artefatos:

```groovy
android {
    sourceSets.main.jniLibs.srcDirs += ['src/main/jniLibs']
}
```

- A estrutura deve ser `jniLibs/<abi>/lib<nome>.so` (ex.: `jniLibs/arm64-v8a/libcore.so`).
  O nome da pasta é a **ABI Android**, não o triple Rust (ver *Cross Compilation Targets →
  Armadilhas comuns*).
- Opção de packaging: `android.packaging.jniLibs.useLegacyPackaging = false` (padrão moderno)
  mantém os `.so` comprimidos e alinhados dentro do APK em vez de extraí-los no install —
  casa com `android:extractNativeLibs="false"` (ver *Application Lifecycle → Manifest
  configuration files*).

### ABI filter specification

- `ndk.abiFilters` limita quais ABIs entram no APK — evite empacotar ABIs que você não
  compila (causaria `UnsatisfiedLinkError` em runtime ou peso inútil):

```groovy
android {
    defaultConfig {
        ndk { abiFilters 'arm64-v8a', 'x86_64' }
    }
    // ou: splits.abi para gerar um APK por ABI (modelo legado),
    // mas hoje prefira o AAB (ver Package Management) que faz o split por você
}
```

- **Garanta que os filtros do Gradle e os targets compilados pelo `cargo-ndk` coincidam.** O
  cenário mais comum de crash em emulador: app empacotado só com `arm64-v8a` rodando num AVD
  `x86_64` → `UnsatisfiedLinkError` ao `loadLibrary`. Inclua `x86_64` enquanto desenvolve.
- Em release com AAB, você normalmente empacota **todas as ABIs de produção** e deixa o
  Google fazer o split por device; os `abiFilters` servem mais para o ciclo de dev e para
  excluir ABIs que você nunca dá suporte.

## Armadilhas comuns

- **`.so` desatualizado no pacote** — falta de `dependsOn` no grafo de tasks. Sintoma:
  mudanças no Rust não aparecem até um clean. Solução: mapeamento de dependência (acima).
- **Mismatch ABI Gradle × cargo-ndk** — compila ARM mas empacota só x86, ou vice-versa →
  `UnsatisfiedLinkError`. Mantenha uma fonte única da lista de ABIs.
- **`minSdkVersion` × API do NDK** — `.so` linkado com API maior que o `minSdk` do Gradle
  quebra em devices antigos (ver *Cross Compilation Targets → Target triple management*).
- **Build do Rust não falha o build do Gradle** — se o passo Rust é um script externo que o
  Gradle ignora o código de saída, um erro de compilação Rust pode passar despercebido.
  Prefira o plugin, que propaga a falha.

## Assuntos correlatos

- **`cargo-ndk` e o ambiente de build:** *Build Environment → Cargo mobile integration*.
- **O que entra no `.so` e seu tamanho:** *Shared Object Creation*.
- **AAB, splits por ABI e download otimizado:** *Package Management*.
- **Geração de bindings no build:** *Automated Bindings → Automated glue code generation*.

## Referências

- rust-android-gradle: https://github.com/mozilla/rust-android-gradle
- Configure the NDK (abiFilters): https://developer.android.com/studio/projects/configure-agp-ndk
- cargo-ndk: https://github.com/bbqsrc/cargo-ndk
- Add C/C++ code (externalNativeBuild): https://developer.android.com/studio/projects/add-native-code
