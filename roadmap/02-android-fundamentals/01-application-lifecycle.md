# Application Lifecycle

> **Seção:** Android Fundamentals
> **Status:** ✅ preenchido

## Visão geral

O Android controla agressivamente o **ciclo de vida** dos componentes do app: Activities
são criadas, pausadas, paradas e destruídas conforme o usuário navega e conforme o sistema
precisa de memória. Quem escreve a camada Rust precisa entender esse ciclo porque o estado
nativo (threads, ponteiros, runtimes async) **sobrevive de forma diferente** do estado da
UI Java/Kotlin — e ignorar isso causa vazamentos, crashes em rotação de tela e perda de
dados em *process death*.

A regra mental mais útil é separar **três escopos de tempo de vida** distintos no app:

1. **Escopo de processo** — vive enquanto o processo Linux do app existir. É aqui que mora o
   estado nativo do Rust (runtime async, conexões, caches). Inicializado uma vez em
   `Application.onCreate`.
2. **Escopo de tela/Activity** — recriado a cada mudança de configuração (rotação, tema,
   idioma). Frágil; *nunca* ancore recursos nativos de longa duração aqui.
3. **Escopo persistente** — sobrevive até à *process death*. Só o que está em disco
   (DataStore, SQLite, arquivos) ou salvo via `onSaveInstanceState`.

O erro arquitetural mais comum em apps com Rust é confundir (1) e (2): inicializar o runtime
Rust no `Activity.onCreate` faz com que cada rotação de tela crie um novo runtime e vaze o
anterior. O estado nativo pertence ao processo, não à tela.

## Subtópicos

### Activity lifecycle states

- A Activity passa por callbacks: `onCreate` → `onStart` → `onResume` → (rodando) →
  `onPause` → `onStop` → `onDestroy`. `onRestart` reentra de `onStop` para `onStart`.
- **Mudanças de configuração** (rotação, idioma, dark mode) destroem e recriam a Activity
  por padrão; use `ViewModel` para reter estado e **não** segure contexto Rust atrelado à
  Activity.
- Inicialize a runtime Rust (ex.: `tokio`) preferencialmente no `Application.onCreate`
  (uma vez por processo), não no `Activity.onCreate`:

```kotlin
class MeuApp : Application() {
    override fun onCreate() {
        super.onCreate()
        System.loadLibrary("core")   // carrega o .so uma vez
        RustCore.init(filesDir.path) // inicializa runtime/estado nativo no escopo de processo
    }
}
```

- Do lado Rust, a inicialização deve ser **idempotente e thread-safe** — use `OnceCell`/
  `OnceLock` para garantir que o runtime seja criado uma única vez mesmo se `init` for
  chamado mais de uma vez:

```rust
use std::sync::OnceLock;
use tokio::runtime::Runtime;

static RT: OnceLock<Runtime> = OnceLock::new();

pub fn runtime() -> &'static Runtime {
    RT.get_or_init(|| Runtime::new().expect("falha ao criar runtime tokio"))
}
```

### Process death scenarios

- O sistema pode **matar o processo** inteiro em background para liberar memória; ao voltar,
  o app é recriado do zero, mas o usuário espera continuar de onde parou.
- Estado volátil em memória nativa **é perdido** — persista o que for importante via
  `onSaveInstanceState`, `SavedStateHandle` ou storage (DataStore/arquivo/SQLite).
- A camada Rust deve ser capaz de **reidratar** seu estado a partir do storage, não assumir
  que sempre houve uma inicialização prévia viva. Projete o núcleo para um modelo
  "carregar-do-disco-ou-criar-default" em vez de depender de estado em RAM.
- **Como reproduzir/testar:** o botão "Don't keep activities" nas opções de desenvolvedor, e
  `adb shell am kill <pacote>`, forçam o cenário. Testar isso cedo evita bugs raros e
  difíceis de reproduzir em produção.
- Há um limite de tamanho para `onSaveInstanceState` (trafega pelo Binder, ~1MB no total do
  processo) — salve **identificadores e estado mínimo**, não blobs grandes; estes vão para
  storage e são recarregados pelo Rust.

### Background service models

- Para trabalho que continua fora da UI:
  - **`WorkManager`** — tarefas adiáveis e garantidas (sync, upload, processamento
    posterior), ideal para a maioria. Sobrevive a reboots e respeita restrições (rede,
    bateria).
  - **Foreground Service** — trabalho contínuo visível ao usuário (player, tracking GPS,
    gravação), exige notificação persistente e, no Android moderno, declaração de
    `foregroundServiceType`.
  - **Coroutines/threads** dentro do escopo da tela — para trabalho curto que não precisa
    sobreviver à navegação.
- Código Rust pesado (cripto, processamento de imagem/áudio) deve rodar em **thread própria
  ou no runtime async**, nunca na main thread, e expor **cancelamento cooperativo** (um
  `CancellationToken`/flag atômica) para respeitar o ciclo do componente — quando o
  `WorkManager` cancela o job, o Rust precisa parar de verdade (ver *Data Synchronization*).
- Atenção aos **limites de execução em background** do Android moderno: apps em background
  têm restrições crescentes; trabalho longo precisa de Foreground Service ou WorkManager,
  não de uma thread solta que o sistema pode congelar.

### Intent communication patterns

- **Intents** são o mecanismo de comunicação entre componentes (Activities, Services,
  Broadcasts). Podem ser **explícitas** (componente nomeado) ou **implícitas** (por ação,
  ex.: `ACTION_VIEW`, `ACTION_SEND`).
- Dados trafegam via **extras** (`putExtra`) — limitados em tamanho pelo Binder
  (`TransactionTooLargeException` ao estourar ~1MB *somando todas as transações em voo*);
  payloads grandes processados em Rust devem ir por **arquivo/URI** (com `FileProvider`),
  não por extra.
- `PendingIntent` permite que outro processo (sistema, notificação, alarme) dispare uma ação
  no seu app posteriormente. Lembre da flag `FLAG_IMMUTABLE`/`FLAG_MUTABLE` obrigatória nas
  versões recentes.
- Para o núcleo Rust, o padrão saudável é a UI traduzir Intents em **comandos de alto
  nível** para o núcleo, em vez de passar o `Intent` cru pela fronteira FFI.

### Manifest configuration files

- O **`AndroidManifest.xml`** declara componentes, permissões, `minSdk/targetSdk`, e
  features de hardware.
- Pontos relevantes para apps com Rust:
  - `android:extractNativeLibs` — controla se os `.so` são extraídos do APK para o disco no
    install. Com `false` (padrão moderno + `useLegacyPackaging false`), os `.so` ficam
    **comprimidos e alinhados dentro do APK** e são mapeados direto — instala menor e mais
    rápido, mas exige `.so` alinhados a página (ver *Cross Compilation Targets → 16 KB page
    size*).
  - Permissões (ex.: `INTERNET`, `CAMERA`) precisam ser declaradas e, se *dangerous*,
    solicitadas em runtime.
  - `<uses-native-library>` para depender de libs do sistema específicas, quando aplicável.
- `minSdkVersion` aqui deve casar com o nível de API embutido no linker do NDK (ver
  *Cross Compilation Targets*); um descasamento silencioso vira crash em devices antigos.

### Resource directory structure

- A pasta `res/` organiza recursos por tipo e *qualifiers* (densidade, idioma, orientação):
  `res/values/`, `res/drawable-*`, `res/layout/`, `res/mipmap-*`, `res/raw/`.
- O sistema seleciona automaticamente a variante correta conforme o dispositivo (densidade,
  locale, modo claro/escuro).
- As **bibliotecas nativas** não ficam em `res/`, e sim em `src/main/jniLibs/<abi>/` (ou
  empacotadas pelo Gradle a partir do output do `cargo-ndk`; ver *Gradle Build
  Integration*).
- `res/raw/` é útil para empacotar **assets que o Rust vai ler** (modelos de ML, dados
  embutidos); acesse-os via `AssetManager` (do crate `ndk`) ou copie para `filesDir` no
  primeiro uso.

## Armadilhas comuns

- **Runtime tokio recriado por rotação** — sintoma: vazamento crescente de threads e memória
  a cada giro de tela. Causa: `init` no `Activity.onCreate`. Solução: escopo de processo +
  `OnceLock`.
- **Assumir que o processo nunca morre** — apps testados só em foreground quebram quando o
  usuário volta após o sistema matar o processo. Sempre projete para reidratação.
- **`GlobalRef` JNI presa a uma Activity** — guardar um callback da Activity como global ref
  no Rust impede o GC de coletar a Activity destruída → vazamento de toda a árvore de views.
  Use `Weak`/limpe no `onDestroy` (ver *Java Native Interface → Global reference
  lifecycle*).
- **Trabalho em background sem cancelamento** — o componente morre, mas a thread Rust
  continua segurando recursos. Sempre exponha cancelamento cooperativo.

## Assuntos correlatos

- **`ViewModel` + `SavedStateHandle`** são a ponte idiomática entre o escopo de tela e o
  estado: o ViewModel sobrevive à rotação; o `SavedStateHandle` sobrevive à process death.
- **Lifecycle-aware components** (`LifecycleObserver`, `repeatOnLifecycle`) garantem que a
  coleta de fluxos vindos do Rust pare quando a tela não está visível, economizando bateria
  (ver *Data Synchronization → Reactive stream patterns*).
- **Inicialização cara:** se a init do Rust for pesada, considere `App Startup` (Jetpack) ou
  inicialização preguiçosa para não atrasar o cold start.

## Referências

- Activity lifecycle: https://developer.android.com/guide/components/activities/activity-lifecycle
- Processes and app lifecycle: https://developer.android.com/guide/components/activities/process-lifecycle
- Save UI states: https://developer.android.com/topic/libraries/architecture/saving-states
- WorkManager: https://developer.android.com/develop/background-work/background-tasks/persistent
- App resources overview: https://developer.android.com/guide/topics/resources/providing-resources
