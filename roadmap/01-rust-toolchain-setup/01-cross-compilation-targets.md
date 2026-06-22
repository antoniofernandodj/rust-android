# Cross Compilation Targets

> **Seção:** Rust Toolchain Setup
> **Status:** ✅ preenchido

## Visão geral

Para rodar código Rust dentro de um app Android é preciso **compilar de forma cruzada
(cross-compilation)**: o compilador (`rustc`, sobre o LLVM) roda no host
(Linux/macOS/Windows x86_64 ou um Mac ARM) mas gera código de máquina para a arquitetura do
dispositivo Android, que é outra. Cada arquitetura é identificada por um **target triple**
(`<arch>-<vendor>-<os>-<abi>`), e cada uma precisa de duas coisas: a **biblioteca padrão
pré-compilada** para aquele alvo (instalada via `rustup target add`) e um **linker** capaz
de gerar binários ELF para Android — papel desempenhado pelo Clang do NDK, que já vem com o
*sysroot* (headers e bibliotecas do sistema) embutido.

Vale entender o porquê do triple. O Rust herda a nomenclatura do LLVM/GNU. Em
`aarch64-linux-android`, `aarch64` é a arquitetura (ARM 64-bit), `linux` é o kernel (o
Android roda sobre o kernel Linux), e `android` é o "ABI/ambiente" — é o que diferencia de
um `aarch64-unknown-linux-gnu` (Linux desktop com glibc), porque o Android usa a **Bionic**
como libc, não a glibc. Essa diferença de libc é exatamente a razão de precisarmos de um
target dedicado: a ABI de chamadas de sistema, o layout de structs e as funções disponíveis
mudam.

As quatro ABIs oficialmente suportadas pelo Android mapeiam para quatro targets do Rust:

| ABI Android   | Target triple Rust              | Uso típico                          | Fatia de mercado |
|---------------|----------------------------------|-------------------------------------|------------------|
| `arm64-v8a`   | `aarch64-linux-android`          | Quase todos os celulares modernos   | ~95% dos devices |
| `armeabi-v7a` | `armv7-linux-androideabi`        | Dispositivos antigos / 32-bit       | minguante        |
| `x86`         | `i686-linux-android`             | Emuladores 32-bit                   | só emulador      |
| `x86_64`      | `x86_64-linux-android`           | Emuladores 64-bit / Chromebooks     | dev + ChromeOS   |

```bash
rustup target add \
  aarch64-linux-android \
  armv7-linux-androideabi \
  i686-linux-android \
  x86_64-linux-android
```

Na prática, a maioria dos apps só precisa de **`arm64-v8a`** (produção) e **`x86_64`**
(emulador no CI/dev). Compilar para as quatro ABIs quadruplica o tempo de build e o tamanho
do APK universal — só faça isso se você realmente tem usuários em hardware antigo.

## Subtópicos

### Architecture aarch64 linux android

- É a arquitetura **ARM 64-bit (`arm64-v8a`)**, padrão de praticamente todos os
  smartphones e tablets lançados desde ~2017. Deve ser sempre o primeiro alvo priorizado.
- Target: `aarch64-linux-android`.
- Linker do NDK: `aarch64-linux-android<API>-clang` (ex.: `aarch64-linux-android24-clang`),
  onde `<API>` é o nível mínimo de API que você suporta.
- Exemplo de build: `cargo build --target aarch64-linux-android --release`.
- O `.so` resultante deve ser colocado em `jniLibs/arm64-v8a/`.
- Como é ARMv8-A, você pode habilitar com segurança recursos como NEON (SIMD), que é
  obrigatório no AArch64 — diferentemente do ARMv7, onde NEON é opcional (ver
  *Performance Optimization → SIMD instruction sets*).

### Architecture armv7 linux androideabi

- ARM **32-bit (`armeabi-v7a`)**, para dispositivos mais antigos e de baixo custo.
- Target: `armv7-linux-androideabi` (note o sufixo **`eabi`** — usa a convenção de chamada
  EABI com suporte a hardware float `armv7-a + VFPv3`).
- Linker: `armv7a-linux-androideabi<API>-clang` (atenção: o prefixo do clang é
  **`armv7a`**, diferente do triple do Rust que é `armv7`). Esse descasamento de nome é uma
  das fontes mais comuns de erro "linker not found".
- `.so` vai em `jniLibs/armeabi-v7a/`.
- Cuidado com tipos de 64 bits: em ARM 32-bit, `usize`/`isize` são 32-bit, então ponteiros
  passados como `jlong` (sempre 64-bit) precisam de cast correto — um bug que só aparece
  nessa ABI e nunca no emulador x86_64.
- Tende a ser descontinuado; inclua apenas se precisar atingir aparelhos legados.

### Architecture i686 linux android

- x86 **32-bit**. Praticamente não existe em hardware real; serve para **emuladores
  32-bit** em máquinas de desenvolvimento Intel/AMD.
- Target: `i686-linux-android`.
- Linker: `i686-linux-android<API>-clang`.
- `.so` vai em `jniLibs/x86/`.
- Pode ser omitido se você só testa em emuladores 64-bit (o caso quase universal hoje).

### Architecture x86_64 linux android

- x86 **64-bit**. Principal alvo para **emuladores rápidos** em hosts x86_64 e para
  Chromebooks/Android-x86.
- Target: `x86_64-linux-android`.
- Linker: `x86_64-linux-android<API>-clang`.
- `.so` vai em `jniLibs/x86_64/`.
- Recomendado manter sempre, pois acelera muito o ciclo de desenvolvimento no emulador.
- **Nota para Macs ARM (M1/M2/M3):** o emulador roda imagens `arm64-v8a` nativamente, então
  nesses hosts você na verdade quer testar com `aarch64-linux-android` no emulador, não
  `x86_64`. Verifique a ABI do AVD antes de assumir.

### Cargo configuration profiles

- Os **profiles** do Cargo (`[profile.dev]`, `[profile.release]`) controlam otimização,
  símbolos de debug e tamanho do binário — críticos para apps móveis, onde cada MB do `.so`
  conta no tamanho de download.
- Para releases pequenos e rápidos, no `Cargo.toml`:

```toml
[profile.release]
opt-level = "z"      # otimiza para tamanho ("s" ou "z"); use 3 para velocidade
lto = true           # Link-Time Optimization reduz tamanho e melhora performance
codegen-units = 1    # melhor otimização à custa de tempo de compilação
panic = "abort"      # remove o código de unwinding (menor .so)
strip = true         # remove símbolos de debug do binário final
```

- **Trade-off de `opt-level`:** `"z"`/`"s"` priorizam tamanho, mas para código numérico
  pesado (imagem, áudio, cripto) o `3` costuma ser bem mais rápido e vale o `.so` maior.
  Meça os dois — às vezes `opt-level = 3` com `lto = true` gera binário menor *e* mais
  rápido porque o inlining elimina código.
- **`panic = "abort"`** tem uma consequência importante: você não pode mais usar
  `catch_unwind` para capturar panics na fronteira FFI (ver *Java Native Interface →
  Exception handling*). Se o seu binding depende disso, mantenha `panic = "unwind"`.
- Use um profile customizado (`[profile.release-mobile]`) quando quiser configs
  diferentes do release desktop, ativando-o com `cargo build --profile release-mobile`.

### Target triple management

- Centralize a configuração de cada target em **`.cargo/config.toml`** para não precisar
  passar `--target` e variáveis de ambiente manualmente toda vez.

```toml
# .cargo/config.toml
[target.aarch64-linux-android]
linker = "aarch64-linux-android24-clang"

[target.armv7-linux-androideabi]
linker = "armv7a-linux-androideabi24-clang"

[target.i686-linux-android]
linker = "i686-linux-android24-clang"

[target.x86_64-linux-android]
linker = "x86_64-linux-android24-clang"
```

- Esses wrappers `*-clang` precisam estar no `PATH` (ver *Build Environment → Path
  environment configuration*). Eles ficam em
  `$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/<host>/bin`.
- O **número de API embutido no nome do linker** (`24` no exemplo) define o `minSdkVersion`
  efetivo do `.so`. Se ele for *maior* que o `minSdkVersion` declarado no Gradle, o app
  pode crashar com `UnsatisfiedLinkError`/símbolos ausentes em devices antigos. Mantenha os
  dois alinhados.
- Ferramentas como **`cargo-ndk`** abstraem todo esse gerenciamento de triple/linker e
  ainda copiam os `.so` para os diretórios `jniLibs/` corretos automaticamente — na prática
  você raramente edita `.cargo/config.toml` à mão num projeto moderno (ver *Build
  Environment → Cargo mobile integration*).

## Armadilhas comuns

- **"linker `aarch64-linux-android24-clang` not found"** — o `bin/` do NDK não está no
  `PATH`, ou o `<API>` do nome não existe nessa versão do NDK. Liste o diretório `bin/`
  para ver os nomes exatos disponíveis.
- **`.so` na pasta errada** — `jniLibs/arm64-v8a/` ≠ `jniLibs/aarch64/`. O nome da pasta é a
  **ABI Android**, não o triple do Rust. Trocar isso gera `UnsatisfiedLinkError` em runtime,
  não erro de build.
- **NDK r23+ removeu o `libgcc`** e migrou para `libunwind`; crates muito antigos ou flags
  copiadas de tutoriais velhos podem falhar no link procurando `-lgcc`. Use um NDK e crates
  atualizados.
- **Esquecer um target no `rustup`** dá o erro "can't find crate for `std`" — sintoma
  clássico de `rustup target add` faltando.

## Assuntos correlatos

- **Tier de suporte do Rust:** os targets Android são *Tier 2* ("garantido que compila"),
  não *Tier 1* ("garantido que os testes passam"). Na prática são muito estáveis, mas vale
  saber o nível de garantia oficial.
- **`build-std`:** em nightly, recompilar a `std` para o alvo permite aplicar
  `panic=abort`/`opt-level=z` à própria biblioteca padrão, encolhendo mais o `.so` (ver
  *Shared Object Creation → Size optimization flags*).
- **16 KB page size:** o Android 15+ migra para páginas de memória de 16 KB em alguns
  dispositivos; `.so` precisam ser alinhados adequadamente no link. NDKs recentes já fazem
  isso por padrão, mas é um ponto a monitorar para apps nativos.

## Referências

- Rust Platform Support: https://doc.rust-lang.org/rustc/platform-support.html
- Android ABIs: https://developer.android.com/ndk/guides/abis
- cargo-ndk: https://github.com/bbqsrc/cargo-ndk
- Cargo profiles: https://doc.rust-lang.org/cargo/reference/profiles.html
- Support for 16 KB page sizes: https://developer.android.com/guide/practices/page-sizes
