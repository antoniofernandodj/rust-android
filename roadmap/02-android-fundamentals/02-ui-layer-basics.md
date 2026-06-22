# UI Layer Basics

> **Seção:** Android Fundamentals
> **Status:** ✅ preenchido

## Visão geral

Em apps Android modernos, a camada de UI é construída com **Jetpack Compose** — um toolkit
declarativo em Kotlin onde a interface é função do estado (`UI = f(state)`). Quando a lógica
roda em Rust, o papel da UI é fino: **renderizar o estado vindo do núcleo Rust** e
**encaminhar eventos do usuário** de volta para ele via FFI. Entender o básico de Compose é
o suficiente para conectar bem as duas camadas.

A arquitetura recomendada é **unidirecional (UDF — Unidirectional Data Flow)**, que casa
perfeitamente com Rust como fonte da verdade:

```
   ┌─────────── eventos / comandos ──────────┐
   │                                          ▼
[ Compose UI ]                          [ Núcleo Rust ]
   ▲                                          │
   └──────────── estado (StateFlow) ──────────┘
```

A UI nunca guarda lógica de negócio: ela observa um `StateFlow` que reflete o estado do Rust
e envia comandos. Isso mantém o Rust como única fonte da verdade, torna a UI trivial de
trocar (Compose hoje, outra coisa amanhã) e deixa toda a lógica testável fora do Android
(ver *Cross Platform Logic* e *Unit Testing*).

## Subtópicos

### Jetpack Compose overview

- Compose substitui XML + `findViewById` por **funções `@Composable`** que descrevem a UI.
- A UI **recompõe** automaticamente quando o estado observado muda — não se manipula a view
  imperativamente. A recomposição é inteligente: só as partes cujo estado mudou são
  redesenhadas.
- Integração com Rust: o estado exposto pelo Rust (via JNI/UniFFI) é colocado em um
  `State`/`StateFlow` no Kotlin; Compose observa e redesenha.

```kotlin
@Composable
fun Greeting(core: RustCore) {
    // collectAsStateWithLifecycle pausa a coleta quando a tela não está visível
    val msg by core.messageFlow.collectAsStateWithLifecycle()
    Text(text = msg)
}
```

- **Recomposição importa para a performance da ponte FFI:** se cada recomposição chamar o
  Rust de novo, você pode gerar travessias JNI demais. Exponha o estado como um fluxo que o
  Rust *empurra* (push), em vez de a UI *puxar* (pull) a cada frame (ver *Data
  Synchronization → Reactive stream patterns*).

### Layout component hierarchy

- Composables se aninham para formar a árvore de UI. Contêineres básicos:
  - **`Column`** (vertical), **`Row`** (horizontal), **`Box`** (sobreposição em camadas),
    `LazyColumn`/`LazyRow` (listas virtualizadas para muitos itens).
  - `Modifier` ajusta tamanho, padding, clique, etc., de forma encadeada e **ordenada** — a
    ordem dos modifiers altera o resultado (`.padding().background()` ≠
    `.background().padding()`).
- A hierarquia é leve e reconstruída a cada recomposição — mantenha o estado fora dela.
- Para listas alimentadas pelo Rust (ex.: milhares de itens), use `LazyColumn` com `key`
  estável por item, e exponha o estado como uma lista imutável vinda do núcleo, para que o
  Compose compute *diffs* eficientes.

### Material Design implementation

- **Material 3** fornece componentes prontos (`Button`, `Card`, `Scaffold`, `TopAppBar`,
  `NavigationBar`, `FloatingActionButton`) seguindo as diretrizes visuais do Google.
- `Scaffold` estrutura a tela (app bar, conteúdo, barra inferior, FAB, snackbars) e cuida do
  *insets* (barras de sistema, notch).
- Use os componentes Material em vez de recriar do zero — ganham **acessibilidade**
  (TalkBack), suporte a RTL e comportamento consistentes de graça. Reimplementar à mão quase
  sempre regride em acessibilidade.

### Theme configuration settings

- O tema centraliza **cores, tipografia e formas** via `MaterialTheme`.
- **Dynamic color** (Material You, Android 12+) deriva a paleta do papel de parede do
  usuário com `dynamicLightColorScheme(context)`/`dynamicDarkColorScheme(context)`.
- Suporte a **tema claro/escuro** vem de `darkColorScheme()`/`lightColorScheme()`, alternado
  conforme `isSystemInDarkTheme()`.
- Defina o tema uma vez no topo (envolvendo o `setContent { }`) e todos os componentes
  filhos o herdam via `CompositionLocal` (ver abaixo).

### State hoisting principles

- **State hoisting** = mover o estado para o nível mais alto que precisa dele, deixando os
  composables **stateless** (recebem `value` + callback `onValueChange`).
- Torna os componentes reutilizáveis e testáveis, e é exatamente o padrão ideal quando a
  **fonte da verdade é o Rust**: o estado "sobe" até o ViewModel que conversa com o núcleo,
  e desce como parâmetro imutável para os composables.

```kotlin
// Stateless: reutilizável, testável, não conhece o Rust
@Composable
fun Counter(count: Int, onIncrement: () -> Unit) {
    Button(onClick = onIncrement) { Text("Count: $count") }
}

// O ViewModel é a ponte com o núcleo Rust
class CounterViewModel(private val core: RustCore) : ViewModel() {
    val count = core.countFlow.stateIn(viewModelScope, SharingStarted.Eagerly, 0)
    fun increment() = viewModelScope.launch { core.increment() }
}
```

- **`remember` vs. estado do Rust:** use `remember`/`rememberSaveable` só para estado
  puramente de UI (ex.: se um menu está aberto). Estado de domínio mora no Rust e desce pelo
  fluxo — nunca duplique a fonte da verdade.

### Composition local usage

- **`CompositionLocal`** propaga valores implicitamente pela árvore (tema, densidade,
  `Context`) sem passá-los como parâmetro em cada função.
- Útil para injetar dependências amplamente usadas (ex.: um handle para o núcleo Rust)
  sem poluir a assinatura de todos os composables:

```kotlin
val LocalRustCore = staticCompositionLocalOf<RustCore> { error("RustCore não provido") }

// no topo:
CompositionLocalProvider(LocalRustCore provides rustCore) { App() }
```

- Use com parcimônia: dependências explícitas continuam preferíveis para a maioria dos
  casos, pois deixam o fluxo de dados visível. `CompositionLocal` é para o que é
  genuinamente "ambiental" (tema, locale, o handle global do núcleo).

## Armadilhas comuns

- **Chamar FFI dentro de um `@Composable`** — composables podem recompor muitas vezes por
  segundo; uma chamada JNI/UniFFI direta no corpo gera travessias e trabalho repetido. Mova
  para o ViewModel/`LaunchedEffect`.
- **Bloquear a main thread** — `runBlocking` ou uma chamada Rust síncrona e pesada na UI
  thread congela a interface (ANR). Trabalho pesado vai para corotina + thread do Rust (ver
  *Application Lifecycle → Background service models*).
- **Estado duplicado** — manter uma cópia do estado no Compose *e* no Rust leva a
  dessincronização. Uma fonte da verdade só (o Rust), espelhada por um fluxo.
- **Esquecer o ciclo de vida na coleta** — `collectAsState()` simples coleta mesmo em
  background; prefira `collectAsStateWithLifecycle()` para pausar e poupar bateria.

## Assuntos correlatos

- **`StateFlow`/`SharedFlow`** são a cola entre eventos do Rust e o Compose; o desenho do
  fluxo (último valor vs. stream de eventos) é detalhado em *Data Synchronization*.
- **Renderização própria em Rust:** quando a UI inteira é desenhada pelo Rust (jogos,
  gráficos), o Compose sai de cena e entra a integração de surface/GPU — ver *Rendering
  Integration*.
- **Navigation Compose** gerencia a pilha de telas; cada tela continua observando o mesmo
  núcleo Rust, que não conhece a navegação — ela é puro detalhe de UI.

## Referências

- Jetpack Compose: https://developer.android.com/jetpack/compose
- State and Jetpack Compose: https://developer.android.com/jetpack/compose/state
- Architecting your Compose UI (UDF): https://developer.android.com/jetpack/compose/architecture
- Material 3: https://m3.material.io/
