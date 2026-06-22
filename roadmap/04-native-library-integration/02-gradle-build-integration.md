# Gradle Build Integration

> **Seção:** Native Library Integration
> **Status:** ✅ preenchido

## Visão geral

O **Gradle** é o sistema de build do Android. Integrar o Rust significa fazer o Gradle (a)
disparar a compilação Rust no momento certo, (b) empacotar os `.so` resultantes no APK/AAB
nas ABIs corretas e (c) tratar isso como dependência das tasks de build, para que um `.so`
desatualizado nunca entre no pacote. Há dois caminhos: deixar o **`cargo-ndk`** gerar os
`.so` e o Gradle só empacotá-los (mais simples), ou usar **`externalNativeBuild`** com
CMake quando o Rust é parte de um build nativo maior.

## Subtópicos

### External native build config

- O bloco `externalNativeBuild` do `build.gradle` integra builds nativos (CMake/ndk-build)
  ao Gradle. Usa-se quando há um `CMakeLists.txt` que, por sua vez, invoca o Cargo ou
  consome um `staticlib` Rust.
- Para projetos Rust puros, costuma ser desnecessário: basta gerar os `.so` com
  `cargo-ndk` e apontar `jniLibs.srcDirs`.

### CMake build file integration

- Quando se usa CMake, o `CMakeLists.txt` pode chamar o Cargo via `add_custom_command` e
  linkar o `.a` Rust (`staticlib`) em um target nativo.
- Plugins como **`rust-android-gradle`** (Mozilla) encapsulam isso, adicionando uma task
  `cargoBuild` que cuida de targets, NDK e cópia dos `.so` automaticamente:

```groovy
plugins { id "org.mozilla.rust-android-gradle.rust-android" }
cargo {
    module = "../rust"
    libname = "core"
    targets = ["arm64", "x86_64"]
}
```

### Cargo manifest inclusion

- O `Cargo.toml` do crate nativo precisa declarar `crate-type = ["cdylib"]` e ficar em um
  diretório conhecido pelo Gradle (ex.: `../rust`).
- Mantenha o crate Rust **fora** de `src/main` para separar fontes; o Gradle referencia o
  output, não o código-fonte.

### Build task dependency mapping

- O ponto crítico: a task que gera os `.so` deve rodar **antes** do empacotamento. Amarre-a
  às tasks `merge<Variant>JniLibFolders` / `preBuild`:

```groovy
tasks.whenTaskAdded { task ->
    if (task.name == 'mergeDebugJniLibFolders') task.dependsOn 'cargoBuildDebug'
}
```

- Sem esse mapeamento, o Gradle pode empacotar um `.so` velho (build não-reprodutível).

### Library search path definitions

- O Gradle procura `.so` em `android.sourceSets.main.jniLibs.srcDirs`. Aponte-o para onde
  o `cargo-ndk` despeja os artefatos:

```groovy
android {
    sourceSets.main.jniLibs.srcDirs += ['src/main/jniLibs']
}
```

- A estrutura deve ser `jniLibs/<abi>/lib<nome>.so` (ex.: `jniLibs/arm64-v8a/libcore.so`).

### ABI filter specification

- `ndk.abiFilters` limita quais ABIs entram no APK — evite empacotar ABIs que você não
  compila (causaria `UnsatisfiedLinkError` ou peso inútil).

```groovy
android {
    defaultConfig {
        ndk { abiFilters 'arm64-v8a', 'x86_64' }
    }
    // ou: splits.abi para gerar um APK por ABI, reduzindo o tamanho de download
}
```

- Garanta que os filtros do Gradle e os targets compilados pelo `cargo-ndk` **coincidam**.

## Referências

- rust-android-gradle: https://github.com/mozilla/rust-android-gradle
- Configure the NDK (abiFilters): https://developer.android.com/studio/projects/configure-agp-ndk
- cargo-ndk: https://github.com/bbqsrc/cargo-ndk
