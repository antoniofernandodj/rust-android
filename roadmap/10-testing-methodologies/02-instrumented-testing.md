# Instrumented Testing

> **Seção:** Testing Methodologies
> **Status:** ✅ preenchido

## Visão geral

**Testes instrumentados** rodam em um dispositivo ou emulador Android real, exercitando o
código no ambiente onde ele de fato vai operar — incluindo a fronteira JNI, o carregamento
do `.so`, permissões e a UI. São mais lentos e frágeis que testes de host, então cobrem o
que só pode ser validado no Android: integração nativa de verdade, comportamento da UI e
crashes nativos. Usam o runner do AndroidX (Espresso/Compose Test) e ferramentas de análise
nativa.

## Subtópicos

### Android emulator integration

- O **AVD (Android Virtual Device)** roda o app em CI/local. Para CI, use emuladores
  *headless* (`-no-window`) ou serviços gerenciados (Gradle Managed Devices, Firebase Test
  Lab).
- Escolha imagens de sistema que casem com as ABIs compiladas (ex.: `x86_64`), senão o `.so`
  Rust não carrega. Acelere com snapshots e KVM/hardware acceleration.

### Test harness execution

- Testes instrumentados ficam em `src/androidTest` e rodam via
  `connectedAndroidTest`/`AndroidJUnitRunner`. Para UI Compose, use `createComposeRule()`.
- Esses testes podem chamar a API nativa de ponta a ponta (Kotlin → JNI → Rust → volta),
  validando o pipeline completo que os testes de host não alcançam.

### Native crash analysis

- Crashes no `.so` Rust geram **tombstones**/sinais (SIGSEGV, SIGABRT). Capture-os e
  **simbolize** com os símbolos de debug guardados separadamente (ver *Strip debug symbols*).
- Ferramentas: `ndk-stack` para resolver stack traces nativas, `addr2line`/`llvm-symbolizer`,
  e `logcat` para o output. Habilite `panic = "abort"` + backtrace para diagnósticos claros.
- Configure um *panic hook* que loga via `liblog` (`android_logger`) antes de abortar, para
  ver o panic do Rust no `logcat`.

### Performance profiling tests

- Meça desempenho no dispositivo real, não só em microbenchmarks de host:
  **Macrobenchmark** (Jetpack) e **simpleperf** para perfis de CPU do código nativo.
- Acompanhe métricas de frame (jank), uso de memória nativa e tempo das chamadas FFI.
  Estabeleça *baselines* e falhe o CI em regressões significativas.

### Automated UI validation

- **Espresso** (Views) e **Compose Test** (`onNodeWithText(...).performClick()`) automatizam
  interação e asserções na UI que é alimentada pelo núcleo Rust.
- Bons para testes de regressão de ponta a ponta: ação do usuário → comando ao núcleo →
  estado atualizado → UI refletindo. Combine com screenshots para regressão visual.

### Emulator snapshot management

- **Snapshots** salvam o estado do emulador (já bootado, com app instalado), cortando o
  tempo de inicialização nos testes — crucial em CI.
- Gradle Managed Devices automatiza criação/descarte de AVDs e snapshots de forma
  reproduzível. Limpe o estado entre suítes para evitar contaminação entre testes.

## Referências

- Build instrumented tests: https://developer.android.com/training/testing/instrumented-tests
- Testing Compose: https://developer.android.com/jetpack/compose/testing
- ndk-stack: https://developer.android.com/ndk/guides/ndk-stack
