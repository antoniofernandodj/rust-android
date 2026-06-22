# Performance Optimization

> **Seção:** Compute Heavy Processing
> **Status:** ✅ preenchido

## Visão geral

O principal motivo de mover lógica para Rust em Android é **desempenho**: processamento de
imagem/áudio, ML, parsing, compressão, criptografia — cargas onde a JVM penaliza com GC,
boxing e *bounds checks* não elimináveis. Esta etapa cobre as técnicas para extrair máxima
velocidade do núcleo Rust: vetorização (SIMD), paralelismo, uso eficiente de cache e redução
de alocações.

A regra que precede todas as outras: **meça primeiro, otimize o gargalo real, revalide**.
Otimização sem medição é, na melhor das hipóteses, sorte. A hierarquia de impacto, do maior
para o menor, costuma ser:

1. **Algoritmo/estrutura de dados** — trocar O(n²) por O(n log n) supera qualquer SIMD.
2. **Alocação e cache** — evitar alocações no hot path e melhorar localidade rende 2–10×.
3. **Paralelismo** — escalar com os núcleos do device.
4. **SIMD/vetorização** — ganho fino em loops numéricos, depois do resto resolvido.
5. **Micro-otimizações** (inlining manual, etc.) — por último, no que o profiler apontar.

Contexto mobile que muda as prioridades vs. desktop/servidor: **bateria e térmica**. Um
device que esquenta sofre *thermal throttling* e fica mais lento; consumo agressivo de CPU
drena bateria e gera reclamação. Às vezes o "ótimo" em mobile é fazer *menos* trabalho (cache
de resultados, *lazy*) em vez de fazer o mesmo trabalho mais rápido.

## Subtópicos

### SIMD instruction sets

- **SIMD** (Single Instruction, Multiple Data) processa vários elementos por instrução —
  ganho grande em loops numéricos (imagem, áudio, álgebra). Em ARM (Android) o conjunto
  relevante é **NEON**; em x86 (emulador) é SSE/AVX.
- Opções em Rust, da mais portável à mais manual:
  - **Autovetorização do LLVM** — com `opt-level = 3` e `target-feature`, o compilador
    vetoriza loops simples sozinho. Escreva loops "amigáveis" (sem dependências entre
    iterações, sem branches) e deixe o LLVM trabalhar. Comece por aqui.
  - **`std::simd`** (portável, ainda nightly) — tipos SIMD que compilam para NEON/SSE
    conforme o alvo.
  - **Intrinsics** de `core::arch::aarch64` (NEON) — controle total, código específico de
    arquitetura, `unsafe`.
- Em AArch64 (`arm64-v8a`) **NEON é garantido** (ver *Cross Compilation Targets*), então você
  pode usá-lo sem checagem; em `armv7` NEON é opcional — verifique em runtime com
  `is_aarch64_feature_detected!`/`is_arm_feature_detected!` antes de usar caminhos
  especializados, com um *fallback* escalar.

### Parallel iterator execution

- **Rayon** paraleliza iteradores trivialmente: trocar `.iter()` por `.par_iter()`
  distribui o trabalho por uma thread pool (work-stealing), escalando com os núcleos:

```rust
use rayon::prelude::*;
let soma: u64 = dados.par_iter().map(|x| processa(x)).sum();
```

- **Cuidados específicos de mobile:**
  - Celulares têm núcleos **heterogêneos** (big.LITTLE): alguns rápidos, outros econômicos. O
    work-stealing do Rayon lida razoavelmente, mas não espere escala linear com a contagem de
    núcleos.
  - Muitas threads competem com a UI thread e o GC da JVM, e **drenam bateria/esquentam**.
    Ajuste o tamanho da pool (`ThreadPoolBuilder::num_threads`) — às vezes limitar a metade
    dos núcleos dá melhor experiência geral que saturar todos.
  - **Não paralelize trabalho pequeno:** o overhead de distribuição supera o ganho. Há um
    limiar — meça onde paralelizar começa a valer.
- Para I/O concorrente (não CPU-bound), use **async/tokio**, não Rayon — Rayon é para
  CPU-bound (ver *Data Synchronization*).

### Cache locality management

- Acesso sequencial à memória é muito mais rápido que aleatório por causa do **cache da CPU**
  e do *prefetcher*. Um *cache miss* custa centenas de ciclos; um hit, poucos. Prefira
  **arrays contíguos** (`Vec<T>`, slices) a estruturas com ponteiros esparsos (listas
  ligadas, árvores com muitos `Box`, `HashMap` para iteração).
- Layout **data-oriented / SoA** (*struct-of-arrays*) melhora a localidade em loops que tocam
  poucos campos de muitos itens:

```rust
// AoS (array de structs): toca campos não usados → polui cache
struct Particula { pos: [f32;3], vel: [f32;3], cor: [u8;4], id: u64 }
let mundo: Vec<Particula>;

// SoA (struct de arrays): o loop de física só carrega pos e vel
struct Mundo { pos: Vec<[f32;3]>, vel: Vec<[f32;3]>, cor: Vec<[u8;4]>, id: Vec<u64> }
```

- Itere na ordem em que os dados estão na memória (row-major para matrizes: percorra a linha
  interna no loop interno). Inverter a ordem dos loops numa matriz grande pode ser ordens de
  magnitude mais lento só por *cache misses*.

### Memory alignment strategies

- Alinhamento correto evita acessos *unaligned* (penalidade de performance, ou falha em
  alguns ARMs/operações SIMD) e é pré-requisito para várias instruções vetoriais. Use
  `#[repr(C)]`/`#[repr(align(N))]` quando o layout importa.
- Buffers compartilhados via FFI devem respeitar o alinhamento esperado pelos **dois lados**
  (ver *Data Synchronization → Shared memory buffer management*); um `ByteBuffer` que o Java
  alocou pode não ter o alinhamento que seu código SIMD assume.
- `bytemuck`/`zerocopy` ajudam a **reinterpretar bytes com segurança** (ex.: ver um
  `&[u8]` como `&[f32]`) checando alinhamento e tamanho — evitam `transmute` `unsafe` à mão.
- `#[repr(C)]` também é o que torna o layout de uma struct **estável e previsível** para
  cruzar a fronteira FFI (o `repr(Rust)` padrão pode reordenar campos).

### Inlining hot function paths

- Funções pequenas e muito chamadas se beneficiam de **inlining**, que elimina o custo da
  chamada e abre espaço para mais otimizações (constant folding, vetorização através da
  fronteira da função). `#[inline]`/`#[inline(always)]` dão a *dica*; **LTO** habilita
  inlining **entre crates** (sem ele, funções de outra crate raramente são inlinadas).
- Não exagere: inlining excessivo incha o binário (`.so` maior) e pode **piorar o uso de
  I-cache** (código demais não cabe no cache de instruções), deixando *mais* lento. Deixe o
  compilador decidir na maioria dos casos e use as anotações só nos hot paths comprovados
  pelo profiler.
- `#[inline(always)]` é uma ordem, não dica — reserve para funções minúsculas
  (acessadores, wrappers). Para o resto, `#[inline]` (dica) ou nada.

### Heap allocation reduction

- Alocação no heap é cara (sincronização do alocador, possível *syscall*) e pressiona o
  alocador global; em hot paths, é frequentemente o gargalo escondido. Estratégias:
  - **Reutilizar buffers** — `Vec::clear()` + reuso mantém a capacidade alocada, em vez de
    realocar a cada iteração/frame. Padrão essencial em loops de áudio/vídeo.
  - **`SmallVec`/`arrayvec`** — coleções pequenas que ficam na **stack** até certo tamanho,
    sem tocar o heap no caso comum.
  - **Evitar `clone()` desnecessário** — passe `&`/`&mut`, use `Cow<T>` quando às vezes
    precisa possuir e às vezes só emprestar.
  - **`with_capacity(n)`** — pré-aloca quando você sabe o tamanho, evitando as realocações de
    crescimento (que copiam tudo a cada dobro).
  - **Arenas/bump allocators** (`bumpalo`) — para muitos objetos de vida curta com o mesmo
    tempo de vida.
- Menos alocação = menos *jitter* de latência, importante para manter 60/120 fps e para
  áudio sem *glitch*. Em mobile, a previsibilidade da latência muitas vezes importa mais que
  o throughput médio.

### Profiling (a base de tudo)

- Antes de otimizar, **meça** — e meça no **device real**, não só no host (o emulador x86_64
  tem perfil de performance totalmente diferente de um ARM real):
  - **`criterion`** — microbenchmarks estatisticamente robustos no host, ótimo para comparar
    duas implementações de uma função.
  - **`perf`/`simpleperf`** (Android) — perfil de CPU do código nativo no device, com
    *flame graphs*.
  - **Android Studio Profiler** — CPU, memória (nativa inclusive) e energia, no device.
  - **`cargo flamegraph`**, **`samply`** — flame graphs no host para encontrar hot paths.
- Estabeleça **baselines** e detecte **regressões** automaticamente no CI (ver *Instrumented
  Testing → Performance profiling tests*); uma otimização que ninguém mede degrada com o
  tempo.

## Armadilhas comuns

- **Otimizar sem medir** — gastar dias num SIMD que afeta 2% do tempo total. Profile primeiro.
- **Medir no emulador** — perfil de CPU/cache/SIMD irreal vs. ARM. Sempre confirme no device.
- **Esquecer `--release`** — benchmarks em debug são inúteis (sem otimização, com overflow
  checks); meça sempre em release.
- **Paralelizar trabalho minúsculo** — overhead do Rayon > ganho. Há um limiar.
- **Saturar a CPU em mobile** — esquenta, throttla e drena bateria; o "mais rápido" no
  benchmark pode ser pior na experiência real.
- **`#[inline(always)]` em tudo** — incha o `.so` e o I-cache, podendo piorar a performance.

## Assuntos correlatos

- **Tamanho do `.so` vs. velocidade:** o trade-off de `opt-level` está em *Cross Compilation
  Targets → Cargo profiles* e *Shared Object Creation → Size optimization*.
- **Sincronização sem travar a UI:** *Data Synchronization* (atômicos, lock-free, canais).
- **Cripto acelerada por hardware** (AES-NI/NEON): *Cryptographic Implementation*.
- **Medir no device como gate de CI:** *Instrumented Testing → Performance profiling tests*.

## Referências

- The Rust Performance Book: https://nnethercote.github.io/perf-book/
- Rayon: https://github.com/rayon-rs/rayon
- std::simd (portable SIMD): https://doc.rust-lang.org/std/simd/index.html
- criterion: https://github.com/bheisler/criterion.rs
- simpleperf (Android): https://developer.android.com/ndk/guides/simpleperf
