# Build Environment

> **Seção:** Rust Toolchain Setup
> **Status:** ✅ preenchido

## Visão geral

O **ambiente de build** reúne todas as ferramentas necessárias para transformar código
Rust em bibliotecas `.so` consumíveis pelo Android: o **NDK** (Native Development Kit, que
fornece o toolchain Clang e as bibliotecas do sistema), o **SDK** (plataformas, build-tools,
emulador), as variáveis de ambiente que apontam para eles, e a automação que amarra tudo no
fluxo do Gradle. Um ambiente bem configurado é **reprodutível**: qualquer dev (ou a CI)
consegue buildar com um único comando, e o build de hoje é idêntico ao de seis meses atrás.

A reprodutibilidade é o tema central desta etapa. Os três fatores que mais geram o clássico
"na minha máquina funciona" são: (1) **versão do NDK** diferente entre máquinas, (2) **versão
do toolchain Rust** divergente, e (3) **variáveis de ambiente** apontando para caminhos
distintos. As seções abaixo atacam cada um — fixar versões, conteinerizar e padronizar
caminhos é o que separa um setup frágil de um robusto.

Antes de tudo, a base: instale o Rust via **`rustup`** (nunca o pacote do gerenciador do SO,
que costuma ficar velho) e fixe a versão do toolchain com um arquivo `rust-toolchain.toml`
versionado no repositório:

```toml
# rust-toolchain.toml — todos usam exatamente o mesmo Rust
[toolchain]
channel = "1.78.0"
targets = ["aarch64-linux-android", "x86_64-linux-android"]
components = ["clippy", "rustfmt"]
```

Com esse arquivo, ao rodar qualquer comando `cargo` o `rustup` baixa e usa automaticamente a
versão e os targets corretos — eliminando o fator (2) de uma vez.

## Subtópicos

### Android NDK installation

- O **NDK** contém o Clang/LLVM já configurado para cada ABI, os headers e as libs
  (`libc`/Bionic, `liblog`, etc.). É o componente que o linker do Rust usa (ver
  *Cross Compilation Targets → Target triple management*).
- Instale via **Android Studio → SDK Manager → SDK Tools → NDK (Side by side)** ou por
  linha de comando com `sdkmanager`:

```bash
sdkmanager --install "ndk;26.1.10909125"
```

- Prefira uma **versão LTS fixa** (ex.: r26 / r27 LTS) e fixe-a no projeto para garantir
  builds reprodutíveis entre máquinas. Anote a versão no README e/ou no `Dockerfile`.
- O toolchain fica em `$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/<host>/bin` — onde
  `<host>` é, por exemplo, `linux-x86_64` ou `darwin-x86_64`.
- **Versões importam:** NDK r23 removeu o GNU binutils antigo e o `libgcc` (substituído por
  `libunwind`); NDKs muito novos às vezes quebram crates que carregam scripts de build C
  desatualizados. Quando atualizar o NDK, rode o build completo das ABIs no CI antes de
  confiar.

### Cargo mobile integration

- Ferramentas dedicadas eliminam a configuração manual de linker, sysroot e cópia de `.so`:
  - **`cargo-ndk`** — wrapper que injeta linker/sysroot corretos e copia os `.so` para a
    estrutura `jniLibs/<abi>/`:
    ```bash
    cargo install cargo-ndk
    cargo ndk -t arm64-v8a -t x86_64 -o ./app/src/main/jniLibs build --release
    ```
    É o caminho mais comum e o que recomendamos como padrão. Note que ele aceita as **ABIs
    Android** (`arm64-v8a`) e traduz internamente para os triples Rust.
  - **`cargo-mobile2`** — scaffolding completo de projetos Android/iOS em Rust, útil para
    começar um projeto do zero com toda a estrutura pronta.
  - **`cargo-apk`** — empacota crates `cdylib` direto em APK (bom para protótipos e jogos
    `NativeActivity` sem camada Java/Kotlin; ver *Rendering Integration*).
- Para projetos com UI nativa (Kotlin/Compose) consumindo um núcleo Rust, **`cargo-ndk`** é
  o caminho mais comum; quando a integração é via Gradle, há também o plugin
  `rust-android-gradle` (ver *Gradle Build Integration*).

### Docker containerization strategy

- Containerizar o ambiente garante que **todos buildam com o mesmo NDK, Rust e SDK**,
  resolvendo de uma vez os três fatores de divergência citados na visão geral.
- Um `Dockerfile` típico parte de uma imagem com Rust e instala SDK/NDK e os targets:

```dockerfile
FROM rust:1.78-slim

ENV ANDROID_SDK_ROOT=/opt/android-sdk
ENV ANDROID_NDK_HOME=/opt/android-ndk/26.1.10909125

# 1. dependências do sistema (java p/ sdkmanager, unzip, etc.)
RUN apt-get update && apt-get install -y --no-install-recommends \
      openjdk-17-jdk-headless unzip curl && rm -rf /var/lib/apt/lists/*

# 2. cmdline-tools + NDK (omitido: download e unzip do sdkmanager)
#    sdkmanager --install "ndk;26.1.10909125" "platform-tools"

# 3. targets Rust + cargo-ndk
RUN rustup target add aarch64-linux-android x86_64-linux-android \
 && cargo install cargo-ndk

ENV PATH="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin:$PATH"
```

- Útil principalmente em **CI/CD**, onde o container vira a unidade reproduzível de build, e
  para onboarding (um `docker build` substitui um README de 40 passos).
- Imagens prontas da comunidade (ex.: as do projeto `rust-android-gradle` ou imagens com
  Android SDK + Rust) poupam manter esse `Dockerfile` à mão.

### Path environment configuration

- Variáveis essenciais que o toolchain e os scripts esperam:
  - `ANDROID_HOME` / `ANDROID_SDK_ROOT` — raiz do SDK (`ANDROID_HOME` é o nome legado,
    `ANDROID_SDK_ROOT` o atual; muitas ferramentas ainda leem ambos).
  - `ANDROID_NDK_HOME` — raiz do NDK.
  - `PATH` deve incluir `.../toolchains/llvm/prebuilt/<host>/bin` para achar os `*-clang`.
- Defina-as no shell profile (`.zshrc`/`.bashrc`) para uso local:

```bash
export ANDROID_SDK_ROOT="$HOME/Android/Sdk"
export ANDROID_NDK_HOME="$ANDROID_SDK_ROOT/ndk/26.1.10909125"
export PATH="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin:$PATH"
```

- **Evite hard-codar a versão do NDK em scripts**; derive de uma variável ou de um arquivo
  de versão único, para que atualizar o NDK seja uma mudança de uma linha.
- No CI, prefira definir essas variáveis no próprio job (ou no container) em vez de depender
  do ambiente da máquina — é o que torna o build determinístico.

### Linker path specifications

- Cada target precisa saber **qual binário usar como linker**. Os wrappers
  `<triple><API>-clang` do NDK já configuram sysroot e bibliotecas corretas (ver
  *Cross Compilation Targets → Target triple management*).
- Configuração em `.cargo/config.toml` ou via variáveis de ambiente, que têm precedência e
  são úteis no CI:
  - `CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER` — define o linker do target.
  - `CC_aarch64-linux-android` / `AR_aarch64-linux-android` — usados por crates com
    **build scripts em C** (via crate `cc`), como `ring`, `openssl-sys`, libs de imagem.
    Sem eles, esses crates tentam usar o compilador C do host e falham no cross-compile.
- O número de API no nome do clang define o **minSdk efetivo** do `.so`; mantenha-o
  alinhado ao `minSdkVersion` do `build.gradle` (descasamento → `UnsatisfiedLinkError`).

### Build script automation

- Automatize o fluxo "buildar Rust → copiar `.so` → buildar APK" para que ninguém precise
  lembrar os comandos. Opções:
  - **Makefile / `cargo-make`** com alvos como `make android-release`.
  - Task do Gradle que chama `cargo ndk` antes do `mergeJniLibs` (ver *Gradle Build
    Integration → Build task dependency mapping*).
  - Scripts `build.rs` para geração de código/bindings (ex.: `uniffi-bindgen`) antes da
    compilação.
- Exemplo de alvo Make que builda as duas ABIs essenciais:

```make
android-release:
	cargo ndk -t arm64-v8a -t x86_64 \
	  -o app/src/main/jniLibs build --release

android-debug:
	cargo ndk -t x86_64 -o app/src/main/jniLibs build
```

- **Boa prática:** o build de debug local pode compilar só `x86_64` (emulador) para ser
  rápido; o de release compila todas as ABIs de produção. Não pague o custo de compilar
  ARM no ciclo de iteração do dia a dia.

## Armadilhas comuns

- **`cmdline-tools` faltando** — o `sdkmanager` vive em `cmdline-tools/latest/bin`; se você
  baixou o zip num caminho diferente, o `sdkmanager` reclama de "latest" ausente. A
  estrutura `cmdline-tools/latest/` é obrigatória.
- **JDK errado** — o Gradle/AGP moderno exige JDK 17+. Um JDK 8/11 antigo no `PATH` gera
  erros obscuros do Gradle, não do Rust.
- **`target/` compartilhado entre host e Android** — builds para o host e para Android
  coexistem em `target/<triple>/`, mas misturar features pode confundir o cache; em dúvida,
  `cargo clean` ao trocar de contexto radicalmente.
- **Esquecer `cargo-ndk` no CI** — o `cargo install` é lento; cacheie `~/.cargo/bin` ou use
  a imagem Docker pré-pronta para não reinstalar a cada job.

## Assuntos correlatos

- **Cache de build no CI:** cachear `~/.cargo/registry`, `~/.cargo/git` e `target/` corta
  drasticamente o tempo (ver *Unit Testing → Continuous integration pipelines*).
- **`sccache`:** cache compartilhado de artefatos de compilação, útil quando várias ABIs
  recompilam dependências em comum.
- **Reprodutibilidade total:** além de fixar versões, `Cargo.lock` versionado garante
  dependências idênticas; combine com `rust-toolchain.toml` e NDK fixo para builds bit-a-bit
  mais previsíveis.

## Referências

- Android NDK Downloads: https://developer.android.com/ndk/downloads
- cargo-ndk: https://github.com/bbqsrc/cargo-ndk
- cargo-mobile2: https://github.com/tauri-apps/cargo-mobile2
- rustup (toolchain management): https://rust-lang.github.io/rustup/
- sdkmanager: https://developer.android.com/tools/sdkmanager
