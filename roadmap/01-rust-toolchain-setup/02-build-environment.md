# Build Environment

> **Seção:** Rust Toolchain Setup
> **Status:** ✅ preenchido

## Visão geral

O **ambiente de build** reúne todas as ferramentas necessárias para transformar código
Rust em bibliotecas `.so` consumíveis pelo Android: o **NDK** (Native Development Kit, que
fornece o toolchain Clang e as bibliotecas do sistema), as variáveis de ambiente que
apontam para ele, e a automação que amarra tudo no fluxo do Gradle. Um ambiente bem
configurado é reprodutível: qualquer dev (ou a CI) consegue buildar com um único comando.

## Subtópicos

### Android NDK installation

- O **NDK** contém o Clang/LLVM já configurado para cada ABI, os headers e as libs
  (`libc`, `liblog`, etc.). É o componente que o linker do Rust usa.
- Instale via **Android Studio → SDK Manager → SDK Tools → NDK (Side by side)** ou por
  linha de comando com `sdkmanager`:

```bash
sdkmanager --install "ndk;26.1.10909125"
```

- Prefira uma **versão LTS fixa** (ex.: r26) e fixe-a no projeto para garantir builds
  reprodutíveis entre máquinas.
- O toolchain fica em `$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/<host>/bin`.

### Cargo mobile integration

- Ferramentas dedicadas eliminam a configuração manual:
  - **`cargo-ndk`** — wrapper que injeta linker/sysroot corretos e copia os `.so`:
    ```bash
    cargo install cargo-ndk
    cargo ndk -t arm64-v8a -t x86_64 -o ./app/src/main/jniLibs build --release
    ```
  - **`cargo-mobile2`** — scaffolding completo de projetos Android/iOS em Rust.
  - **`cargo-apk`** — empacota crates `cdylib` direto em APK (bom para protótipos
    sem camada Java/Kotlin).
- Para projetos com UI nativa (Kotlin/Compose), `cargo-ndk` é o caminho mais comum.

### Docker containerization strategy

- Containerizar o ambiente garante que **todos buildam com o mesmo NDK, Rust e SDK**,
  evitando o clássico "na minha máquina funciona".
- Um `Dockerfile` típico parte de uma imagem com Rust + SDK/NDK (ex.: baseada em
  `rust:slim`) e instala os targets:

```dockerfile
FROM rust:1.78-slim
ENV ANDROID_NDK_HOME=/opt/android-ndk
RUN rustup target add aarch64-linux-android x86_64-linux-android \
 && cargo install cargo-ndk
# ... copiar NDK, definir PATH ...
```

- Útil principalmente em **CI/CD**, onde o container vira a unidade reproduzível de build.

### Path environment configuration

- Variáveis essenciais que o toolchain e os scripts esperam:
  - `ANDROID_HOME` / `ANDROID_SDK_ROOT` — raiz do SDK.
  - `ANDROID_NDK_HOME` — raiz do NDK.
  - `PATH` deve incluir `.../toolchains/llvm/prebuilt/<host>/bin` para achar os clang.
- Defina-as no shell profile (`.zshrc`/`.bashrc`) ou em um `.env` versionado por exemplo:

```bash
export ANDROID_NDK_HOME="$HOME/Android/Sdk/ndk/26.1.10909125"
export PATH="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin:$PATH"
```

### Linker path specifications

- Cada target precisa saber **qual binário usar como linker**. Os wrappers
  `<triple><API>-clang` do NDK já configuram sysroot e bibliotecas corretas.
- Configuração em `.cargo/config.toml` (ver doc de *Cross Compilation Targets*) ou via
  variáveis `CARGO_TARGET_<TRIPLE>_LINKER` e `CC_<triple>`/`AR_<triple>` quando crates
  com build scripts em C precisam compilar código nativo.
- O número de API no nome do clang define o **minSdk** efetivo do `.so`; mantenha-o
  alinhado ao `minSdkVersion` do `build.gradle`.

### Build script automation

- Automatize o fluxo "buildar Rust → copiar `.so` → buildar APK" para que ninguém precise
  lembrar os comandos. Opções:
  - **Makefile / `cargo-make`** com alvos como `make android-release`.
  - Task do Gradle que chama `cargo ndk` antes do `mergeJniLibs` (ver doc de *Gradle
    Build Integration*).
  - Scripts `build.rs` para geração de código/bindings antes da compilação.
- Exemplo de alvo Make:

```make
android-release:
	cargo ndk -t arm64-v8a -t x86_64 \
	  -o app/src/main/jniLibs build --release
```

## Referências

- Android NDK Downloads: https://developer.android.com/ndk/downloads
- cargo-ndk: https://github.com/bbqsrc/cargo-ndk
- cargo-mobile2: https://github.com/tauri-apps/cargo-mobile2
