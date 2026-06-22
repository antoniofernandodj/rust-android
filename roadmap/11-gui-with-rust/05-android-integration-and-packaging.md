# Android Integration & Packaging (UI em Rust)

> **Seção:** GUI with Rust
> **Status:** ✅ preenchido

## Visão geral

Os capítulos anteriores cobriram *como desenhar a UI* em Rust — de baixo nível (*Rendering
Integration*) aos toolkits egui, iced e Slint. Este capítulo fecha a seção tratando do que é
comum a todos: **como um app de UI 100% Rust vira um app Android instalável** — o ponto de
entrada nativo, o ciclo de vida, o input/IME, a permissão e o empacotamento — e **como
escolher** entre os toolkits.

Há uma decisão de arquitetura que precede tudo: **UI em Rust vs. UI em Compose com núcleo
Rust** (ver *UI Layer Basics*). Não é "uma é melhor"; são casos diferentes:

- **UI em Rust** (egui/iced/Slint) — quando você quer **uma base de UI única
  multiplataforma** (desktop + mobile), um jogo/ferramenta gráfica, ou independência do
  toolkit Android. Custo: integração nativa (IME, acessibilidade, *look & feel* Android) é
  por sua conta ou do toolkit, e quase nunca fica tão "nativa" quanto Compose.
- **Compose + núcleo Rust** — quando é um **app de produto Android** que deve parecer e se
  comportar como app Android (Material, acessibilidade, navegação, integração do sistema).
  É o caminho recomendado para a maioria dos apps; a UI em Rust é a exceção especializada.

## Subtópicos

### Como escolher o toolkit (egui × iced × Slint)

| Critério | egui | iced | Slint |
|----------|------|------|-------|
| Modelo | imediato (IMGUI) | retido / Elm (TEA) | declarativo / reativo (DSL) |
| UI descrita em | Rust, por frame | Rust, `view()` | arquivo `.slint` |
| Suporte Android | bom (winit) | experimental | **oficial e maduro** |
| IME/teclado, ciclo de vida | você ajusta | você ajusta | backend cuida |
| Licença | MIT/Apache | MIT | **dual (ver licença)** |
| Forte em | ferramentas, debug, jogos | apps previsíveis, multiplataforma | apps "com cara nativa", embarcados |

- **Quer o menor atrito para um app Android com cara decente em Rust puro?** Slint (backend
  Android oficial), atento à licença.
- **Ferramenta/overlay/jogo, UI muito dinâmica, simplicidade?** egui.
- **Base única desktop+mobile com arquitetura previsível (TEA) e licença permissiva?** iced —
  validando antes o suporte mobile.
- **App de produto Android sério?** reconsidere: Compose + núcleo Rust (*UI Layer Basics*).

### Ponto de entrada nativo: `android-activity` e `NativeActivity`/`GameActivity`

- Um app de UI Rust **não tem Activity Kotlin** (ou tem o mínimo). O ponto de entrada é uma
  **`NativeActivity`** (apps gerais) ou **`GameActivity`** (jogos, com integração de input
  melhor), abstraídos pelo crate **`android-activity`** — a base sobre a qual winit, egui,
  iced e Slint rodam no Android.
- O padrão é uma função anotada com `#[no_mangle] fn android_main(app: AndroidApp)` (via macro
  `#[android_activity::main]` ou o re-export do toolkit), que recebe o `AndroidApp` com acesso
  a janela, ciclo de vida, input e `AssetManager`:

```rust
#[no_mangle]
fn android_main(app: android_activity::AndroidApp) {
    // egui/iced/Slint constroem o event loop sobre este `app`
    minha_ui::rodar(app);
}
```

- O `AndroidManifest.xml` declara a `NativeActivity` e aponta `android.app.lib_name` para o
  seu `.so` (ver *Application Lifecycle → Manifest configuration files*). O crate `cargo-apk`
  gera esse manifest automaticamente para casos simples.

### Ciclo de vida, surface e input/IME

- Os mesmos cuidados de *Rendering Integration* valem, mas aqui são **mediados pelo toolkit**:
  - **Surface criada/destruída** ao entrar/sair de background — o toolkit pausa/retoma o
    render; você só não deve segurar recursos GPU através da destruição.
  - **Rotação/resize** — reconfigurar a surface; toolkits retidos relayoutam sozinhos.
  - **Input e densidade** — toque/teclas chegam via `android-activity`; aplique o *scale
    factor* (o Slint/eframe já fazem).
- **IME (teclado virtual)** é o ponto historicamente mais espinhoso em apps nativos: mostrar/
  esconder o teclado, receber texto composto, *autofill*. **Slint** trata isso no backend
  oficial; **egui/iced** têm suporte variável e podem exigir ajustes/JNI ao
  `InputMethodManager`. **Teste digitação cedo.**
- **Acessibilidade:** o ecossistema usa **AccessKit** para expor a árvore de UI ao TalkBack;
  o suporte é crescente mas não equivale ao de Compose. Se acessibilidade é requisito legal,
  pese isso na escolha.

### Lógica de negócio: o núcleo continua puro

- Mesmo com a UI em Rust, **mantenha a separação núcleo puro × camada de UI** (ver *Cross
  Platform Logic*): a UI (egui/iced/Slint) é só apresentação; regras, estado e I/O ficam num
  crate de núcleo testável no host. Isso preserva a testabilidade (*Unit Testing*) e permite
  reusar o mesmo núcleo se um dia a UI mudar (para Compose, desktop, etc.).
- Trabalho pesado fora da UI thread + atualização marshalada de volta segue valendo (ver
  *Data Synchronization*): canais/`watch` no egui (`request_repaint`), `Task`/`Subscription`
  no iced, `invoke_from_event_loop` no Slint.

### Build e empacotamento

- Dois caminhos, conforme o quanto de Android você tem:
  - **`cargo-apk`** — empacota um crate `cdylib` direto em APK, gerando manifest e estrutura
    mínimos. Ótimo para protótipos e apps Rust-puro simples (ver *Build Environment → Cargo
    mobile integration*):
    ```bash
    cargo install cargo-apk
    cargo apk run --release       # builda, empacota e instala no device
    ```
  - **Gradle + `.so`** — quando você quer controle total (assinatura, R8, AAB, *tracks* da
    Play Store), trate a UI Rust como um `.so` num projeto Android mínimo e empacote pelo
    fluxo padrão (ver *Gradle Build Integration* e *Package Management*). `cargo-ndk` gera os
    `.so` por ABI; o Gradle empacota.
- **ABIs:** compile ao menos `arm64-v8a` (produção) e `x86_64`/`arm64` (emulador), e garanta
  que a ABI do AVD casa (ver *Cross Compilation Targets* e *Instrumented Testing → emulator*).
- **Distribuição na Play Store** é igual à de qualquer app com `.so`: **AAB**, assinatura,
  `-keep` para qualquer classe JNI residual, símbolos de debug para crashes nativos — tudo em
  *Package Management*.

### Validação no Android (faça cedo)

- Antes de comprometer um produto a uma UI Rust, **prototipe o caminho completo no device**:
  build/empacotamento, carregamento do `.so`, **rotação**, **entrada de texto (IME)**,
  *back button*, ciclo de vida (background/foreground, process death) e desempenho/bateria.
- É exatamente nessas bordas (não no "desenhar um botão") que os toolkits diferem em
  maturidade — descobrir cedo evita retrabalho caro.

## Armadilhas comuns

- **Escolher o toolkit por gosto e descobrir limitação mobile depois** — decida com a tabela e
  *valide o caminho completo no device* antes de investir.
- **Back button / gestos do sistema ignorados** — apps nativos precisam tratar o *back*
  explicitamente; usuários esperam o comportamento Android.
- **IME quebrado** — teste digitação cedo; pode ser o fator decisivo entre toolkits.
- **ABI do `.so` ≠ ABI do device/emulador** — `UnsatisfiedLinkError` (ver *Gradle Build
  Integration*).
- **Lógica de negócio dentro da camada de UI** — perde testabilidade no host; mantenha o
  núcleo puro.
- **Esquecer acessibilidade** quando é requisito — avalie AccessKit/escolha Compose se preciso.

## Assuntos correlatos

- **Baixo nível por trás dos toolkits** (surface, event loop, input): *Rendering Integration*.
- **Os toolkits em si:** *Immediate Mode egui*, *Retained Mode iced*, *Declarative Slint*.
- **Alternativa "app de produto":** *UI Layer Basics* (Compose + núcleo Rust).
- **Empacotar, assinar, publicar:** *Gradle Build Integration* e *Package Management*.
- **Compilar por ABI / emulador:** *Cross Compilation Targets* e *Instrumented Testing*.

## Referências

- android-activity: https://github.com/rust-mobile/android-activity
- cargo-apk: https://github.com/rust-mobile/cargo-apk
- Rust Mobile (organização e ferramentas): https://github.com/rust-mobile
- AccessKit: https://github.com/AccessKit/accesskit
