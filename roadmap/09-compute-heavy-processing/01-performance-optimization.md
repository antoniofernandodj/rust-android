# Performance Optimization

> **Seção:** Compute Heavy Processing
> **Status:** ✅ preenchido

## Visão geral

O principal motivo de mover lógica para Rust em Android é **desempenho**: processamento de
imagem/áudio, ML, parsing, criptografia — cargas onde a JVM penaliza com GC e boxing. Esta
etapa cobre as técnicas para extrair máxima velocidade do núcleo Rust: vetorização (SIMD),
paralelismo, uso eficiente de cache e redução de alocações. Regra: **meça primeiro**
(profiling), otimize o gargalo real, e revalide.

## Subtópicos

### SIMD instruction sets

- **SIMD** (Single Instruction, Multiple Data) processa vários elementos por instrução —
  ganho enorme em loops numéricos. Em ARM (Android) o conjunto relevante é **NEON**.
- Opções em Rust: `std::simd` (portátil, nightly), intrinsics de `core::arch::aarch64`
  (NEON), ou deixar o autovetorizador do LLVM agir com `target-feature`/`opt-level=3`.
- Verifique features em runtime com `is_aarch64_feature_detected!` antes de usar caminhos
  especializados.

### Parallel iterator execution

- **Rayon** paraleliza iteradores trivialmente: trocar `.iter()` por `.par_iter()`
  distribui o trabalho por uma thread pool, escalando com os núcleos do celular.

```rust
use rayon::prelude::*;
let soma: u64 = dados.par_iter().map(|x| processa(x)).sum();
```

- Cuidado em mobile: muitas threads competem com a UI e drenam bateria; ajuste o tamanho da
  pool e evite paralelizar trabalho pequeno (overhead supera o ganho).

### Cache locality management

- Acesso sequencial à memória é muito mais rápido que aleatório por causa do **cache da CPU**.
  Prefira **arrays contíguos** (`Vec<T>`, struct-of-arrays) a estruturas com ponteiros
  esparsos (listas ligadas, muitos `Box`).
- Layout *data-oriented* (SoA) melhora a localidade em loops que tocam poucos campos de
  muitos itens. Itere na ordem em que os dados estão na memória.

### Memory alignment strategies

- Alinhamento correto evita acessos *unaligned* (penalidade ou falha em alguns ARMs) e é
  pré-requisito para SIMD. Use `#[repr(C)]`/`#[repr(align(N))]` quando o layout importa.
- Buffers compartilhados via FFI devem respeitar o alinhamento esperado pelos dois lados;
  `bytemuck`/`zerocopy` ajudam a reinterpretar bytes com segurança.

### Inlining hot function paths

- Funções pequenas e muito chamadas se beneficiam de **inlining**, que elimina o custo da
  chamada e abre espaço para mais otimizações. `#[inline]`/`#[inline(always)]` dão a dica;
  **LTO** habilita inlining entre crates.
- Não exagere: inlining excessivo incha o binário e pode piorar o uso de I-cache. Deixe o
  compilador decidir na maioria dos casos e use as anotações nos hot paths comprovados.

### Heap allocation reduction

- Alocação no heap é cara e pressiona o alocador. Estratégias:
  - **Reutilizar buffers** (`Vec::clear` + reuso) em vez de realocar a cada iteração.
  - `SmallVec`/`arrayvec` para coleções pequenas na **stack**.
  - Evitar `clone()` desnecessário; passar `&` / usar `Cow`.
  - `with_capacity` para evitar realocações de crescimento.
- Menos alocação = menos jitter de latência, importante para manter 60/120 fps.

### Profiling (base de tudo)

- Antes de otimizar, **meça**: `criterion` para microbenchmarks, `perf`/`simpleperf`
  (Android) e o profiler do Android Studio para o nativo. Otimize o que o profiler apontar.

## Referências

- The Rust Performance Book: https://nnethercote.github.io/perf-book/
- Rayon: https://github.com/rayon-rs/rayon
- std::simd (portable SIMD): https://doc.rust-lang.org/std/simd/index.html
