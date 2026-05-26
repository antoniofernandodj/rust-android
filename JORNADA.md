# Jornada: Hello World Android com iced (Rust)

> Como foi construir um APK Android com o framework de UI `iced` do zero,
> passando por cada obstáculo até o texto aparecer na tela.

---

## O objetivo

Criar o app Android mais simples possível usando **iced** — um framework de UI
em Rust inspirado no Elm — e compilá-lo como APK instalável num dispositivo real.
A tela seria apenas um "Hello, World!" centralizado.

---

## O ambiente

- **Sistema:** Linux x86_64
- **Rust:** 1.90 (stable)
- **Projeto inicial:** `cargo new sharedroid` — literalmente só um `println!`

Nada estava instalado: sem Android SDK, sem NDK, sem Java moderno, sem
targets Android no rustup.

---

## Fase 1 — Instalar o Android SDK e NDK

### Problema: Java 11 não roda o `sdkmanager`

O `sdkmanager` moderno exige Java 17+, mas o sistema tinha Java 11.
Como não havia acesso a `sudo`, a solução foi instalar Java 17 via
**SDKMAN** (gerenciador de versões sem root):

```bash
curl -s "https://get.sdkman.io" | bash
source ~/.sdkman/bin/sdkman-init.sh
sdk install java 17.0.13-tem
```

### Download do SDK

```bash
wget "https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip"
unzip ... -d ~/android-sdk/cmdline-tools/latest/
```

### Problema: plataforma android-34 incompatível com NDK 25.2

Após instalar o NDK 25.2.9519653, o `cargo-apk` reclamou:
> "Android SDK has no platforms installed"

O NDK 25.2 suporta até API level **33**. Havíamos instalado `platforms;android-34`.
Fix: instalar `platforms;android-33`.

```bash
sdkmanager "platforms;android-33"
```

---

## Fase 2 — Configurar o Rust para Android

### Targets cross-compilation

```bash
rustup target add aarch64-linux-android armv7-linux-androideabi \
                  x86_64-linux-android i686-linux-android
```

### cargo-apk

`cargo-apk` é uma ferramenta que compila o crate como `cdylib`,
envolve com o glue code do NativeActivity e empacota tudo num APK:

```bash
cargo install cargo-apk
```

---

## Fase 3 — Estruturar o projeto

A primeira estrutura de `Cargo.toml` tentou ter tanto um `[[bin]]`
(para desktop) quanto um `[lib]` (para Android). O `cargo-apk`
processava **todos** os targets — e entrava em pânico ao tentar
empacotar o binário junto com a cdylib:

```
thread 'main' panicked: Bin is not compatible with Cdylib
```

### Solução: workspace

Separamos em dois membros:

```
sharedroid/          ← lib Android (cdylib + rlib)
└── desktop/         ← binary separado para testar no Linux
```

O `cargo-apk` só vê o membro raiz e não toca no `desktop/`.

---

## Fase 4 — Primeira tentativa de build Android

```bash
cargo apk check
```

### Erro 1: `android-activity` sem feature `native-activity`

```
error[E0583]: file not found for module `activity_impl`
```

A crate `android-activity` precisa de uma das features
`native-activity` ou `game-activity` para compilar.
Tínhamos a crate como dependência opcional por trás de um feature
flag — e o `cargo-apk` não a ativava.

**Fix:** dependência target-específica, sempre ativa no Android:

```toml
[target.'cfg(target_os = "android")'.dependencies]
android-activity = { version = "0.6", features = ["native-activity"] }
winit            = { version = "0.30", features = ["android-native-activity"] }
```

---

## Fase 5 — Patch 1: `modifier_supplement` inexistente no Android

Após corrigir o `android-activity`, o novo erro foi em `iced_winit 0.13`:

```
error[E0432]: unresolved import `winit::platform::modifier_supplement`
  --> iced_winit-0.13.0/src/conversion.rs:198
```

O módulo `winit::platform::modifier_supplement` (com métodos como
`key_without_modifiers`) existe em Linux/macOS/Windows, mas **não no Android**.

`iced_winit 0.13` protegia isso apenas com:

```rust
#[cfg(not(target_arch = "wasm32"))]   // ← nunca pensou no Android!
{
    use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
    event.key_without_modifiers()
}
```

### Fix: patch local do `iced_winit`

Copiamos o fonte do `iced_winit 0.13` para `patches/iced_winit/` e
adicionamos `target_os = "android"` ao guard:

```rust
#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios")))]
{
    use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
    event.key_without_modifiers()
}

#[cfg(any(target_arch = "wasm32", target_os = "android", target_os = "ios"))]
{
    event.logical_key.clone()  // fallback sem modifier supplement
}
```

Registrado no `Cargo.toml`:

```toml
[patch.crates-io]
iced_winit = { path = "patches/iced_winit" }
```

---

## Fase 6 — Patch 2: `EventLoop` sem `AndroidApp`

O próximo obstáculo foi em tempo de execução.
No Android, **winit 0.30** exige que o `EventLoop` receba o `AndroidApp`
explicitamente via `with_android_app()`:

```rust
EventLoop::builder()
    .with_android_app(app)  // obrigatório no Android
    .build()
```

Mas `iced_winit::program::run()` criava o loop assim, sem nenhum app:

```rust
let event_loop = EventLoop::with_user_event()
    .build()                // panic no Android! AndroidApp ausente
    .expect("Create event loop");
```

### Fix: global `OnceLock` no patch do `iced_winit`

Adicionamos ao patch um módulo `android` que armazena o `AndroidApp`
antes do loop ser criado:

```rust
// patches/iced_winit/src/lib.rs
#[cfg(target_os = "android")]
pub mod android {
    use std::sync::OnceLock;
    pub use winit::platform::android::activity::AndroidApp;

    static ANDROID_APP: OnceLock<AndroidApp> = OnceLock::new();

    pub fn set_android_app(app: AndroidApp) {
        let _ = ANDROID_APP.set(app);
    }

    pub(crate) fn get_android_app() -> AndroidApp {
        ANDROID_APP.get().expect("AndroidApp not set").clone()
    }
}
```

E no `program.rs` do patch, passamos o app ao criar o loop:

```rust
#[cfg(target_os = "android")]
{
    use winit::platform::android::EventLoopBuilderExtAndroid;
    EventLoop::with_user_event()
        .with_android_app(crate::android::get_android_app())
        .build()
        .expect("Create event loop")
}
```

O `android_main` no nosso `lib.rs` registra o app antes de chamar `run()`:

```rust
#[no_mangle]
fn android_main(android_app: iced_winit::android::AndroidApp) {
    iced_winit::android::set_android_app(android_app);
    run().expect("iced app failed");
}
```

---

## Fase 7 — Primeiro APK funcionando (parcialmente)

```
✅  APK built: target/debug/apk/sharedroid.apk  (170 MB debug)
```

O app instalava e abria. Mas a tela mostrava apenas **cinza escuro** —
o fundo do `Theme::Dark` renderizava corretamente, mas nenhum texto aparecia.

---

## Fase 8 — Desvio: tentativa com `tiny-skia`

A hipótese era que o pipeline de texto do `wgpu` tinha problemas com
shaders no Android. Tentamos trocar para o renderer `tiny-skia`
(software rendering, sem GPU):

```toml
iced = { version = "0.13", default-features = false, features = ["tiny-skia", "web-colors"] }
```

Resultado: **tela completamente em branco** — pior que antes.

`tiny-skia` usa `softbuffer` para apresentar os pixels, e o `softbuffer`
não conseguia acessar a surface do `ANativeWindow` corretamente nessa
configuração. Voltamos para `wgpu`.

---

## Fase 9 — Root cause real: fonte não carregada no Android

Após análise cuidadosa do código-fonte de `iced_graphics 0.13`,
encontramos a causa raiz em `src/text.rs`:

```rust
// Inicialização global do FontSystem
cosmic_text::FontSystem::new_with_fonts([
    // ícones — sempre carregados
    Source::Binary(include_bytes!("../fonts/Iced-Icons.ttf")),

    // FiraSans — SÓ carregada em wasm32!
    #[cfg(all(target_arch = "wasm32", feature = "fira-sans"))]
    Source::Binary(include_bytes!("../fonts/FiraSans-Regular.ttf")),
])
```

**No Android**, o `cosmic-text` inicializava com apenas a fonte de ícones
(`Iced-Icons.ttf`), que não contém letras latinas. Para texto normal,
tentava carregar fontes do sistema via `fontdb` em `/system/fonts/`,
mas isso falhava silenciosamente — resultando em texto 100% transparente
sobre o fundo cinza do wgpu.

### Fix final: embutir a fonte no binário

Copiamos `FiraSans-Regular.ttf` (que já estava no registry do `iced_graphics`)
para `src/fonts/` e a carregamos explicitamente:

```rust
const FIRA_SANS: &[u8] = include_bytes!("fonts/FiraSans-Regular.ttf");

pub fn run() -> iced::Result {
    iced::application("Hello World", App::update, App::view)
        .theme(|_| Theme::Dark)
        .font(FIRA_SANS)                           // embute no APK
        .default_font(Font::with_name("Fira Sans")) // força o uso dela
        .run()
}
```

---

## Resultado final

O APK instala, abre, e exibe "Hello, World!" centralizado em branco
sobre fundo escuro — exatamente o objetivo.

```
✅  APK built: target/debug/apk/sharedroid.apk  (170 MB debug)
```

---

## Mapa dos problemas

```
Android SDK
└── Java 11 não roda sdkmanager          → SDKMAN + Java 17
└── API 34 incompatível com NDK 25.2     → instalar platforms;android-33

Rust / cargo
└── android-activity sem feature         → dep target-específica
└── cargo-apk + bin + lib = panic        → workspace separado

iced_winit 0.13 (patch necessário)
└── modifier_supplement inexistente      → cfg(not(android)) guard
└── EventLoop sem AndroidApp             → OnceLock global + with_android_app

Rendering
└── tiny-skia não funciona no Android    → volta para wgpu
└── Texto invisível com wgpu             → FiraSans embutida via include_bytes!
                                           (root cause: só carregada em wasm32)
```

---

## Arquitetura final

```
sharedroid/
├── src/
│   ├── lib.rs              ← UI iced + android_main
│   └── fonts/
│       └── FiraSans-Regular.ttf
├── desktop/
│   └── src/main.rs         ← wrapper para rodar no Linux
├── patches/
│   └── iced_winit/         ← fork local do iced_winit 0.13
│       └── src/
│           ├── conversion.rs   (guard modifier_supplement)
│           ├── lib.rs          (android::set_android_app)
│           └── program.rs      (with_android_app no EventLoop)
├── Cargo.toml              ← workspace + [patch.crates-io]
└── Makefile                ← setup / build / install
```

---

## Lições aprendidas

1. **iced 0.13 não tem suporte oficial a Android** — funciona, mas requer patches.
   O time do iced focou em desktop/wasm; o caminho Android está parcialmente aberto.

2. **winit 0.30 exige `AndroidApp` explícito** — não há global automático como
   em versões anteriores. Todo framework que usa winit precisa propagar o `AndroidApp`.

3. **Fontes precisam ser embutidas no Android** — `fontdb` não encontra
   `/system/fonts/` de forma confiável no contexto de um NativeActivity Rust.
   O padrão correto é `include_bytes!` + `.font()` no iced.

4. **`tiny-skia` vs `wgpu` no Android** — `wgpu` funciona melhor que `tiny-skia`
   nesse setup. O `softbuffer` (backend do tiny-skia) tem suporte a Android
   mais frágil que o Vulkan/GLES do wgpu.

5. **`[patch.crates-io]` é poderoso** — permite corrigir bugs em dependências
   sem aguardar um release upstream, mantendo o patch versionado junto ao projeto.
