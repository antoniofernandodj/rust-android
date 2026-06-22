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

## Subtópicos

### Activity lifecycle states

- A Activity passa por callbacks: `onCreate` → `onStart` → `onResume` → (rodando) →
  `onPause` → `onStop` → `onDestroy`. `onRestart` reentra de `onStop` para `onStart`.
- **Mudanças de configuração** (rotação, idioma) destroem e recriam a Activity por padrão;
  use `ViewModel` para reter estado e **não** segurar contexto Rust atrelado à Activity.
- Inicialize a runtime Rust (ex.: `tokio`) preferencialmente no `Application.onCreate`
  (uma vez por processo), não no `Activity.onCreate`.

### Process death scenarios

- O sistema pode **matar o processo** inteiro em background para liberar memória; ao voltar,
  o app é recriado do zero, mas o usuário espera continuar de onde parou.
- Estado volátil em memória nativa **é perdido** — persista o que for importante via
  `onSaveInstanceState`, `SavedStateHandle` ou storage (DataStore/arquivo/SQLite).
- A camada Rust deve ser capaz de **reidratar** seu estado a partir do storage, não assumir
  que sempre houve uma inicialização prévia viva.

### Background service models

- Para trabalho que continua fora da UI:
  - **`WorkManager`** — tarefas adiáveis e garantidas (sync, upload), ideal para a maioria.
  - **Foreground Service** — trabalho contínuo visível ao usuário (player, tracking GPS),
    exige notificação persistente.
  - **Coroutines/threads** dentro do escopo da tela — para trabalho curto.
- Código Rust pesado (cripto, processamento) deve rodar em **thread própria**, nunca na
  main thread, e expor cancelamento cooperativo para respeitar o ciclo do componente.

### Intent communication patterns

- **Intents** são o mecanismo de comunicação entre componentes (Activities, Services,
  Broadcasts). Podem ser **explícitas** (componente nomeado) ou **implícitas** (por ação,
  ex.: `ACTION_VIEW`).
- Dados trafegam via **extras** (`putExtra`) — limitados em tamanho (~1MB no Binder);
  payloads grandes processados em Rust devem ir por arquivo/URI, não por extra.
- `PendingIntent` permite que outro processo (sistema, notificação) dispare uma ação no
  seu app posteriormente.

### Manifest configuration files

- O **`AndroidManifest.xml`** declara componentes, permissões, `minSdk/targetSdk`, e
  features de hardware.
- Pontos relevantes para apps com Rust:
  - `android:extractNativeLibs` — controla se os `.so` são extraídos do APK.
  - Permissões (ex.: `INTERNET`, `CAMERA`) precisam ser declaradas e, se *dangerous*,
    solicitadas em runtime.
  - `<uses-native-library>` para libs do sistema, quando aplicável.

### Resource directory structure

- A pasta `res/` organiza recursos por tipo e *qualifiers* (densidade, idioma, orientação):
  `res/values/`, `res/drawable-*`, `res/layout/`, `res/mipmap-*`, `res/raw/`.
- O sistema seleciona automaticamente a variante correta conforme o dispositivo.
- As **bibliotecas nativas** não ficam em `res/`, e sim em `src/main/jniLibs/<abi>/` (ou
  empacotadas pelo Gradle a partir do output do `cargo-ndk`).

## Referências

- Activity lifecycle: https://developer.android.com/guide/components/activities/activity-lifecycle
- Processes and app lifecycle: https://developer.android.com/guide/components/activities/process-lifecycle
- App resources overview: https://developer.android.com/guide/topics/resources/providing-resources
