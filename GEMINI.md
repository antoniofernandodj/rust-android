# rustandroid

App Android "Hello World" feito com o framework de interface [iced](https://iced.rs/) (Rust).

## Visão Geral do Projeto

Este projeto demonstra como construir uma aplicação GUI multiplataforma (Android e Desktop) usando o `iced`. Como o `iced` (v0.13) não possui suporte oficial para Android, este repositório inclui um patch local para o `iced_winit` para lidar com o ciclo de vida do `NativeActivity` do Android e os requisitos do loop de eventos.

### Tecnologias Principais
- **Rust**: Linguagem principal.
- **iced (v0.13)**: Framework de interface (inspirado em Elm).
- **wgpu**: Backend gráfico.
- **winit (v0.30)**: Gerenciamento de janelas e eventos.
- **cargo-apk**: Ferramenta para compilar e empacotar APKs Android.
- **SDKMAN**: Usado para gerenciar o Java 17 (exigido pelas ferramentas do Android SDK).

### Arquitetura
- **Raiz (`rustandroid`)**: A biblioteca principal (`src/lib.rs`) que contém a lógica da interface e o ponto de entrada para Android (`android_main`).
- **`desktop/`**: Um membro do workspace que fornece um wrapper simples para rodar o app em plataformas desktop (Linux/macOS/Windows).
- **`patches/iced_winit/`**: Um fork local do `iced_winit` que corrige problemas específicos do Android (ex: ausência de `modifier_supplement` e a necessidade do `AndroidApp` no `EventLoop`).
- **`src/fonts/`**: Contém `FiraSans-Regular.ttf`, que é embutido no binário para garantir que o texto seja renderizado corretamente no Android (onde as fontes do sistema não são facilmente acessíveis para o Rust).

## Compilação e Execução

O projeto utiliza um `Makefile` para simplificar as tarefas comuns.

### Configuração (Setup)
Execute este comando uma vez para instalar o Java 17, Android SDK/NDK, alvos do Rust e o `cargo-apk`.
```bash
make setup
```

### Android
- **Compilar APK de Debug**: `make build` (Saída: `target/debug/apk/rustandroid.apk`)
- **Compilar APK de Release**: `make release`
- **Instalar no Dispositivo**: `make install` (Requer `adb` e um dispositivo/emulador conectado)
- **Rodar no Dispositivo**: `make run` (Compila, instala e inicia o app, seguido pelo `logcat`)

### Desktop
Para testar a interface rapidamente na sua máquina local:
```bash
cargo run -p desktop
```

### Variáveis de Ambiente
As seguintes variáveis são usadas pelo sistema de build (configuradas com padrões no `Makefile`):
- `ANDROID_HOME`: Caminho para o Android SDK (padrão: `~/android-sdk`)
- `ANDROID_NDK_ROOT`: Caminho para o Android NDK (padrão: `~/android-sdk/ndk/25.2.9519653`)
- `JAVA_HOME`: Caminho para o Java 17 (padrão: `~/.sdkman/candidates/java/current`)

## Convenções de Desenvolvimento

### Patches
Se você precisar modificar como o `iced` interage com o sistema de janelas de baixo nível no Android, edite os arquivos em `patches/iced_winit/`. Essas mudanças são aplicadas automaticamente via seção `[patch.crates-io]` no `Cargo.toml` raiz.

### Interface e Ponto de Entrada Android
- Todo o código da interface deve residir em `src/lib.rs`.
- A função `android_main` em `src/lib.rs` é o ponto de entrada para Android. Ela inicializa o logger e define o global `AndroidApp` antes de chamar a função compartilhada `run()`.

### Gerenciamento de Fontes
No Android, o `iced` não consegue encontrar as fontes do sistema de forma confiável. Para exibir texto:
1.  Coloque arquivos `.ttf` em `src/fonts/`.
2.  Use `include_bytes!` para embuti-los em `src/lib.rs`.
3.  Carregue-os usando `.font()` e defina um padrão com `.default_font()` no builder do `iced::application`.

### Logs (Registro de Eventos)
- Use a crate `log` para logs.
- No Android, os logs são gerenciados pelo `android_logger` e podem ser visualizados via `adb logcat -s RustStdoutStderr:D`.
