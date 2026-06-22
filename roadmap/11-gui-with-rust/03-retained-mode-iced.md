# Retained Mode UI (iced)

> **Seção:** GUI with Rust
> **Status:** ✅ preenchido

## Visão geral

O **iced** é o principal toolkit de UI *retido* e **declarativo** em Rust, inspirado na **The
Elm Architecture (TEA)** — o mesmo padrão que originou o Redux e influenciou o próprio fluxo
unidirecional do Jetpack Compose. Diferente do egui (modo imediato, ver *Immediate Mode
egui*), no iced você **não redesenha tudo a cada frame**: você descreve o estado e o
framework cuida de produzir e atualizar a árvore de widgets.

O coração do iced são quatro peças que formam um ciclo fechado:

```
        ┌──────────────── Message ───────────────┐
        │  (evento: clique, tecla, resposta async)│
        ▼                                         │
   update(state, msg) ──muta o estado──►  view(state) ──desenha──► UI
        │                                                            │
        └──────────── usuário interage ◄─────────────────────────────┘
```

- **State** — sua struct de estado da aplicação.
- **Message** — um enum com *tudo* que pode acontecer (eventos de UI, respostas de tarefas).
- **update(&mut state, message)** — a única função que **muta** o estado, em reação a uma
  mensagem.
- **view(&state) -> Element<Message>** — função **pura** que descreve a UI a partir do estado;
  os widgets emitem `Message` quando interagidos.

Esse modelo é extremamente previsível (toda mudança passa por `update`, fácil de testar e
depurar) e casa **perfeitamente** com o padrão deste livro: o **núcleo Rust é o State** e a
lógica de negócio é o `update` — a UI vira uma projeção fina (ver *Cross Platform Logic* e
*UI Layer Basics → UDF*). Em troca, é mais "cerimonioso" que o egui para UIs muito dinâmicas e
o suporte mobile do iced é **mais experimental** que o do egui/Slint (avalie maturidade antes
de comprometer um produto).

## Subtópicos

### The Elm Architecture (TEA) na prática

- O ciclo `State → view → Message → update → State` elimina estado espalhado: **não existe
  estado mutável dentro dos widgets**, só no seu `State`, e ele só muda em `update`.

```rust
#[derive(Default)]
struct Contador { valor: i64 }

#[derive(Debug, Clone)]
enum Msg { Incrementar, Decrementar }

impl Contador {
    fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Incrementar => self.valor += 1,
            Msg::Decrementar => self.valor -= 1,
        }
    }
    fn view(&self) -> iced::Element<Msg> {
        use iced::widget::{button, column, text};
        column![
            button("+").on_press(Msg::Incrementar),
            text(self.valor),
            button("-").on_press(Msg::Decrementar),
        ].into()
    }
}
```

- Note que `view` **não executa ações** — o botão só *declara* que, ao ser pressionado, emite
  `Msg::Incrementar`. Quem age é o `update`. Essa separação é o que torna a UI testável: você
  testa `update` chamando-o com mensagens, sem renderizar nada.

### Widgets, layout e composição declarativa

- A UI é uma **árvore de `Element<Message>`** construída por funções/macros: `column![]`,
  `row![]`, `container`, `scrollable`, `text`, `button`, `text_input`, `checkbox`, `slider`,
  `pick_list`, etc.
- Layout é declarativo com `Length::{Fill, Shrink, Fixed}`, `padding`, `spacing`,
  `align_items` — você descreve as regras e o iced calcula posições (não há coordenadas
  manuais).
- **Composição:** uma "tela" ou "componente" é só uma função `fn view(&self) -> Element<Msg>`
  que retorna um sub-trecho da árvore; componha funções como no Compose. Para reutilizar com
  mensagens próprias, mapeia-se o tipo de mensagem com `.map(...)`.
- O iced suporta **temas** (`Theme`, claro/escuro, customizado) e *styling* por widget,
  permitindo aparência consistente.

### Comandos, tarefas assíncronas e a ponte com o núcleo

- Trabalho assíncrono (I/O, chamar o núcleo Rust pesado) **não** roda em `view` nem trava
  `update`: `update` retorna uma **`Task`/`Command`** que o runtime do iced executa fora da
  UI thread e cujo resultado volta como uma nova `Message`:

```rust
fn update(&mut self, msg: Msg) -> iced::Task<Msg> {
    match msg {
        Msg::Carregar => iced::Task::perform(
            self.nucleo.clone().carregar_async(),   // future do núcleo Rust
            Msg::Carregado,                          // resultado vira mensagem
        ),
        Msg::Carregado(dados) => { self.dados = dados; iced::Task::none() }
        _ => iced::Task::none(),
    }
}
```

- Isso integra naturalmente com um **núcleo Rust async** (tokio) e com a fronteira de dados do
  livro: comandos descem, resultados/eventos sobem como mensagens (ver *Data Synchronization →
  Message passing*). O `update` é o ponto onde a UI conversa com o núcleo.
- **`Subscription`** é o mecanismo para fluxos contínuos (timer, eventos do sistema, um
  `Stream` do núcleo): o iced "assina" um stream e cada item vira uma `Message` — o análogo do
  `StateFlow` coletado pelo Compose (ver *Reactive stream patterns*).

### Renderização e backends

- O iced renderiza via **`wgpu`** (Vulkan/GLES, ver *Rendering Integration → WGPU backend*) ou
  um backend `tiny-skia` (CPU) para fallback. O texto usa `cosmic-text`, com bom *shaping*
  para múltiplos idiomas — vantagem sobre o egui em apps multilíngues.
- Por ser retido, só recomputa/redesenha o que muda — eficiente em UIs estáveis, alinhado à
  meta de bateria em mobile.

### Estado mobile do iced (maturidade e limitações)

- O foco histórico do iced é **desktop**; o suporte a **Android** existe (via
  `winit`/`android-activity`) mas é **mais novo e menos batido em produção** que o do
  egui/Slint. Espere arestas em: ciclo de vida da surface, **IME/teclado virtual**, gestos e
  empacotamento.
- Por isso, ao escolher o iced para Android, **valide cedo** o caminho completo (build,
  carregamento, input de texto, rotação) num protótipo antes de comprometer o produto (ver
  *Android Integration & Packaging → Validação*).
- Para UI Android "de produto" com requisitos fortes de integração nativa, Compose + núcleo
  Rust continua mais seguro; o iced brilha quando você quer **uma base de UI única
  multiplataforma** (desktop + mobile) com a previsibilidade do TEA.

## Armadilhas comuns

- **Tentar agir em `view`** — `view` é puro; ações vão para `update` via `Message`/`Task`.
  Misturar quebra o modelo e a testabilidade.
- **Bloquear `update` com trabalho síncrono pesado** — use `Task::perform`/`Subscription`
  para sair da UI thread.
- **`Message` gigante e acoplado** — um enum coeso e por-tela facilita; use `.map()` para
  compor sub-componentes com mensagens próprias.
- **Assumir paridade mobile com desktop** — teste IME, rotação e ciclo de vida no Android
  desde o início; o suporte é mais experimental.
- **Recriar estado de domínio na UI** — o State da UI espelha o núcleo; a fonte da verdade
  segue sendo o núcleo Rust.

## Assuntos correlatos

- **Modo imediato (alternativa):** *Immediate Mode egui* — compare TEA (retido) vs. IMGUI.
- **Fluxo unidirecional no Android:** *UI Layer Basics → State hoisting* (Compose é "TEA-like").
- **Async/mensagens cruzando para o núcleo:** *Data Synchronization*.
- **Baixo nível e empacotamento:** *Rendering Integration* e *Android Integration &
  Packaging*.

## Referências

- iced: https://github.com/iced-rs/iced
- The Elm Architecture: https://guide.elm-lang.org/architecture/
- iced (livro/guia): https://book.iced.rs/
- android-activity (suporte mobile): https://github.com/rust-mobile/android-activity
