# Data Synchronization

> **Seção:** State Management
> **Status:** ✅ preenchido

## Visão geral

Num app Android com núcleo Rust há **duas threads/mundos** que precisam compartilhar estado:
a UI thread (Kotlin/Compose) e as threads de trabalho do Rust. **Sincronização de dados**
trata de como passar mudanças entre elas de forma segura — sem *data races*, sem travar a
UI e sem corromper estado. Rust dá garantias fortes em tempo de compilação (`Send`/`Sync`),
mas a fronteira FFI exige padrões explícitos: passagem de mensagens, buffers compartilhados,
event bus e estruturas atômicas/lock-free para os pontos quentes.

Vale fixar dois conceitos que sustentam tudo o que vem depois:

- **`Send`** — um tipo pode ser **movido** para outra thread. **`Sync`** — um tipo pode ser
  **compartilhado** (`&T`) entre threads. O compilador Rust recusa, em tempo de compilação,
  compartilhar o que não é seguro — é a "fearless concurrency". O que atravessa a fronteira
  FFi e é guardado para outra thread (ex.: estado num `Object` UniFFI acessado de várias
  corrotinas) precisa ser `Send + Sync`, tipicamente `Arc<...>` com sincronização interna.
- **A UI thread é sagrada.** Bloqueá-la por mais de alguns milissegundos gera *jank*; por ~5s
  gera ANR (App Not Responding) e o sistema oferece matar o app. Toda operação Rust não
  trivial deve rodar fora dela e devolver o resultado de forma assíncrona.

A escolha do mecanismo segue a natureza do dado, e há um trade-off central — **copiar
(mensagens) vs. compartilhar (memória)**:

| Padrão | Bom para | Custo |
|--------|----------|-------|
| Mensagens (canais) | comandos, eventos, estado pequeno | cópia/move; simples e seguro |
| Buffer compartilhado | frames, áudio, blobs grandes | sincronização + tempo de vida |
| Atômicos / lock-free | flags, contadores, hot paths | difícil de acertar; meça antes |

## Subtópicos

### Message passing architectures

- O padrão mais seguro e idiomático em Rust: **não compartilhe memória, comunique por
  mensagens**. Canais (`std::sync::mpsc`, `crossbeam-channel`, `tokio::mpsc`) transferem a
  posse dos dados entre threads, eliminando *data races* por construção.
- A UI envia *comandos* para o núcleo e o núcleo emite *eventos/estado* de volta; cada lado
  tem sua fila. Isso modela bem o fluxo unidirecional (UDF) descrito em *UI Layer Basics*:

```rust
enum Comando { Incrementar, Resetar, Carregar(String) }
enum Evento  { EstadoMudou(EstadoUi), Erro(String) }

// thread/worker do núcleo
fn worker(cmds: Receiver<Comando>, eventos: Sender<Evento>) {
    let mut estado = Estado::default();
    while let Ok(cmd) = cmds.recv() {
        match cmd {
            Comando::Incrementar => estado.n += 1,
            Comando::Resetar     => estado = Estado::default(),
            Comando::Carregar(p) => { /* I/O */ }
        }
        let _ = eventos.send(Evento::EstadoMudou(estado.to_ui()));
    }
}
```

- **Bounded vs. unbounded:** canais ilimitados podem crescer sem fim se o produtor for mais
  rápido que o consumidor (memória estoura). Prefira canais **bounded** para aplicar
  *back-pressure* — o produtor espera quando a fila enche, regulando o ritmo.
- **Sentido FFI:** os comandos chegam de chamadas Kotlin→Rust; os eventos voltam via
  callback interface (UniFFI) ou um `StateFlow` alimentado por polling de um canal.

### Shared memory buffer management

- Quando há **muitos dados** (ex.: frames de câmera/vídeo, áudio PCM), copiar via mensagem é
  caro. Usa-se buffer compartilhado (`Arc<Mutex<Vec<u8>>>`, um *ring buffer*, ou um
  `ByteBuffer` direto via JNI).
- Cuidados: alinhar o **tempo de vida** do buffer entre Rust e Java, e proteger o acesso
  concorrente. Em JNI, `NewDirectByteBuffer` evita cópia — o Java enxerga a mesma memória
  que o Rust alocou — mas exige que o Rust **mantenha a memória viva** enquanto o Java a usa
  (não dropar o `Vec` cujo ponteiro foi entregue!) (ver *Java Native Interface → Native
  memory allocation*).
- Padrões para evitar cópia *e* corrida ao mesmo tempo:
  - **Double buffering / swap:** um buffer sendo escrito pelo Rust, outro sendo lido pela UI;
    troca atômica de ponteiro entre eles (ver *Atomic pointer usage* / `arc-swap`).
  - **Ring buffer SPSC** (single-producer single-consumer) para áudio em tempo real, sem
    lock no caminho quente.
- Sempre respeite o **alinhamento** esperado pelos dois lados (ver *Performance Optimization
  → Memory alignment strategies*); `bytemuck`/`zerocopy` ajudam a reinterpretar bytes com
  segurança.

### Event bus implementation

- Um **event bus** desacopla produtores e consumidores: o núcleo publica eventos em um tópico
  e múltiplos assinantes reagem. Em Rust, `tokio::sync::broadcast` (vários consumidores, cada
  um recebe tudo) ou `watch` (um valor "atual" observável) são bases boas.
- No lado Kotlin, esses eventos viram um `StateFlow`/`SharedFlow` que o Compose coleta. Bom
  para notificações de mudança de estado **sem o núcleo conhecer a UI** — o Rust só publica
  "algo mudou"; quem se importa, escuta.
- Com UniFFI, o sentido Rust→Kotlin se faz com uma **callback interface**: o Kotlin
  registra um observador, e o Rust o invoca a cada evento, alimentando o `MutableStateFlow`
  do ViewModel (ver *Automated Bindings → Type translation rules*).
- Cuidado com `broadcast` lento: se um assinante não consome a tempo, ele "perde" mensagens
  (lagged). Defina a política (perder os mais antigos vs. aplicar back-pressure) de propósito.

### Reactive stream patterns

- Modele o estado como **stream reativo**: o consumidor observa um fluxo de valores no tempo.
  Rust: `futures::Stream`, `tokio::sync::watch` (mantém só o último valor — ideal para
  "estado atual") ou bibliotecas reativas.
- Casa naturalmente com Compose, que é declarativo: `watch` expõe "o estado atual" e a UI
  recompõe a cada novo valor. Evita *polling* e mantém uma **única fonte da verdade** (o
  Rust).
- **Push vs. pull:** prefira o Rust **empurrar** mudanças (push, via watch/callback) a a UI
  **puxar** a cada frame (pull, chamando FFI na recomposição) — pull gera travessias JNI
  demais e desperdício (ver *UI Layer Basics → Armadilhas comuns*).
- No Kotlin, exponha como `StateFlow` e colete com `collectAsStateWithLifecycle()` para
  pausar quando a tela não está visível (economia de bateria; ver *Application Lifecycle*).

### Atomic pointer usage

- Para flags e contadores compartilhados sem o custo de um `Mutex`, use os **tipos
  atômicos** (`AtomicBool`, `AtomicUsize`, `AtomicPtr`) com a *ordering* correta:
  - `Relaxed` — só atomicidade, sem garantia de ordem (bom para contadores/estatísticas).
  - `Acquire`/`Release` — par usado para "publicar" dados: o consumidor faz `Acquire` e
    enxerga tudo que o produtor escreveu antes do `Release`.
  - `SeqCst` — ordem total global; mais simples de raciocinar, mais caro. Use quando em
    dúvida e otimize depois.
- Exemplo clássico em mobile — **flag de cancelamento cooperativo** (ver *Application
  Lifecycle → Background service models*):

```rust
let cancelar = Arc::new(AtomicBool::new(false));
// worker:
while !cancelar.load(Ordering::Relaxed) { /* processa um pedaço */ }
// UI (ao sair da tela): cancelar.store(true, Ordering::Relaxed);
```

- `AtomicPtr` permite trocar ponteiros de forma atômica (ex.: *hot-swap* de configuração),
  mas exige cuidado com a **recuperação segura da memória antiga** (quando é seguro liberar o
  ponteiro substituído?) — daí preferir `arc-swap` (abaixo).

### Lock free concurrency

- Estruturas **lock-free** (filas, `arc-swap`, `crossbeam` com *epoch-based reclamation*)
  evitam bloqueio e *priority inversion* — importantes para não engasgar a UI thread nem o
  thread de áudio em tempo real, onde um lock segurado por outra thread pode causar
  *glitches*.
- São difíceis de acertar à mão (ABA problem, ordering sutil, reclamação de memória).
  **Prefira crates testados**: `crossbeam` (filas, `ArrayQueue`), `arc-swap` (trocar um
  `Arc` atomicamente — perfeito para config/estado imutável compartilhado), `dashmap` (mapa
  concorrente).
- **Regra de ouro:** use lock-free apenas onde a **medição** mostrar que o lock é gargalo. Na
  esmagadora maioria dos casos um `Mutex`/`RwLock`/canal é suficiente e muito mais simples de
  manter correto. "Premature lock-free is the root of much evil."
- Para validar código concorrente, use **`loom`** nos testes — ele explora exaustivamente os
  *interleavings* possíveis de threads (ver *Unit Testing → Deterministic test scenarios*).

## Armadilhas comuns

- **Bloquear a UI thread** esperando o Rust — `runBlocking` na main thread = ANR. Use
  corrotina + canal/callback.
- **Buffer direto liberado cedo** — entregar um `NewDirectByteBuffer` e dropar o `Vec` do
  Rust enquanto o Java ainda lê → use-after-free / crash nativo.
- **Canal unbounded crescendo sem fim** — produtor mais rápido que o consumidor estoura
  memória. Use bounded + back-pressure.
- **Deadlock por ordem de locks** — pegar dois `Mutex` em ordens diferentes em threads
  diferentes trava o app. Padronize a ordem, ou prefira mensagens (sem locks).
- **`ordering` Relaxed onde precisava Acquire/Release** — publicar um ponteiro com `Relaxed`
  pode deixar o consumidor ver dados não inicializados. Em dúvida, `SeqCst`.

## Assuntos correlatos

- **Posse e tempo de vida dos dados que cruzam threads/FFI:** *Resource Ownership*
  (`Arc`/`Weak`, handles).
- **Como a UI consome o estado:** *UI Layer Basics → State hoisting / Reactive stream*.
- **Paralelismo de trabalho pesado (não sincronização, mas relacionado):** *Performance
  Optimization → Parallel iterator execution* (Rayon).
- **`async` cruzando a fronteira:** UniFFI mapeia `async fn`↔`suspend fun` — *Automated
  Bindings*.

## Referências

- Fearless Concurrency (The Rust Book): https://doc.rust-lang.org/book/ch16-00-concurrency.html
- tokio::sync: https://docs.rs/tokio/latest/tokio/sync/index.html
- crossbeam: https://docs.rs/crossbeam/
- arc-swap: https://docs.rs/arc-swap/
- loom: https://github.com/tokio-rs/loom
