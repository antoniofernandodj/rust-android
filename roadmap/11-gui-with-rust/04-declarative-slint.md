# Declarative UI (Slint)

> **Seção:** GUI with Rust
> **Status:** ✅ preenchido

## Visão geral

O **Slint** é um toolkit de UI declarativo cuja característica distintiva é ter uma
**linguagem de marcação própria** (`.slint`) para descrever a interface — separada do código
Rust, no espírito do QML/Qt Quick ou do XML/Compose. Você descreve *o que* a UI é numa DSL
legível e reativa; o Rust cuida da lógica. É o toolkit Rust que mais se aproxima da
experiência de um framework de UI "tradicional" e tem **suporte oficial e maduro a Android**.

A diferença para egui (imediato) e iced (TEA em Rust puro) está em **onde a UI é descrita**:

| Toolkit | UI descrita em | Estilo | Curva |
|---------|----------------|--------|-------|
| egui | código Rust, por frame | imediato | rápida |
| iced | código Rust, `view()` | retido / TEA | média |
| **Slint** | **arquivo `.slint` (DSL)** | retido / reativo + bindings | DSL nova p/ aprender |

O Slint compila o `.slint` em código Rust (via macro/build script), gerando structs e métodos
tipados que você manipula do Rust. A UI é **reativa por property bindings**: você altera uma
*property* e tudo que depende dela atualiza sozinho — sem `update`/diff manual.

Ponto importante de **licenciamento**: o Slint é dual-licenciado (GPL / Royalty-free para
apps proprietários sob condições / comercial pago). Diferente de egui (MIT/Apache) e iced
(MIT), **verifique a licença** se o app é comercial e fechado — pode exigir a licença
royalty-free ou paga. É um critério de seleção real, não só técnico.

## Subtópicos

### A linguagem `.slint` (markup declarativo)

- A UI é um arquivo `.slint` com uma sintaxe declarativa de elementos aninhados, *properties*,
  *callbacks* e expressões reativas:

```slint
// app.slint
export component JanelaContador inherits Window {
    in-out property <int> valor: 0;     // property: reativa
    callback incrementar();             // callback: chamado a partir da UI, tratado no Rust

    VerticalBox {
        Text { text: "Valor: " + root.valor; }   // re-renderiza quando `valor` muda
        Button {
            text: "Incrementar";
            clicked => { root.incrementar(); }
        }
    }
}
```

- **Property bindings reativos:** `Text { text: "Valor: " + root.valor }` recalcula
  automaticamente quando `valor` muda — esse é o modelo mental central (reatividade
  declarativa, como no Compose/QML), não um loop por frame nem um `update` manual.
- A DSL tem layouts (`VerticalBox`, `HorizontalBox`, `GridBox`), widgets padrão
  (`std-widgets.slint`: `Button`, `LineEdit`, `CheckBox`, `ComboBox`, `ListView`...),
  animações declarativas (`animate`), estados e estilização.

### Integração Rust ↔ `.slint` (gerar e ligar)

- O `.slint` é compilado para Rust por um **build script** (`slint-build` no `build.rs`) ou
  pela macro `slint::include_modules!()`, gerando uma struct tipada (`JanelaContador`) com
  getters/setters de properties e *handlers* de callbacks:

```rust
slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = JanelaContador::new()?;
    let weak = ui.as_weak();                 // Weak para não criar ciclo (ver Resource Ownership)
    ui.on_incrementar(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_valor(ui.get_valor() + 1);     // ou delega ao núcleo Rust
        }
    });
    ui.run()
}
```

- O fluxo é: a **lógica/estado vive no Rust** (idealmente no núcleo, ver *Cross Platform
  Logic*); as **callbacks** do `.slint` chamam o núcleo; e o resultado é empurrado de volta
  via **setters de property**, disparando a reatividade. Padrão idêntico ao do livro: UI fina,
  núcleo como fonte da verdade.
- **`Weak`/upgrade:** o handle da UI é `Arc`-like; capture `as_weak()` nas closures de
  callback para **evitar ciclos de referência** (a UI segura a closure que segura a UI). Ver
  *Resource Ownership → Reference counting*.

### Modelos de dados e listas (`ModelRc`)

- Para listas/coleções, o Slint usa o trait **`Model`** (`ModelRc<T>`, `VecModel<T>`): você
  expõe um modelo de dados que o `ListView`/`for` do `.slint` itera reativamente — alterar o
  modelo atualiza a lista sem recriar a UI.
- É o equivalente do `LazyColumn` + lista observável do Compose (ver *UI Layer Basics →
  Layout component hierarchy*). Para dados vindos do núcleo Rust, mapeie o estado de domínio
  para um `VecModel` e atualize-o quando o núcleo emitir mudanças (ver *Data Synchronization →
  Reactive stream patterns*).

### Threading e atualização a partir de outra thread

- A UI do Slint roda numa **thread de UI** própria; você **não pode** mutar properties
  diretamente de outra thread. Use `slint::invoke_from_event_loop(...)` (ou um `Weak` +
  `upgrade_in_event_loop`) para agendar a atualização na thread de UI:

```rust
let weak = ui.as_weak();
std::thread::spawn(move || {
    let dados = nucleo.processar_pesado();           // fora da UI thread
    let _ = weak.upgrade_in_event_loop(move |ui| {   // volta para a UI thread
        ui.set_resultado(dados.into());
    });
});
```

- Isso espelha exatamente a disciplina do *Data Synchronization*: trabalho pesado fora da UI
  thread, atualização marshalada de volta — só que o Slint dá o primitivo pronto
  (`invoke_from_event_loop`).

### Renderização e backends no Android

- O Slint tem **backend Android oficial** (`slint::android`), construído sobre
  `android-activity`, que cuida de surface, ciclo de vida, **IME/teclado virtual**, toque e
  densidade — as partes que no egui/iced você costuma ajustar à mão (ver *Rendering
  Integration → Input handling / Canvas native bridge*).
- Renderiza via GPU (Skia/`femtovg` sobre GLES/Vulkan) ou software, conforme o backend.
  Texto e i18n são bem suportados.
- Na prática, é o caminho de menor atrito para um **app de UI 100% Rust com cara de app
  Android decente**, justamente por esse backend cuidar da integração nativa.

## Armadilhas comuns

- **Ignorar a licença** — para app comercial fechado, confirme qual licença Slint se aplica
  *antes* de adotar. Pode mudar a decisão.
- **Mutar property de outra thread** — use `invoke_from_event_loop`/`upgrade_in_event_loop`,
  nunca direto.
- **Ciclo de referência UI↔closure** — capture `as_weak()` nas callbacks, não a UI forte.
- **Pôr lógica de negócio no `.slint`** — a DSL é para *apresentação*; regras e estado moram
  no núcleo Rust (testável no host).
- **Recompilar `.slint` "esquecido"** — mudanças na DSL exigem o build script/macro rodar;
  configure o `build.rs` para o Cargo reprocessar ao alterar o arquivo.

## Assuntos correlatos

- **Comparação dos três toolkits e quando usar cada um:** *Android Integration & Packaging →
  Como escolher*.
- **Reatividade/declaratividade no Android:** *UI Layer Basics* (Compose é o análogo Kotlin).
- **`Weak`/ciclos e ownership:** *Resource Ownership*.
- **Marshalar trabalho entre threads:** *Data Synchronization*.

## Referências

- Slint: https://slint.dev/
- Slint + Rust (docs): https://docs.slint.dev/latest/docs/rust/slint/
- Slint Android: https://docs.slint.dev/latest/docs/slint/guide/platforms/android/
- Licenciamento Slint: https://slint.dev/pricing
