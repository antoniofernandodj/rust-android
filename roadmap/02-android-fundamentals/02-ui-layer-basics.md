# UI Layer Basics

> **Seção:** Android Fundamentals
> **Status:** ✅ preenchido

## Visão geral

Em apps Android modernos, a camada de UI é construída com **Jetpack Compose** — um toolkit
declarativo em Kotlin onde a interface é função do estado (`UI = f(state)`). Quando a lógica
roda em Rust, o papel da UI é fino: **renderizar o estado vindo do núcleo Rust** e
**encaminhar eventos do usuário** de volta para ele via FFI. Entender o básico de Compose é
o suficiente para conectar bem as duas camadas.

## Subtópicos

### Jetpack Compose overview

- Compose substitui XML + `findViewById` por **funções `@Composable`** que descrevem a UI.
- A UI **recompõe** automaticamente quando o estado observado muda — não se manipula a view
  imperativamente.
- Integração com Rust: o estado exposto pelo Rust (via JNI/UniFFI) é colocado em um
  `State`/`StateFlow` no Kotlin; Compose observa e redesenha.

```kotlin
@Composable
fun Greeting(core: RustCore) {
    val msg by core.messageFlow.collectAsState()
    Text(text = msg)
}
```

### Layout component hierarchy

- Composables se aninham para formar a árvore de UI. Contêineres básicos:
  - **`Column`** (vertical), **`Row`** (horizontal), **`Box`** (sobreposição).
  - `Modifier` ajusta tamanho, padding, clique, etc., de forma encadeada.
- A hierarquia é leve e reconstruída a cada recomposição — mantenha o estado fora dela.

### Material Design implementation

- **Material 3** fornece componentes prontos (`Button`, `Card`, `Scaffold`, `TopAppBar`,
  `NavigationBar`) seguindo as diretrizes visuais do Google.
- `Scaffold` estrutura a tela (app bar, conteúdo, barra inferior, FAB).
- Use os componentes Material em vez de recriar do zero — ganham acessibilidade e
  comportamento consistentes de graça.

### Theme configuration settings

- O tema centraliza **cores, tipografia e formas** via `MaterialTheme`.
- **Dynamic color** (Material You, Android 12+) deriva a paleta do papel de parede.
- Suporte a **tema claro/escuro** vem de `darkColorScheme()`/`lightColorScheme()`.
- Defina o tema uma vez no topo e todos os componentes filhos o herdam.

### State hoisting principles

- **State hoisting** = mover o estado para o nível mais alto que precisa dele, deixando os
  composables **stateless** (recebem `value` + callback `onValueChange`).
- Torna os componentes reutilizáveis e testáveis, e é exatamente o padrão ideal quando a
  **fonte da verdade é o Rust**: o estado "sobe" até o ViewModel que conversa com o núcleo.

```kotlin
@Composable
fun Counter(count: Int, onIncrement: () -> Unit) {
    Button(onClick = onIncrement) { Text("Count: $count") }
}
```

### Composition local usage

- **`CompositionLocal`** propaga valores implicitamente pela árvore (tema, densidade,
  `Context`) sem passá-los como parâmetro em cada função.
- Útil para injetar dependências amplamente usadas (ex.: um handle para o núcleo Rust)
  sem poluir a assinatura de todos os composables.
- Use com parcimônia: dependências explícitas continuam preferíveis para a maioria dos
  casos, pois deixam o fluxo de dados visível.

## Referências

- Jetpack Compose: https://developer.android.com/jetpack/compose
- State and Jetpack Compose: https://developer.android.com/jetpack/compose/state
- Material 3: https://m3.material.io/
