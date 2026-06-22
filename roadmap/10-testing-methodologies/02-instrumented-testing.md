# Instrumented Testing

> **Seção:** Testing Methodologies
> **Status:** ✅ preenchido

## Visão geral

**Testes instrumentados** rodam em um dispositivo ou emulador Android real, exercitando o
código no ambiente onde ele de fato vai operar — incluindo a fronteira JNI, o carregamento
do `.so`, permissões, o Keystore e a UI. São mais lentos e frágeis que testes de host, então
cobrem **só o que não dá para validar no host**: integração nativa de verdade, comportamento
da UI, acesso a APIs do sistema e crashes nativos. Usam o runner do AndroidX (Espresso/
Compose Test) e ferramentas de análise nativa.

A pergunta a fazer antes de escrever um teste instrumentado é: *"isto poderia ser um teste de
host?"* (ver *Unit Testing → pirâmide de testes*). Se a resposta é sim, escreva-o no host —
é 100× mais rápido. Reserve o emulador para o que genuinamente depende do Android:

- O `.so` **carrega** nas ABIs empacotadas? (`System.loadLibrary` não falha)
- A travessia Kotlin → JNI/UniFFI → Rust → volta funciona com dados reais?
- O acesso ao **Keystore**, sensores, rede, permissões se comporta como esperado?
- A **UI** reflete o estado do núcleo após uma ação do usuário?
- O `.so` **não crasha** em condições reais (e, se crashar, conseguimos simbolizar)?

## Subtópicos

### Android emulator integration

- O **AVD (Android Virtual Device)** roda o app em CI/local. Para CI, use emuladores
  *headless* (`-no-window`) ou serviços gerenciados (**Gradle Managed Devices**, **Firebase
  Test Lab**) que provisionam e descartam devices de forma reprodutível.
- **A ABI do AVD precisa casar com os `.so` empacotados** (ver *Gradle Build Integration →
  ABI filter specification*): um AVD `x86_64` não carrega um `.so` só-`arm64-v8a` →
  `UnsatisfiedLinkError`. Em hosts x86 use imagens `x86_64`; em Macs ARM, imagens
  `arm64-v8a`.
- Acelere com **KVM** (Linux) / HAXM / hypervisor, e com **snapshots** (abaixo). Sem
  aceleração de hardware, o emulador é proibitivamente lento no CI.

### Test harness execution

- Testes instrumentados ficam em `src/androidTest` e rodam via
  `connectedAndroidTest`/`AndroidJUnitRunner`. Para UI Compose, use `createComposeRule()`;
  para Views clássicas, Espresso.
- Esses testes podem chamar a API nativa **de ponta a ponta** (Kotlin → JNI/UniFFI → Rust →
  volta), validando o pipeline completo que os testes de host não alcançam — incluindo o
  carregamento do `.so`, a inicialização do runtime (ver *Application Lifecycle*) e a
  serialização real dos tipos pela fronteira.

```kotlin
@RunWith(AndroidJUnit4::class)
class RustCoreTest {
    @Test fun saudacao_atravessa_a_fronteira() {
        // exercita o .so de verdade no device
        assertEquals("Olá, Ana!", RustCore.saudacao("Ana"))
    }
}
```

### Native crash analysis

- Crashes no `.so` Rust geram **tombstones** e sinais (SIGSEGV, SIGABRT). Capture-os e
  **simbolize** com os símbolos de debug guardados separadamente antes do `strip` (ver
  *Shared Object Creation → Strip debug symbols*) — sem eles, a stack é só endereços hex.
- Ferramentas: **`ndk-stack`** (resolve stack traces nativas a partir do logcat),
  `addr2line`/`llvm-symbolizer`, e `logcat` para o output bruto.
- Configure um **panic hook** que loga via `liblog` antes de abortar, para ver o panic do
  Rust legível no `logcat`:

```rust
fn instalar_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        // android_logger encaminha para o logcat (tag visível no Studio)
        log::error!("PANIC no núcleo Rust: {info}");
    }));
}
// inicialize android_logger uma vez no JNI_OnLoad / init:
//   android_logger::init_once(Config::default().with_max_level(LevelFilter::Debug));
```

- Combine com `panic = "abort"` + `RUST_BACKTRACE` (onde aplicável) para diagnósticos claros.
  Em produção, suba os **símbolos nativos** para o Play Console (`debugSymbolLevel`) e
  acompanhe os crashes desimbolizados no **Android Vitals**/Crashlytics.

### Performance profiling tests

- Meça desempenho no **device real**, não só em microbenchmarks de host (o ARM real tem
  perfil totalmente diferente do emulador x86 — ver *Performance Optimization → Profiling*):
  - **Macrobenchmark** (Jetpack) — mede *startup time*, *jank* (frames lentos), *scroll
    performance* de cenários reais, com resultados estáveis.
  - **simpleperf** — perfis de CPU do código nativo, com flame graphs, no device.
  - **Baseline Profiles** — geram perfis de AOT que aceleram o cold start (relevante quando o
    `.so` e a init do Rust pesam no startup).
- Acompanhe métricas de **frame (jank)**, **uso de memória nativa** (vazamentos do `.so`; ver
  *Resource Ownership → Memory leak detection*) e **tempo das chamadas FFI**. Estabeleça
  *baselines* e **falhe o CI em regressões** significativas — performance que ninguém mede
  degrada com o tempo.

### Automated UI validation

- **Espresso** (Views) e **Compose Test** (`composeTestRule.onNodeWithText("Count: 1")
  .assertExists()`, `.performClick()`) automatizam interação e asserções na UI que é
  alimentada pelo núcleo Rust.
- Bons para testes de **regressão de ponta a ponta**: ação do usuário → comando ao núcleo
  Rust → estado atualizado → UI refletindo (o fluxo UDF de *UI Layer Basics*). É aqui que se
  valida que toda a cadeia, incluindo a sincronização de estado (ver *Data Synchronization*),
  funciona junta.
- Combine com **screenshot testing** (ex.: Roborazzi, Paparazzi para host, ou screenshots no
  device) para regressão **visual** — pega mudanças de layout que asserções textuais não
  veem.
- Para fluxos complexos e *crash testing* exploratório, o **monkey**/UI Automator e o
  *App Crawler* ajudam a achar caminhos que quebram o `.so`.

### Emulator snapshot management

- **Snapshots** salvam o estado do emulador (já bootado, com app instalado, permissões
  concedidas), cortando o tempo de inicialização nos testes — crucial em CI, onde *bootar* um
  AVD do zero a cada job é caríssimo.
- **Gradle Managed Devices** automatiza criação/descarte de AVDs e snapshots de forma
  reproduzível (você declara o device no `build.gradle` e o Gradle cuida do resto), evitando
  o "AVD configurado na mão" que difere entre máquinas.
- **Limpe o estado entre suítes** (`clearPackageData`) para evitar contaminação entre testes
  — um teste que deixou dados no Keystore/DataStore pode mascarar a falha de outro.

## Armadilhas comuns

- **ABI do AVD ≠ ABI do `.so`** — a causa nº 1 de `UnsatisfiedLinkError` no CI. Garanta que
  empacota a ABI do emulador (`x86_64` em CI x86).
- **Esquecer de subir símbolos** — crashes de produção viram endereços hex inúteis no
  Vitals. Configure `debugSymbolLevel`/arquive os símbolos.
- **Testes instrumentados fazendo trabalho de teste de host** — lentos e frágeis sem
  necessidade. Empurre lógica pura para o host.
- **Emulador sem aceleração no CI** — minutos por teste. Use KVM/snapshots/Managed Devices ou
  Firebase Test Lab.
- **Estado vazando entre testes** — limpe dados do app entre execuções.

## Assuntos correlatos

- **A maioria dos testes deve estar no host:** *Unit Testing* (pirâmide).
- **Símbolos para simbolizar crashes:** *Shared Object Creation → Strip debug symbols*.
- **Profiling no device vs. host:** *Performance Optimization → Profiling*.
- **CI: o que rodar em cada push vs. nightly:** *Unit Testing → Continuous integration*.

## Referências

- Build instrumented tests: https://developer.android.com/training/testing/instrumented-tests
- Testing Compose: https://developer.android.com/jetpack/compose/testing
- ndk-stack: https://developer.android.com/ndk/guides/ndk-stack
- Gradle Managed Devices: https://developer.android.com/studio/test/gradle-managed-devices
- Macrobenchmark: https://developer.android.com/topic/performance/benchmarking/macrobenchmark-overview
