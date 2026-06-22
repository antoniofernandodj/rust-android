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

## Subtópicos

### Message passing architectures

- O padrão mais seguro e idiomático em Rust: **não compartilhe memória, comunique por
  mensagens**. Canais (`std::sync::mpsc`, `crossbeam-channel`, `tokio::mpsc`) transferem a
  posse dos dados entre threads.
- A UI envia *comandos* para o núcleo e o núcleo emite *eventos/estado* de volta; cada lado
  tem sua fila. Isso elimina locks na maioria dos casos e modela bem o fluxo unidirecional.

### Shared memory buffer management

- Quando há **muitos dados** (ex.: frames, áudio), copiar via mensagem é caro. Usa-se
  buffer compartilhado (`Arc<Mutex<Vec<u8>>>`, ou `ByteBuffer` direto via JNI).
- Cuidados: alinhar o **tempo de vida** do buffer entre Rust e Java, e proteger o acesso
  concorrente. Em JNI, `NewDirectByteBuffer` evita cópia mas exige que o Rust mantenha a
  memória viva enquanto o Java a usa.

### Event bus implementation

- Um **event bus** desacopla produtores e consumidores: o núcleo publica eventos em um tópico
  e múltiplos assinantes reagem. Em Rust, `tokio::sync::broadcast` ou `watch` são bases boas.
- No lado Kotlin, esses eventos viram um `StateFlow`/`SharedFlow` que o Compose coleta.
  Bom para notificações de mudança de estado sem o núcleo conhecer a UI.

### Reactive stream patterns

- Modele o estado como **stream reativo**: o consumidor observa um fluxo de valores no tempo.
  Rust: `futures::Stream`, `tokio::sync::watch` (último valor) ou bibliotecas reativas.
- Casa naturalmente com Compose, que é declarativo: `watch` expõe "o estado atual" e a UI
  recompõe a cada novo valor. Evita polling e mantém uma única fonte da verdade.

### Atomic pointer usage

- Para flags e contadores compartilhados sem o custo de um `Mutex`, use os **tipos
  atômicos** (`AtomicBool`, `AtomicUsize`, `AtomicPtr`) com a ordering correta
  (`Relaxed`, `Acquire`/`Release`, `SeqCst`).
- `AtomicPtr` permite trocar ponteiros de forma atômica (ex.: hot-swap de configuração),
  mas exige cuidado com a recuperação segura da memória antiga (ver lock-free abaixo).

### Lock free concurrency

- Estruturas **lock-free** (filas, `arc-swap`, `crossbeam` epoch-based reclamation) evitam
  bloqueio e *priority inversion*, importantes para não engasgar a UI thread.
- São difíceis de acertar à mão; prefira crates testados (`crossbeam`, `arc-swap`,
  `dashmap`) em vez de implementar primitivas lock-free próprias.
- Use apenas onde a medição mostrar que o lock é gargalo — na maioria dos casos um
  `Mutex`/canal é suficiente e muito mais simples de manter correto.

## Referências

- Fearless Concurrency (The Rust Book): https://doc.rust-lang.org/book/ch16-00-concurrency.html
- tokio::sync: https://docs.rs/tokio/latest/tokio/sync/index.html
- crossbeam: https://docs.rs/crossbeam/
