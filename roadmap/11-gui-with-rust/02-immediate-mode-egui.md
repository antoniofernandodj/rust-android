# Immediate Mode UI (egui)

> **Seção:** GUI with Rust
> **Status:** ✅ preenchido

## Visão geral

Depois da integração de baixo nível (surface/GPU, ver *Rendering Integration*), o próximo
degrau é usar um **toolkit de UI em Rust** para não desenhar widgets na mão. O **egui** é o
toolkit de *modo imediato* (*immediate mode GUI*, IMGUI) mais popular do ecossistema: leve,
fácil de embutir e excelente para ferramentas, *debug overlays*, editores e apps gráficos
onde a UI muda muito.

A ideia central do **modo imediato** é radicalmente diferente do Compose/Android (que é
*retained*, mantém uma árvore de views). No egui, **a cada frame você redescreve a UI inteira
chamando funções**; não há árvore persistente de widgets, não há `findViewById`, não há
estado de widget guardado pelo framework. Se o botão deve aparecer, você chama `ui.button(...)`
naquele frame; se não, simplesmente não chama.

```rust
// Este código roda TODO frame. A "árvore" é só o fluxo de chamadas.
egui::CentralPanel::default().show(ctx, |ui| {
    ui.heading("Contador");
    ui.label(format!("Valor: {}", self.contador));
    if ui.button("Incrementar").clicked() {
        self.contador += 1;
    }
});
```

Trade-off fundamental: o modo imediato é **simples de raciocinar** (a UI é literalmente o seu
código, sem sincronização de estado entre UI e dados) e ótimo para UIs dinâmicas, mas
**redesenha tudo a cada frame** (custo de CPU/bateria) e tem menos polimento "nativo"
(acessibilidade, integração com IME do sistema) que toolkits *retained*. Para mobile, isso
significa: ótimo para um jogo/ferramenta, menos indicado para um app de produto que precisa
parecer e se comportar como app Android padrão (aí prefira Compose + núcleo Rust, ver *UI
Layer Basics*).

## Subtópicos

### Modo imediato vs. retido (o modelo mental)

- **Imediato (egui):** sem estado de UI persistente; você reconstrói tudo por frame. O estado
  da *aplicação* mora na sua struct; a UI é uma função pura desse estado naquele instante.
- **Retido (Compose, iced, Slint):** o framework mantém uma árvore de elementos e aplica
  *diffs*; você descreve o estado desejado e o framework decide o que mudar.
- Consequência prática no egui: como a UI roda toda hora, **não faça trabalho pesado dentro do
  bloco de UI** (I/O, cálculo). Faça no núcleo Rust, em outra thread/async, e só *leia* o
  resultado no frame (ver *Data Synchronization*).
- O egui é *retained* só onde precisa: posições de janela, scroll e foco são guardados em uma
  *memory* interna por `Id` — mas você raramente mexe nisso.

### eframe, contexto e o loop de frame

- **`eframe`** é o "framework de aplicação" que embrulha o egui com uma janela
  (`winit`) e um backend de render (`wgpu` ou `glow`/GLES). É o ponto de entrada padrão e
  **tem suporte oficial a Android** via `winit`/`android-activity`.
- Você implementa o trait `eframe::App` com um único método central, `update`, chamado a cada
  frame:

```rust
struct MeuApp { nucleo: Arc<Nucleo>, contador: i64 }

impl eframe::App for MeuApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label(format!("estado: {}", self.nucleo.estado_atual()));
            if ui.button("Ação").clicked() {
                self.nucleo.enviar_comando(Comando::Acao); // delega ao núcleo
            }
        });
    }
}
```

- O **`egui::Context`** é o handle central: clonável (é um `Arc` por dentro), thread-safe para
  *pedir* repaint de outra thread (`ctx.request_repaint()`), e carrega entrada, fontes e
  estilo.
- **Repaint sob demanda:** por padrão o eframe só redesenha quando há input. Quando o **núcleo
  Rust** produz um novo estado em background, chame `ctx.request_repaint()` para acordar o
  loop — assim você não gasta bateria redesenhando 60fps à toa (ver *Rendering Integration →
  Event loop synchronization*).

### Layout, widgets e composição

- Layout no egui é por **fluxo** dentro de containers: `ui.horizontal(|ui| ...)`,
  `ui.vertical(...)`, `Grid`, `ScrollArea`, e *panels* (`TopBottomPanel`, `SidePanel`,
  `CentralPanel`, `Window`).
- Widgets retornam uma **`Response`** que carrega a interação daquele frame: `.clicked()`,
  `.hovered()`, `.changed()`, `.dragged()`. Não há callbacks/listeners — você reage no mesmo
  lugar onde declara o widget.
- Para widgets ligados a dados, passa-se uma **referência mutável**:
  `ui.text_edit_singleline(&mut self.nome)`, `ui.checkbox(&mut self.ativo, "Ativo")`. O egui
  escreve direto no seu estado — sem *state hoisting* manual.
- Componha widgets próprios como **funções** que recebem `&mut Ui`: a "componentização" é só
  fatorar funções, não criar classes.

### Estado da aplicação e a ponte com o núcleo Rust

- Como o egui não guarda estado de domínio, **a fonte da verdade é o seu núcleo Rust** — o
  mesmo princípio do resto do livro (ver *Cross Platform Logic*). A struct `App` segura um
  `Arc<Nucleo>` e, a cada frame, *lê* o estado e *envia* comandos.
- Para não bloquear o frame: o núcleo roda trabalho pesado em outra thread/`tokio` e publica
  resultados via canal/`watch`; o `update` só consome o último valor (ver *Data
  Synchronization → Reactive stream patterns*). Ao chegar novidade, `request_repaint()`.
- Persistência: o `eframe` pode serializar o estado da app (feature `persistence`,
  `serde`) entre execuções — útil, mas o estado de domínio sério deve ir para storage do
  núcleo (DataStore/arquivo/SQLite), reidratável após *process death* (ver *Application
  Lifecycle*).

### Renderização: wgpu/glow e fontes

- O egui em si só **gera uma lista de triângulos + texturas** (tessellation); quem desenha é o
  backend. No Android, o `eframe` usa `wgpu` (Vulkan/GLES) ou `glow` (GLES). `wgpu` é a
  escolha moderna (ver *Rendering Integration → WGPU backend*).
- **Fontes e emoji:** o egui embute fontes próprias; para texto não-latino (CJK, árabe) ou
  emoji você precisa **registrar fontes adicionais** (`ctx.set_fonts`) — caso contrário
  aparecem "□". É um ponto de atenção comum em apps multilíngues.
- **DPI/escala:** mobile tem densidade alta; ajuste `ctx.set_pixels_per_point()` ao *scale
  factor* da tela para a UI não ficar minúscula (ver *Rendering Integration → Input handling*).

## Armadilhas comuns

- **Trabalho pesado no `update`** — I/O ou cálculo dentro do bloco de UI trava o frame
  (jank/ANR). Delegue ao núcleo em outra thread; leia só o resultado.
- **Esquecer `request_repaint()`** — quando o núcleo atualiza em background mas o egui está em
  modo sob demanda, a tela "congela" até o próximo toque. Peça repaint ao receber o evento.
- **Texto não-latino/emoji vira "□"** — registre fontes via `set_fonts`.
- **UI minúscula em telas HiDPI** — ajuste `pixels_per_point` ao scale factor.
- **Esperar acessibilidade nativa completa** — o egui tem suporte a *accessibility* (via
  AccessKit) em evolução, mas não equivale ao TalkBack sobre Compose. Avalie se o público
  exige acessibilidade forte.

## Assuntos correlatos

- **Camada de baixo nível** (surface, ciclo de vida, input, IME): *Rendering Integration*.
- **Toolkits retidos** com arquitetura diferente: *Retained Mode (iced)* e *Declarative
  (Slint)*.
- **Empacotar um app egui puro no Android** (winit/android-activity, cargo-apk, distribuição):
  *Android Integration & Packaging*.
- **Núcleo como fonte da verdade e threads:** *Cross Platform Logic* e *Data Synchronization*.

## Referências

- egui: https://github.com/emilk/egui
- eframe (template oficial, com Android): https://github.com/emilk/eframe_template
- egui no Android (winit/android-activity): https://github.com/rust-mobile/android-activity
- AccessKit (acessibilidade): https://github.com/AccessKit/accesskit
