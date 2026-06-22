# Cross Compilation Targets

> **Seção:** Rust Toolchain Setup
> **Status:** ✅ preenchido

## Visão geral

Para rodar código Rust dentro de um app Android é preciso **compilar de forma cruzada
(cross-compilation)**: o compilador roda no host (Linux/macOS/Windows x86_64) mas gera
código de máquina para a arquitetura do dispositivo Android. Cada arquitetura é
identificada por um **target triple** (`<arch>-<vendor>-<os>-<abi>`), e cada uma precisa
ser adicionada via `rustup target add` e ter um linker do NDK configurado.

As quatro ABIs oficialmente suportadas pelo Android mapeiam para quatro targets do Rust:

| ABI Android   | Target triple Rust              | Uso típico                          |
|---------------|----------------------------------|-------------------------------------|
| `arm64-v8a`   | `aarch64-linux-android`          | Quase todos os celulares modernos   |
| `armeabi-v7a` | `armv7-linux-androideabi`        | Dispositivos antigos / 32-bit       |
| `x86`         | `i686-linux-android`             | Emuladores 32-bit                   |
| `x86_64`      | `x86_64-linux-android`           | Emuladores 64-bit / Chromebooks     |

```bash
rustup target add \
  aarch64-linux-android \
  armv7-linux-androideabi \
  i686-linux-android \
  x86_64-linux-android
```

## Subtópicos

### Architecture aarch64 linux android

- É a arquitetura **ARM 64-bit (`arm64-v8a`)**, padrão de praticamente todos os
  smartphones e tablets lançados desde ~2017. Deve ser sempre o primeiro alvo priorizado.
- Target: `aarch64-linux-android`.
- Linker do NDK: `aarch64-linux-android<API>-clang` (ex.: `aarch64-linux-android24-clang`),
  onde `<API>` é o nível mínimo de API que você suporta.
- Exemplo de build: `cargo build --target aarch64-linux-android --release`.
- O `.so` resultante deve ser colocado em `jniLibs/arm64-v8a/`.

### Architecture armv7 linux androideabi

- ARM **32-bit (`armeabi-v7a`)**, para dispositivos mais antigos e de baixo custo.
- Target: `armv7-linux-androideabi` (note o sufixo **`eabi`** — usa a convenção de chamada
  EABI com suporte a hardware float `armv7-a + VFPv3`).
- Linker: `armv7a-linux-androideabi<API>-clang` (atenção: o prefixo do clang é
  **`armv7a`**, diferente do triple do Rust que é `armv7`).
- `.so` vai em `jniLibs/armeabi-v7a/`.
- Tende a ser descontinuado; inclua apenas se precisar atingir aparelhos legados.

### Architecture i686 linux android

- x86 **32-bit**. Praticamente não existe em hardware real; serve para **emuladores
  32-bit** em máquinas de desenvolvimento Intel/AMD.
- Target: `i686-linux-android`.
- Linker: `i686-linux-android<API>-clang`.
- `.so` vai em `jniLibs/x86/`.
- Pode ser omitido se você só testa em emuladores 64-bit.

### Architecture x86_64 linux android

- x86 **64-bit**. Principal alvo para **emuladores rápidos** em hosts x86_64 e para
  Chromebooks/Android-x86.
- Target: `x86_64-linux-android`.
- Linker: `x86_64-linux-android<API>-clang`.
- `.so` vai em `jniLibs/x86_64/`.
- Recomendado manter sempre, pois acelera muito o ciclo de desenvolvimento no emulador.

### Cargo configuration profiles

- Os **profiles** do Cargo (`[profile.dev]`, `[profile.release]`) controlam otimização,
  símbolos de debug e tamanho do binário — críticos para apps móveis.
- Para releases pequenos e rápidos, no `Cargo.toml`:

```toml
[profile.release]
opt-level = "z"      # otimiza para tamanho ("s" ou "z"); use 3 para velocidade
lto = true           # Link-Time Optimization reduz tamanho e melhora performance
codegen-units = 1    # melhor otimização à custa de tempo de compilação
panic = "abort"      # remove o código de unwinding (menor .so)
strip = true         # remove símbolos de debug do binário final
```

- Use um profile customizado (`[profile.release-mobile]`) quando quiser configs
  diferentes do release desktop.

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

- Ferramentas como **`cargo-ndk`** abstraem todo esse gerenciamento de triple/linker e
  ainda copiam os `.so` para os diretórios `jniLibs/` corretos automaticamente.

## Referências

- Rust Platform Support: https://doc.rust-lang.org/rustc/platform-support.html
- Android ABIs: https://developer.android.com/ndk/guides/abis
- cargo-ndk: https://github.com/bbqsrc/cargo-ndk
