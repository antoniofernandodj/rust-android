# Rendering Integration

> **Seção:** GUI with Rust
> **Status:** ✅ preenchido

## Visão geral

Além de usar Rust só para a lógica (com a UI em Compose, ver *UI Layer Basics*), é possível
**renderizar a própria UI em Rust**, útil para jogos, apps gráficos, ferramentas criativas ou
interfaces *custom* (egui, Bevy, Slint, Skia). Nesse modelo o Android fornece apenas uma
superfície (`SurfaceView`/`NativeActivity`) e o Rust desenha diretamente via GPU. Isso exige
integrar o pipeline gráfico, o **event loop** e a propagação de input entre o sistema Android
e o renderer nativo.

A decisão de arquitetura vem primeiro, pois define quanto Android você ainda escreve:

| Modelo | Quem controla a tela | Use quando… |
|--------|----------------------|-------------|
| **Compose + Rust lógica** | Compose (Kotlin) | app "normal"; UI declarativa padrão |
| **`SurfaceView`/`TextureView` em Activity** | híbrido: Rust desenha numa superfície dentro de UI Kotlin | jogo/visualização **embutido** numa tela com chrome Kotlin |
| **`NativeActivity`/`GameActivity`** | Rust (quase) 100% | jogo/app gráfico de tela cheia, sem UI Kotlin |

O modelo híbrido (`SurfaceView`) é o mais comum para apps que têm *uma* tela gráfica pesada
no meio de uma UI normal; `NativeActivity` é o caminho dos jogos puros. O resto do capítulo
descreve as peças comuns aos dois.

## Subtópicos

### WGPU backend implementation

- **`wgpu`** é a abstração gráfica moderna em Rust (implementa a API WebGPU; mapeia para
  **Vulkan** ou **GLES** no Android), segura e multiplataforma. É a base de engines e
  toolkits de UI (egui, Bevy) e o caminho recomendado para gráficos novos em Rust.
- No Android, o `wgpu` cria uma `Surface` a partir do **`ANativeWindow`** obtido da
  `SurfaceView`/`NativeActivity`. A ponte se faz pelo trait `raw-window-handle` (`wgpu`
  consome um `RawWindowHandle`):

```rust
// esboço: criar surface a partir do ANativeWindow do Android
let instance = wgpu::Instance::default();
let surface  = unsafe { instance.create_surface(&janela) }?;       // janela: impl HasWindowHandle
let adapter  = instance.request_adapter(&Default::default()).await.unwrap();
let (device, queue) = adapter.request_device(&Default::default(), None).await?;
surface.configure(&device, &config);   // formato, tamanho, present mode
```

- Prefira **Vulkan** onde disponível; o `wgpu` faz *fallback* para GLES em aparelhos antigos.
  A fragmentação de GPUs/drivers Android é real — teste em hardware variado.

### Skia graphics engine usage

- **Skia** é o motor 2D que o próprio Android e o Chrome usam para desenhar; em Rust é
  acessível via **`skia-safe`**. Excelente para UI 2D vetorial, texto de alta qualidade
  (hinting, shaping) e shapes — onde o `wgpu` puro exigiria reimplementar muito.
- Alternativa ao 3D do `wgpu` quando o foco é **desenho 2D de alta fidelidade**. Pode
  renderizar para uma surface GPU (Ganesh/Graphite, acelerado) ou para um bitmap (CPU).
- Toolkits como **Slint** e **egui** abstraem esse nível: você descreve a UI e eles cuidam do
  backend (wgpu/skia) — bom meio-termo entre "Compose" e "desenhar pixel a pixel".

### Canvas native bridge

- A ponte com o Android se dá por:
  - **`NativeActivity`/`GameActivity`** (app gráfico sem Java de UI; o ciclo de vida chega
    como callbacks nativos) — caminho dos jogos.
  - **`SurfaceView`/`TextureView`** embutida numa Activity Kotlin, passando o handle de
    janela ao Rust via JNI — caminho híbrido. `SurfaceView` tem sua própria superfície de
    composição (melhor performance); `TextureView` integra-se à árvore de views (pode ser
    transformada/animada pelo Compose, ao custo de uma cópia).
- O crate **`ndk`** (e o histórico `ndk-glue`/hoje `android-activity`) expõe o
  `ANativeWindow`, o `AssetManager`, sensores e os **callbacks de ciclo de vida da surface**
  (criada / redimensionada / destruída) que o renderer precisa respeitar.
- **Regra de ouro do ciclo de vida da surface:** a surface pode ser **destruída a qualquer
  momento** (app vai a background, tela desliga). Você **deve** parar de renderizar e
  liberar/recriar os recursos GPU atrelados a ela — desenhar numa surface destruída crasha
  (ver *Application Lifecycle*).

### Event loop synchronization

- O renderer roda em um **event loop** que recebe eventos do sistema e dispara frames. Em
  Rust, **`winit`** (com suporte Android via `android-activity`) é o padrão para abstrair
  isso multiplataforma; em jogos com `GameActivity`, há o loop nativo próprio.
- Sincronize o desenho com o **VSync**/ritmo da surface para evitar *tearing* e desperdício.
  No `wgpu`, o `present_mode` controla isso: `Fifo` (VSync, eficiente em bateria, padrão
  recomendado em mobile) vs. `Mailbox`/`Immediate` (menor latência, mais energia).
- **Renderize sob demanda quando possível:** para uma UI que muda pouco, só redesenhe quando
  há mudança ou quando o sistema pede `redraw`, em vez de um loop a 60fps constante — isso
  economiza bateria drasticamente. Jogos costumam renderizar contínuo; ferramentas/UI, sob
  demanda.
- **Pause o loop** quando a Activity vai a background (surface destruída) e retome no
  `Resumed`/recriação da surface.

### Input handling propagation

- Toques (multi-touch), teclado, gestos e eventos de IME chegam como **`InputEvent`** do
  Android e precisam ser encaminhados ao renderer Rust (via `ndk`/`android-activity`/
  `winit`), convertidos em eventos da sua UI nativa.
- Cuide da **conversão de coordenadas**: eventos vêm em pixels físicos; sua UI pode trabalhar
  em pixels lógicos (dp). Aplique o *scale factor* (densidade da tela) para não ter toque
  "deslocado" em telas de alta densidade.
- Se houver **UI Kotlin sobreposta** (modelo híbrido), defina claramente **quem consome cada
  evento** para evitar conflito — quem está "em cima" intercepta; o `SurfaceView` pode
  precisar repassar (ou não) toques para a hierarquia Compose.
- **IME/teclado virtual** é notoriamente trabalhoso em apps `NativeActivity` (mostrar/
  esconder, receber texto composto) — toolkits como egui/Slint já lidam com isso; fazer à mão
  exige chamadas JNI ao `InputMethodManager`.

### Frame buffer rendering

- O ciclo de frame com `wgpu`: adquirir a próxima imagem da swapchain
  (`surface.get_current_texture()`) → gravar comandos numa *render pass* → submeter à fila
  (`queue.submit`) → apresentar (`frame.present()`):

```rust
let frame = surface.get_current_texture()?;          // pode falhar se a surface mudou
let view  = frame.texture.create_view(&Default::default());
let mut enc = device.create_command_encoder(&Default::default());
{
    let mut rpass = enc.begin_render_pass(&descritor_com_view(&view));
    // ... draw calls ...
}
queue.submit([enc.finish()]);
frame.present();
```

- Gerencie corretamente o **redimensionamento** (reconfigurar a surface com o novo tamanho;
  ignorar isso distorce ou crasha) e a **perda de contexto** (`SurfaceError::Lost`/`Outdated`
  → reconfigurar; `OutOfMemory` → abortar com mensagem).
- Faça o trabalho pesado de render **fora da UI thread** e respeite o *back-pressure* do
  compositor (não enfileire frames mais rápido do que a tela apresenta) para manter framerate
  estável e não inflar a latência de toque.
- Libere recursos GPU (texturas, buffers) via `Drop` quando não precisar mais (ver *Resource
  Ownership → Automatic drop trait execution*) — memória de GPU é escassa em mobile.

## Armadilhas comuns

- **Desenhar após `surfaceDestroyed`** — crash. Pare o loop e libere a surface no callback de
  destruição; recrie ao voltar.
- **Esquecer de reconfigurar no resize/rotação** — imagem esticada ou `SurfaceError`. Trate o
  evento de tamanho.
- **Loop a 60fps numa UI estática** — bateria/calor sem necessidade. Renderize sob demanda
  onde fizer sentido; use `present_mode = Fifo`.
- **Coordenadas sem aplicar densidade** — toque "errado" em telas HiDPI. Multiplique pelo
  scale factor.
- **`unwrap()` em `get_current_texture`** — a surface pode estar `Lost/Outdated` legitimamente
  (rotação, background); trate o erro reconfigurando, não com panic.

## Assuntos correlatos

- **Quando NÃO renderizar em Rust:** para a maioria dos apps, Compose + núcleo Rust (*UI
  Layer Basics*) é mais simples e idiomático. Renderização própria é para gráficos/jogos.
- **Empacotar como app nativo puro:** `cargo-apk`/`NativeActivity` (ver *Build Environment →
  Cargo mobile integration*).
- **Ciclo de vida da surface ↔ ciclo de vida da Activity:** *Application Lifecycle*.
- **Performance do render (alocação por frame, SoA):** *Performance Optimization*.

## Referências

- wgpu: https://wgpu.rs/
- The `ndk` / `android-activity` crates: https://docs.rs/ndk
- winit (Android support): https://github.com/rust-windowing/winit
- egui: https://github.com/emilk/egui
- Slint (Android): https://slint.dev/
