# Java Native Interface

> **Seção:** FFI and Bindings
> **Status:** ✅ preenchido

## Visão geral

O **JNI (Java Native Interface)** é a ponte oficial entre o mundo gerenciado da JVM/ART
(Kotlin/Java) e o código nativo (Rust/C/C++). Em Rust, o crate **`jni`** fornece os tipos
(`JNIEnv`, `JObject`, `jstring`, etc.) para escrever funções que a JVM consegue chamar.
Dominar JNI significa entender as convenções de nomeação, o mapeamento de tipos e —
crucialmente — o **gerenciamento de referências** e o **tratamento de exceções** que
atravessam a fronteira, pois erros aí viram crashes nativos difíceis de depurar.

Antes de mergulhar nos detalhes, vale o contexto: o JNI é **verboso, propenso a erros e
cheio de `unsafe`**. Para a maioria dos projetos novos, a recomendação é usar **UniFFI**
(ver *Automated Bindings*), que gera toda essa cola automaticamente. Você deve dominar JNI
manual quando: (a) precisa chamar **APIs Java a partir do Rust** (Keystore, sensores,
`AssetManager`) — algo que a UniFFI não faz; (b) está integrando com uma base de código JNI
existente; ou (c) quer controle fino sobre a fronteira por performance. Pense no JNI manual
como a "linguagem assembly" da interoperabilidade: bom conhecer, melhor evitar no dia a dia.

Mentalmente, há dois sentidos de travessia:

- **Java → Rust** (downcall): a JVM chama uma função nativa sua. É o caso das funções
  `Java_...` abaixo.
- **Rust → Java** (upcall): seu código nativo chama métodos Java via `JNIEnv` (ex.: acessar
  o Keystore). Exige um `JNIEnv` válido para a thread atual — e threads criadas pelo Rust
  precisam se **anexar à JVM** primeiro (`AttachCurrentThread`).

## Subtópicos

### JNI function signature conventions

- A JVM localiza a função nativa pelo **nome simbólico**:
  `Java_<pacote>_<Classe>_<metodo>`, com `_` no lugar de `.`. A função Rust deve ser
  `#[no_mangle] pub extern "system"`.

```rust
use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::jstring;

#[no_mangle]
pub extern "system" fn Java_com_exemplo_app_RustCore_saudacao<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    nome: JString<'local>,
) -> jstring {
    let nome: String = env.get_string(&nome).unwrap().into();
    let saida = format!("Olá, {nome}!");
    env.new_string(saida).unwrap().into_raw()
}
```

- Todo método nativo é declarado no Kotlin com `external fun` e a lib carregada via
  `System.loadLibrary("nome_da_lib")` (sem o prefixo `lib` e a extensão `.so`).
- **Cuidado com underscores no pacote:** se o nome do pacote contém `_`, o JNI usa o
  *escape* `_1`. Ex.: pacote `com.foo_bar` vira `Java_com_foo_1bar_...`. É uma fonte clássica
  de "method not found" — por isso prefira pacotes sem underscore.
- Existe também o **`RegisterNatives`**, alternativa em que você registra os ponteiros de
  função em tempo de execução (em `JNI_OnLoad`) em vez de depender do nome simbólico. É mais
  rápido de resolver, permite nomes arbitrários e ajuda quando o `strip` removeria os
  símbolos — mas é mais código de setup.

### Type mapping requirements

- Tipos primitivos têm equivalência direta: `jint`↔`i32`, `jlong`↔`i64`, `jboolean`↔`u8`
  (`0`/`1`), `jfloat`↔`f32`, `jdouble`↔`f64`, `jchar`↔`u16`, `jbyte`↔`i8`.
- Tipos de referência (`String`, arrays, objetos) chegam como handles opacos
  (`JString`, `JObjectArray`, `JByteArray`...) e exigem chamadas do `JNIEnv` para ler/escrever.
- **Strings** precisam de conversão explícita (`get_string`/`new_string`) — JNI usa UTF-16
  "modificado" (com tratamento especial de `NUL` e surrogates), e o crate `jni` faz a ponte
  para `String` UTF-8 do Rust. Para muitos dados textuais, isso significa **cópia +
  reencoding**; pese o custo em hot paths.
- **Arrays primitivos** (`JByteArray`, etc.) podem ser acessados com cópia
  (`convert_byte_array`) ou sem cópia via `get_array_elements`/critical sections — estas
  últimas evitam alocação mas têm restrições severas (não pode chamar outras funções JNI nem
  bloquear enquanto a região está "presa"). Para grandes buffers, considere
  `NewDirectByteBuffer` (ver *Native memory allocation* e *Data Synchronization → Shared
  memory buffer management*).

### Local reference management

- Toda referência a objeto Java retornada pelo `JNIEnv` é uma **local reference**, válida
  apenas durante a chamada nativa e contabilizada numa tabela de tamanho limitado (o padrão
  garantido é pequeno, ~512–maior; não conte com muito).
- Em **loops** que criam muitas referências, libere-as (`delete_local_ref`) ou use
  `with_local_frame` para criar um escopo que descarta tudo de uma vez:

```rust
env.with_local_frame(16, |env| -> Result<(), jni::errors::Error> {
    for i in 0..n {
        let item = env.get_object_array_element(&array, i)?;
        // ...usa item; será liberado ao sair do frame...
    }
    Ok(())
})?;
```

- Não guarde locals entre chamadas JNI — elas são invalidadas quando a função retorna. Para
  reter, promova a global (ver abaixo).
- O sintoma de estourar a tabela é um `JNI ERROR (app bug): local reference table overflow`
  no logcat seguido de abort — quase sempre é um loop sem liberação.

### Global reference lifecycle

- Para reter um objeto Java **entre chamadas** (ex.: um callback guardado em uma struct
  Rust, ou a referência a uma classe), promova-o a **global reference** com
  `env.new_global_ref(obj)`.
- Globals **não** são liberadas automaticamente: você deve chamar o equivalente a
  `DeleteGlobalRef` — o `GlobalRef` do crate `jni` faz isso no `Drop`, então mantê-lo dentro
  de uma struct Rust com tempo de vida bem definido cuida disso. Esquecer = vazamento.
- Use globals com moderação; cada uma **mantém o objeto vivo** e impede o GC de coletá-lo —
  guardar uma global da Activity vaza toda a árvore de views (ver *Application Lifecycle →
  Armadilhas comuns*). Para callbacks, considere `WeakGlobalRef` quando não quiser prolongar
  a vida do objeto.
- A `JavaVM` (obtida de `env.get_java_vm()`) **é** segura para guardar globalmente e é o que
  permite anexar threads do Rust depois.

### Exception handling across boundaries

- Uma exceção Java lançada durante uma chamada JNI fica **pendente**: chamadas subsequentes
  ao `JNIEnv` têm comportamento indefinido até você tratá-la. Isto inclui exceções lançadas
  por *upcalls* que você fez (ex.: um método Java que você chamou jogou uma exceção).
- Cheque com `env.exception_check()` e limpe com `env.exception_clear()` (ou
  `exception_describe` para logar) antes de continuar.
- Para sinalizar erro do Rust para o Java, use `env.throw_new("java/lang/RuntimeException",
  msg)` e retorne um valor *default* — a exceção só é "vista" pela JVM ao retornar.
- **Nunca** deixe um `panic!` do Rust cruzar a fronteira FFI — isso é UB. Capture com
  `std::panic::catch_unwind` e converta em exceção:

```rust
#[no_mangle]
pub extern "system" fn Java_com_exemplo_app_RustCore_fazAlgo<'local>(
    mut env: JNIEnv<'local>, _class: JClass<'local>,
) {
    let resultado = std::panic::catch_unwind(|| {
        // lógica que pode entrar em panic
    });
    if resultado.is_err() {
        let _ = env.throw_new("java/lang/RuntimeException", "panic no núcleo Rust");
    }
}
```

- Lembre que `catch_unwind` só funciona com `panic = "unwind"`; se você usa
  `panic = "abort"` (ver *Cross Compilation Targets → Cargo profiles*) o processo aborta em
  vez de desenrolar — uma decisão de design a tomar conscientemente.

### Native memory allocation

- Memória alocada em Rust (ex.: um `Box` cujo ponteiro é devolvido como `jlong` "handle")
  **não é gerenciada pelo GC** — você é responsável por liberá-la (ver *Resource Ownership →
  Boxed pointer usage*).
- Padrão comum: `Box::into_raw` para "vazar" o ponteiro para o Java guardar, e um método
  nativo `destroy(handle: jlong)` que faz `Box::from_raw` para reconstruir e dropar.
- Buffers diretos (`NewDirectByteBuffer`) permitem compartilhar memória sem cópia, mas
  exigem cuidado com tempo de vida — o Rust deve manter o buffer vivo enquanto o Java o usa.

```rust
#[no_mangle]
pub extern "system" fn Java_com_exemplo_app_RustCore_create(/* ... */) -> jlong {
    let estado = Box::new(Estado::new());
    Box::into_raw(estado) as jlong          // handle entregue ao Java
}

#[no_mangle]
pub extern "system" fn Java_com_exemplo_app_RustCore_destroy(_e: JNIEnv, _c: JClass, h: jlong) {
    if h == 0 { return; }                          // proteção contra null/double-free
    unsafe { drop(Box::from_raw(h as *mut Estado)) }   // libera
}
```

- **Boa prática no lado Kotlin:** implemente `AutoCloseable` na classe que guarda o handle e
  chame `destroy` no `close()`, usando `use { }` para liberação determinística — assim o
  ciclo de vida nativo segue o ciclo de vida do objeto Kotlin (ver *Resource Ownership →
  Memory leak detection*).

## Armadilhas comuns

- **Usar o `JNIEnv` na thread errada** — o `JNIEnv` é por-thread e não pode ser compartilhado
  entre threads. Para chamar Java de uma thread do Rust, guarde a `JavaVM` e faça
  `attach_current_thread` nessa thread.
- **`unwrap()` em produção** — os exemplos usam `unwrap()` por brevidade, mas um `unwrap` que
  falha vira panic → UB na fronteira. Trate erros e converta em exceção.
- **Double-free / use-after-free de handles** — chamar `destroy` duas vezes, ou usar o handle
  depois de destruído. Zere o handle no Kotlin após `close()` e cheque `== 0` no Rust.
- **Strings com UTF-16 modificado** — caracteres fora do BMP (emoji) e `NUL` embutidos têm
  tratamento especial; confie no `get_string`/`new_string` do crate `jni` em vez de
  manipular bytes à mão.

## Assuntos correlatos

- **UniFFI** elimina quase todo este capítulo para o caso Java↔Rust comum — ver *Automated
  Bindings*. Use JNI manual onde a UniFFI não chega (chamar Java do Rust).
- **`ndk` crate** dá acesso tipado a APIs nativas do Android (AssetManager, surface, sensores)
  sem JNI manual para várias delas — ver *Rendering Integration*.
- **Segurança de memória na fronteira** (handles, `NonNull`, `Drop`) é aprofundada em
  *Resource Ownership*; **passagem de dados entre threads** em *Data Synchronization*.

## Referências

- Crate `jni`: https://docs.rs/jni
- JNI Specification: https://docs.oracle.com/javase/8/docs/technotes/guides/jni/spec/jniTOC.html
- JNI tips (Android): https://developer.android.com/training/articles/perf-jni
- The Rustonomicon (FFI): https://doc.rust-lang.org/nomicon/ffi.html
