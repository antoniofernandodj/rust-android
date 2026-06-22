# Automated Bindings

> **Seção:** FFI and Bindings
> **Status:** ✅ preenchido

## Visão geral

Escrever JNI à mão é verboso e propenso a erros (ver *Java Native Interface*). **Bindings
automáticos** geram a camada de cola (glue code) a partir de uma definição de interface,
garantindo que os dois lados — Rust e Kotlin — fiquem sempre em sincronia. A ferramenta de
referência no ecossistema mobile é a **UniFFI** (da Mozilla, nascida no Firefox para
Android/iOS), que gera bindings idiomáticos para Kotlin, Swift e Python a partir de Rust.
Alternativas como **cbindgen** (gera headers C) e **bindgen** (consome headers C) cobrem
cenários de interoperação em C.

O ganho principal não é só escrever menos código — é **eliminar uma classe inteira de bugs**.
No JNI manual, um descasamento entre a assinatura Kotlin (`external fun`) e a função
`Java_...` em Rust só explode em runtime, com um crash obscuro. Com bindings gerados, a cola
dos dois lados sai da *mesma* fonte, então é impossível dessincronizar: se a API Rust muda, o
Kotlin gerado muda junto, e o compilador Kotlin acusa qualquer chamada quebrada.

Qual escolher, em uma frase:

| Ferramenta | Direção | Use quando… |
|------------|---------|-------------|
| **UniFFI** | Rust → Kotlin/Swift | é o caso mobile padrão; quer idiomático e seguro |
| **cbindgen** | Rust → header C | outra linguagem/CMake consome seu Rust via C ABI |
| **bindgen** | header C → Rust | você precisa chamar uma lib C de terceiros do Rust |

Para um app Android novo com núcleo Rust, **comece pela UniFFI** e só desça para
JNI/cbindgen/bindgen onde ela não alcança.

## Subtópicos

### UniFFI interface definition

- A interface pode ser descrita de duas formas:
  - **Arquivo UDL** (`.udl`) — uma IDL própria que lista funções, objetos e tipos. Era a
    forma original; ainda suportada.
  - **Macros procedurais** (`#[uniffi::export]`, `#[derive(uniffi::Record)]`,
    `#[derive(uniffi::Enum)]`, `#[uniffi::export(callback_interface)]`) direto no código
    Rust — abordagem moderna preferida ("proc-macro mode"), sem arquivo separado, com a
    assinatura sempre ao lado da implementação.

```rust
#[uniffi::export]
pub fn saudacao(nome: String) -> String {
    format!("Olá, {nome}!")
}

#[derive(uniffi::Record)]
pub struct Usuario { pub nome: String, pub idade: u32 }

#[derive(uniffi::Enum)]
pub enum Status { Ativo, Inativo { desde: u64 } }

uniffi::setup_scaffolding!();   // gera o ponto de entrada FFI do crate
```

- Conceitos centrais do modelo de tipos da UniFFI:
  - **`Record`** — struct de dados, copiada por valor através da fronteira (POD).
  - **`Object`** — tipo com identidade e métodos, passado por referência opaca
    (`Arc<Self>`); o objeto vive no Rust e o Kotlin segura um handle (lembra o padrão de
    handle do JNI, mas gerado e seguro).
  - **`Enum`** — vira `sealed class`/`enum class` idiomática no Kotlin.

### Foreign function declaration

- A partir da definição, a UniFFI gera o **scaffolding Rust** (`uniffi::setup_scaffolding!()`)
  com funções `extern "C"` e a classe/objeto Kotlin correspondente.
- No Kotlin você apenas importa o pacote gerado e chama `saudacao("Ana")` — sem `external
  fun`, sem JNI manual, sem `System.loadLibrary` explícito (o código gerado cuida do load).
  Os detalhes de FFI ficam escondidos.
- **Objetos** geram uma classe `AutoCloseable`/`Disposable` no Kotlin: a destruição do
  recurso nativo é amarrada ao `close()`/`use {}`, resolvendo automaticamente o problema de
  ciclo de vida que no JNI manual exigia o par `create`/`destroy` (ver *Resource Ownership*).

### Header generation processes

- Em fluxos baseados em C, **`cbindgen`** lê o código Rust e gera um header `.h` com as
  assinaturas `extern "C"`, consumível por C/C++/Gradle `externalNativeBuild` (ver *Gradle
  Build Integration*). Configurável por `cbindgen.toml`; pode rodar num `build.rs`.
- **`bindgen`** faz o caminho inverso: lê um header C de uma lib de terceiros e gera os
  bindings Rust `unsafe extern "C"` correspondentes — é como você consome uma `.so`/`.a`
  nativa pré-existente a partir do Rust.
- A UniFFI dispensa headers manuais — gera os artefatos (`.kt`) por um build step próprio
  (`uniffi-bindgen generate`), descrito abaixo.

### Type translation rules

- Cada ferramenta define como tipos cruzam a fronteira:
  - UniFFI: tipos primitivos, `String`, `Vec<T>`, `HashMap<K,V>`, `Option<T>`,
    `Result<T, E>`, structs (`Record`), enums, `Duration`/`SystemTime`, bytes
    (`Vec<u8>`/`ByteArray`), objetos com métodos (`Object`) e até **callback
    interfaces/traits** e **funções `async`**.
  - Tipos `Result<T, E>` viram **exceptions idiomáticas** no Kotlin: o erro `E` (um
    `#[derive(uniffi::Error)]`) vira uma `Exception` que você captura com `try/catch` — bem
    mais limpo que checar exceção pendente no JNI manual.
- **Callbacks (Rust chamando Kotlin):** uma `callback_interface` permite o núcleo Rust
  notificar a UI (ex.: progresso, eventos), invertendo o sentido da chamada sem JNI
  upcall manual — muito usado para o fluxo de eventos descrito em *Data Synchronization*.
- **`async`:** a UniFFI faz a ponte entre `async fn` do Rust e `suspend fun` do Kotlin,
  integrando com corrotinas — ótimo para operações de I/O sem bloquear a UI thread.
- Há um limite: tipos sem representação clara nos dois lados precisam ser modelados com os
  tipos suportados (ex.: usar um `Record` com campos simples em vez de uma struct arbitrária
  com tipos exóticos).

### Interface abstraction layers

- A boa prática é manter um **crate de núcleo puro** (lógica, sem FFI) e um **crate fino de
  binding** que apenas anota/exporta a API pública. Isso mantém o núcleo testável fora do
  Android (`cargo test` no host) e evita poluir a lógica com detalhes de FFI (ver *Cross
  Platform Logic → Shared logic layer architecture*).

```
core/        # lógica pura, sem uniffi — roda em qualquer host, 100% testável
  └── lib.rs
bindings/    # crate cdylib que só reexporta a API pública com #[uniffi::export]
  └── lib.rs # uniffi::setup_scaffolding!();
```

- A interface exposta deve ser **pequena e estável** (fachada): poucas funções de alto
  nível orientadas a caso de uso, em vez de espelhar toda a API interna. Uma fachada enxuta
  estabiliza a ABI entre versões e reduz a superfície de bugs de FFI.

### Automated glue code generation

- O pipeline típico com UniFFI:
  1. `cargo build` (ou `cargo ndk`) compila o `cdylib` Rust para as ABIs alvo.
  2. `uniffi-bindgen generate --library <caminho/lib.so> --language kotlin --out-dir ...`
     gera os `.kt` (modo moderno: extrai os metadados direto do `.so`).
  3. Os `.kt` entram no `sourceSet` do módulo Android; o `.so` vai para `jniLibs/` (ver
     *Gradle Build Integration*).
- Integre os passos 1–2 em um build script / task Gradle para que a regeneração seja
  **automática a cada build** — assim ninguém esquece de regenerar e fica com bindings
  desatualizados (um bug silencioso e chato).

## Armadilhas comuns

- **Bindings `.kt` desatualizados** — gerar à mão e esquecer de regerar após mudar a API
  Rust leva a crashes/erros de link. Automatize a geração no build.
- **Versão do `uniffi` divergindo** — a versão do crate `uniffi` no `Cargo.toml` deve casar
  com a do `uniffi-bindgen` que gera os `.kt`; descasamento gera ABI incompatível. Fixe e
  versione ambas (idealmente o `uniffi-bindgen` vem do mesmo crate).
- **Expor demais** — anotar o núcleo inteiro com `#[uniffi::export]` acopla a ABI a detalhes
  internos. Exporte só a fachada.
- **Tipos não suportados** — tentar exportar uma struct com um campo de tipo arbitrário
  falha na geração; modele com tipos suportados (`Record`/`Enum`/`Object`).

## Assuntos correlatos

- **Quando NÃO usar UniFFI:** chamar APIs Java *do* Rust (Keystore, sensores) ainda exige
  JNI manual ou o crate `ndk`/`jni` — ver *Java Native Interface* e *Cryptographic
  Implementation → Hardware backed keystore access*.
- **`flapigen`/`diplomat`** são alternativas de geração de binding com trade-offs
  diferentes; UniFFI é a mais madura para o caso Android/iOS.
- **Empacotamento:** R8/ProGuard pode remover classes geradas pela UniFFI — lembre das
  regras `-keep` (ver *Package Management → Proguard rule configuration*).

## Referências

- UniFFI: https://mozilla.github.io/uniffi-rs/
- UniFFI proc-macro mode: https://mozilla.github.io/uniffi-rs/proc_macro/index.html
- cbindgen: https://github.com/mozilla/cbindgen
- bindgen: https://rust-lang.github.io/rust-bindgen/
