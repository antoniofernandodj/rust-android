# Shared Object Creation

> **Seção:** Native Library Integration
> **Status:** ✅ preenchido

## Visão geral

O artefato que o Android carrega em runtime é uma **biblioteca compartilhada** `.so`
(*shared object*, no formato ELF). Para o Cargo produzir um `.so` em vez de um binário
executável ou de uma `.rlib` (formato interno do Rust), o crate deve declarar
`crate-type = ["cdylib"]`. Esta etapa cobre como controlar o que entra nesse `.so`:
símbolos exportados, tamanho, e a forma da biblioteca (dinâmica vs. estática).

```toml
# Cargo.toml
[lib]
crate-type = ["cdylib"]   # gera lib<nome>.so para o Android
```

Entender a anatomia do `.so` ajuda a depurar e a otimizar. Um `.so` ELF tem, entre outras
coisas: a **tabela de símbolos dinâmicos** (`.dynsym` — o que outros módulos podem ver, ex.:
suas funções `Java_...`), seções de código (`.text`) e dados (`.data`/`.rodata`), e
metadados de *unwinding* (`.eh_frame`). Cada decisão abaixo (visibilidade, strip, panic
strategy) mexe diretamente no tamanho dessas seções — e o tamanho do `.so` é peso de
download que o usuário paga. Ferramentas úteis para inspecionar: `nm -D libcore.so` (lista
símbolos exportados), `llvm-objdump`, `size`, e `bloaty` (analisa o que ocupa espaço).

## Subtópicos

### Dynamic library compilation

- `crate-type = ["cdylib"]` gera uma **dynamic library** (`.so`) carregada em runtime via
  `System.loadLibrary("nome")` — note que o prefixo `lib` e a extensão `.so` são implícitos
  (`System.loadLibrary("core")` carrega `libcore.so`).
- É o formato esperado pelo Android para código nativo; **cada ABI gera seu próprio `.so`**
  (ver *Cross Compilation Targets*), todos com o mesmo nome, em pastas `jniLibs/<abi>/`
  diferentes.
- Mantém o código nativo separado do bytecode do APK e permite carregamento sob demanda
  (você pode adiar `loadLibrary` até a feature que precisa dele ser usada).
- Diferença para `dylib`: `dylib` é o formato dinâmico *interno* do Rust (Rust↔Rust);
  `cdylib` expõe a **C ABI**, que é o que a JVM e o linker do Android entendem. Para Android
  é sempre `cdylib`.

### Static library bundling

- `crate-type = ["staticlib"]` gera um **`.a`** para ser **linkado dentro** de outra
  biblioteca nativa (ex.: um `.so` C++ via `externalNativeBuild`/CMake; ver *Gradle Build
  Integration*).
- Útil quando o Rust é um *componente* de uma lib nativa maior (ex.: você já tem um motor
  C++ e quer adicionar um módulo em Rust), e não a biblioteca de topo carregada diretamente.
- Para apps Android puros que carregam o Rust diretamente, prefira `cdylib` — é mais simples
  e evita a camada CMake.
- Dá para declarar **vários crate-types** ao mesmo tempo (`["cdylib", "staticlib", "rlib"]`)
  quando você quer o `.so` para Android, o `.a` para integração C, e a `rlib` para testes
  no host — o Cargo produz todos.

### Symbol visibility control

- Por padrão queremos exportar **apenas** os símbolos da fronteira FFI (funções
  `Java_...` ou as geradas pela UniFFI) e esconder o resto, reduzindo tamanho e
  superfície de ataque/conflito.
- Em Rust, `#[no_mangle] pub extern` mantém o nome do símbolo exportado; tudo o mais pode ser
  ocultado no link. Técnicas:
  - **Version script / `--exclude-libs`:** `-C link-arg=-Wl,--exclude-libs,ALL` esconde
    símbolos vindos de bibliotecas estáticas linkadas.
  - **`-C link-arg=-Wl,--version-script=exports.map`** com um mapa que lista explicitamente
    só os símbolos a exportar e marca o resto como `local: *;`.
- Por que importa: dois `.so` no mesmo processo exportando o mesmo símbolo global podem
  colidir (o loader resolve para um só), causando bugs sutis. Esconder o que não é fronteira
  evita isso.
- Símbolos não exportados permitem que o linker faça *dead-code elimination* mais agressiva,
  encolhendo o binário.

### Library name mangling

- O Rust faz **name mangling** dos símbolos por padrão (codifica módulo, tipos, hash) para
  permitir genéricos e evitar colisões; `#[no_mangle]` desativa isso para que o nome esperado
  pela JVM/JNI seja preservado **exatamente** (`Java_com_exemplo_...`).
- O nome passado a `System.loadLibrary("foo")` deve casar com o arquivo `libfoo.so`, e o
  nome simbólico da função deve casar com o `external fun` do Kotlin (ver *Java Native
  Interface → JNI function signature conventions*).
- Para UniFFI, os nomes são gerados e gerenciados pela ferramenta — não mexa neles nem
  aplique `strip` que os remova antes da hora.
- Alternativa ao `no_mangle`+nome simbólico: `RegisterNatives` em `JNI_OnLoad`, que registra
  os ponteiros em runtime e é robusto a `strip` (ver *Java Native Interface*).

### Strip debug symbols

- Builds de release devem **remover símbolos de debug** para encolher o `.so`
  drasticamente (um `.so` pode cair de dezenas de MB para poucos MB).
- Via Cargo: `strip = true` (ou `strip = "debuginfo"`/`"symbols"`) no `[profile.release]`;
  ou manualmente com o `llvm-strip` do NDK.
- **Guarde os símbolos separadamente** antes de strippar (`llvm-objcopy --only-keep-debug`
  gerando um `libcore.so.debug`) para conseguir **simbolizar crashes nativos** depois — sem
  eles, uma stack trace de tombstone é só um monte de endereços hex inúteis (ver
  *Instrumented Testing → Native crash analysis*).
- No fluxo da Play Store, você sobe os **símbolos nativos de debug** (`debugSymbolLevel` no
  Gradle) para o Console desimbolizar os crashes do Android Vitals automaticamente.

### Size optimization flags

- Combine várias técnicas para um `.so` enxuto (ver também *Cross Compilation Targets →
  Cargo profiles*):
  - `opt-level = "z"` (ou `"s"`), `lto = true`, `codegen-units = 1`, `panic = "abort"`.
  - `strip = true`.
  - Evitar dependências pesadas; usar `default-features = false` e ativar só o necessário.
    Uma única dependência transitiva grande (ex.: um runtime async completo onde bastaria um
    executor mínimo) pode dominar o tamanho.
  - `build-std` com `panic_immediate_abort` (nightly) recompila a `std` enxuta, em casos
    extremos — corta as mensagens de panic e código de formatação.
- **Meça o que cresce:** rode `cargo bloaty` / `bloaty libcore.so` ou
  `cargo build --release -Z print-link-args` para ver o que domina o binário antes de
  otimizar às cegas.
- O Gradle pode comprimir/empacotar os `.so` no APK; meça o tamanho **instalado** e o
  **tamanho de download** (o AAB entrega só a ABI do device — ver *Package Management →
  Android App Bundle creation*), não só o do APK universal.

## Armadilhas comuns

- **`crate-type` errado** — esquecer `cdylib` produz uma `.rlib`/binário e o Gradle não acha
  `.so` nenhum; ou produzir só `staticlib` quando o app esperava carregar diretamente.
- **`strip` cedo demais** — strippar antes de arquivar os símbolos torna impossível
  simbolizar crashes de produção. Arquive primeiro.
- **LTO + tempo de build** — `lto = true` + `codegen-units = 1` deixam o build lento; use só
  no profile de release, mantendo o dev rápido.
- **Símbolo exportado a mais colidindo** — esquecer de esconder símbolos pode causar conflito
  com outras `.so` (libc++, outras libs nativas) no mesmo processo.

## Assuntos correlatos

- **Empacotamento e assinatura** do `.so` no APK/AAB: *Package Management*.
- **Onde o `.so` precisa ficar** e como o Gradle o encontra: *Gradle Build Integration →
  Library search path definitions*.
- **`libc++_shared.so`:** se você linka contra a STL do C++ (via algum crate com `cc`), pode
  precisar empacotar `libc++_shared.so` junto — um ponto de atenção em integrações híbridas.

## Referências

- Linkage (crate types): https://doc.rust-lang.org/reference/linkage.html
- Cargo profiles: https://doc.rust-lang.org/cargo/reference/profiles.html
- Minimizing Rust binary size: https://github.com/johnthagen/min-sized-rust
- bloaty (size profiler): https://github.com/google/bloaty
