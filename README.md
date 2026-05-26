# sharedroid

Hello World Android app built with [iced](https://iced.rs/) (Rust GUI framework).

## Pré-requisitos mínimos

| Ferramenta  | Versão          | Observação                         |
|-------------|-----------------|-------------------------------------|
| Rust        | stable (≥ 1.80) | via [rustup.rs](https://rustup.rs) |
| Java        | 17              | necessário para o `sdkmanager`      |
| curl / wget | qualquer        | para baixar o SDK Android           |

> Java 17 é instalado automaticamente pelo `make setup` via [SDKMAN](https://sdkman.io).

## Início rápido

```bash
# 1. Instalar todas as dependências (só na primeira vez)
make setup

# 2. Compilar o APK debug
make build
# → target/debug/apk/sharedroid.apk (≈ 170 MB em debug)

# 3. Instalar em um device Android conectado
make install

# 4. Testar no desktop Linux (opcional)
cargo run -p desktop
```

## Estrutura do projeto

```
sharedroid/
├── src/lib.rs          ← código da UI (iced Hello World)
├── desktop/            ← wrapper para rodar no desktop
│   └── src/main.rs
├── patches/
│   └── iced_winit/     ← patch de iced_winit 0.13 para Android
│       └── src/
│           ├── conversion.rs   (guarda modifier_supplement)
│           ├── lib.rs          (expõe android::set_android_app)
│           └── program.rs      (passa AndroidApp ao EventLoop)
├── Cargo.toml          ← workspace + metadados Android
├── Makefile            ← setup / build / install / run
└── README.md
```

## Como o patch funciona

`iced_winit 0.13` tem dois problemas ao compilar para Android:

1. **`modifier_supplement` inexistente no Android** – o módulo
   `winit::platform::modifier_supplement` só existe em Linux/macOS/Windows.
   O patch adiciona `#[cfg(not(target_os = "android"))]` nos locais relevantes.

2. **EventLoop sem AndroidApp** – winit 0.30 exige que o `AndroidApp` seja
   passado explicitamente ao criar o `EventLoop` no Android.
   O patch:
   - Expõe `iced_winit::android::set_android_app(app)` (global `OnceLock`)
   - Passa o app salvo ao `EventLoopBuilder` antes de construir o loop

O `android_main` em `src/lib.rs` chama `set_android_app(android_app)` antes de
`iced::application(...).run()`, conectando tudo.

## Alvos Android

| ABI              | Target Rust                |
|------------------|----------------------------|
| arm64-v8a        | `aarch64-linux-android`    |
| armeabi-v7a      | `armv7-linux-androideabi`  |
| x86_64 (emulador)| `x86_64-linux-android`     |

Por padrão só `aarch64` é compilado. Para adicionar outros, edite
`build_targets` em `Cargo.toml` → `[package.metadata.android]`.

## Variáveis de ambiente relevantes

```bash
ANDROID_HOME      = ~/android-sdk
ANDROID_NDK_ROOT  = ~/android-sdk/ndk/25.2.9519653
JAVA_HOME         = ~/.sdkman/candidates/java/current
```
