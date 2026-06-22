# Automated Bindings

> **Seção:** FFI and Bindings
> **Status:** ✅ preenchido

## Visão geral

Escrever JNI à mão é verboso e propenso a erros. **Bindings automáticos** geram a camada de
cola (glue code) a partir de uma definição de interface, garantindo que os dois lados —
Rust e Kotlin — fiquem sempre em sincronia. A ferramenta de referência no ecossistema mobile
é a **UniFFI** (da Mozilla), que gera bindings idiomáticos para Kotlin, Swift e Python a
partir de Rust. Alternativas como **cbindgen** (gera headers C) e **bindgen** (consome
headers C) cobrem cenários de interoperação em C.

## Subtópicos

### UniFFI interface definition

- A interface pode ser descrita de duas formas:
  - **Arquivo UDL** (`.udl`) — uma IDL própria que lista funções, objetos e tipos.
  - **Macros procedurais** (`#[uniffi::export]`, `#[derive(uniffi::Record)]`) direto no
    código Rust — abordagem moderna preferida, sem arquivo separado.

```rust
#[uniffi::export]
pub fn saudacao(nome: String) -> String {
    format!("Olá, {nome}!")
}

#[derive(uniffi::Record)]
pub struct Usuario { pub nome: String, pub idade: u32 }
```

### Foreign function declaration

- A partir da definição, a UniFFI gera o **scaffolding Rust** (`uniffi::setup_scaffolding!()`)
  com funções `extern "C"` e a classe/objeto Kotlin correspondente.
- No Kotlin você apenas importa o pacote gerado e chama `saudacao("Ana")` — sem `external
  fun`, sem JNI manual. Os detalhes de FFI ficam escondidos.

### Header generation processes

- Em fluxos baseados em C, **`cbindgen`** lê o código Rust e gera um header `.h` com as
  assinaturas `extern "C"`, consumível por C/C++/Gradle externalNativeBuild.
- **`bindgen`** faz o caminho inverso: lê um header C de uma lib de terceiros e gera os
  bindings Rust `unsafe extern "C"` correspondentes.
- A UniFFI dispensa headers manuais — gera os artefatos (`.kt`) por um build step próprio
  (`uniffi-bindgen generate`).

### Type translation rules

- Cada ferramenta define como tipos cruzam a fronteira:
  - UniFFI: `String`, `Vec<T>`, `Option<T>`, `Result<T, E>`, structs (`Record`), enums,
    objetos com métodos (`Object`) e até **callbacks/traits** e **async**.
  - Tipos `Result<T, E>` viram **exceptions** idiomáticas no Kotlin.
- Há um limite: tipos que não têm representação clara nos dois lados precisam ser
  modelados com os tipos suportados (ex.: usar `Record` em vez de uma struct arbitrária).

### Interface abstraction layers

- A boa prática é manter um **crate de núcleo puro** (lógica, sem FFI) e um **crate fino de
  binding** que apenas anota/exporta a API pública. Isso mantém o núcleo testável fora do
  Android e evita poluir a lógica com detalhes de FFI.
- A interface exposta deve ser **pequena e estável** (fachada): poucas funções de alto
  nível, em vez de espelhar toda a API interna.

### Automated glue code generation

- O pipeline típico com UniFFI:
  1. `cargo build` compila o `cdylib` Rust.
  2. `uniffi-bindgen generate --language kotlin` gera os `.kt`.
  3. Os `.kt` entram no `sourceSet` do módulo Android; o `.so` vai para `jniLibs/`.
- Integre os passos 1–2 em um build script / task Gradle para que a regeneração seja
  automática a cada build (ver doc de *Gradle Build Integration*).

## Referências

- UniFFI: https://mozilla.github.io/uniffi-rs/
- cbindgen: https://github.com/mozilla/cbindgen
- bindgen: https://rust-lang.github.io/rust-bindgen/
