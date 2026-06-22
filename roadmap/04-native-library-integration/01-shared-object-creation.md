# Shared Object Creation

> **Seção:** Native Library Integration
> **Status:** ✅ preenchido

## Visão geral

O artefato que o Android carrega em runtime é uma **biblioteca compartilhada** `.so`
(*shared object*, ELF). Para o Cargo produzir um `.so` em vez de um binário ou `.rlib`, o
crate deve declarar `crate-type = ["cdylib"]`. Esta etapa cobre como controlar o que entra
nesse `.so`: símbolos exportados, tamanho e a forma da biblioteca (dinâmica vs. estática).

```toml
# Cargo.toml
[lib]
crate-type = ["cdylib"]   # gera lib<nome>.so para o Android
```

## Subtópicos

### Dynamic library compilation

- `crate-type = ["cdylib"]` gera uma **dynamic library** (`.so`) carregada em runtime via
  `System.loadLibrary("nome")` — note que o prefixo `lib` e a extensão `.so` são implícitos.
- É o formato esperado pelo Android para código nativo; cada ABI gera seu próprio `.so`.
- Mantém o código nativo separado do APK base e permite carregamento sob demanda.

### Static library bundling

- `crate-type = ["staticlib"]` gera um **`.a`** para ser **linkado dentro** de outra
  biblioteca nativa (ex.: um `.so` C++ via `externalNativeBuild`/CMake).
- Útil quando o Rust é um *componente* de uma lib nativa maior, e não a biblioteca de topo.
- Para apps Android puros que carregam o Rust diretamente, prefira `cdylib`.

### Symbol visibility control

- Por padrão queremos exportar **apenas** os símbolos da fronteira FFI (funções
  `Java_...` ou as geradas pela UniFFI) e esconder o resto, reduzindo tamanho e
  superfície.
- Em Rust, `#[no_mangle] pub extern` mantém o nome do símbolo; tudo o mais pode ser
  ocultado no link com flags como `-C link-arg=-Wl,--exclude-libs,ALL` e version scripts.
- Símbolos não exportados permitem que o linker faça *dead-code elimination* mais agressiva.

### Library name mangling

- O Rust faz **name mangling** dos símbolos por padrão; `#[no_mangle]` desativa isso para
  que o nome esperado pela JVM/JNI seja preservado exatamente.
- O nome passado a `System.loadLibrary("foo")` deve casar com o arquivo `libfoo.so`.
- Para UniFFI, os nomes são gerados e gerenciados pela ferramenta — não mexa neles.

### Strip debug symbols

- Builds de release devem **remover símbolos de debug** para encolher o `.so`
  drasticamente (um `.so` pode cair de dezenas de MB para poucos MB).
- Via Cargo: `strip = true` no `[profile.release]`; ou manualmente com o `llvm-strip` do
  NDK.
- Guarde os símbolos separadamente (`.so.debug`) para conseguir simbolizar crashes nativos
  depois (ver *Native crash analysis*).

### Size optimization flags

- Combine várias técnicas para um `.so` enxuto:
  - `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`.
  - `strip = true`.
  - Evitar dependências pesadas; usar `default-features = false`.
  - `build-std` com panic=abort (nightly) para reduzir ainda mais, em casos extremos.
- O Gradle pode comprimir os `.so` no APK; meça o tamanho **instalado**, não só o do APK.

## Referências

- Linkage (crate types): https://doc.rust-lang.org/reference/linkage.html
- Cargo profiles: https://doc.rust-lang.org/cargo/reference/profiles.html
- Minimizing Rust binary size: https://github.com/johnthagen/min-sized-rust
