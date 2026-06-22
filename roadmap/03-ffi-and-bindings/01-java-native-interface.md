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

## Subtópicos

### JNI function signature conventions

- A JVM localiza a função nativa pelo **nome simbólico**:
  `Java_<pacote>_<Classe>_<metodo>`, com `_` no lugar de `.`. A função Rust deve ser
  `#[no_mangle] pub extern "system"`.

```rust
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
  `System.loadLibrary("nome_da_lib")`.

### Type mapping requirements

- Tipos primitivos têm equivalência direta: `jint`↔`i32`, `jlong`↔`i64`, `jboolean`↔`u8`,
  `jfloat`↔`f32`, `jdouble`↔`f64`.
- Tipos de referência (`String`, arrays, objetos) chegam como handles opacos
  (`JString`, `JObjectArray`...) e exigem chamadas do `JNIEnv` para ler/escrever.
- **Strings** precisam de conversão explícita (`get_string`/`new_string`) — JNI usa UTF-16
  modificado, e o crate `jni` faz a ponte para `String` UTF-8 do Rust.

### Local reference management

- Toda referência a objeto Java retornada pelo `JNIEnv` é uma **local reference**, válida
  apenas durante a chamada nativa e contabilizada numa tabela de tamanho limitado.
- Em **loops** que criam muitas referências, libere-as (`delete_local_ref`) ou use
  `with_local_frame` para evitar **estouro da tabela de referências locais**.
- Não guarde locals entre chamadas JNI — elas são invalidadas quando a função retorna.

### Global reference lifecycle

- Para reter um objeto Java **entre chamadas** (ex.: um callback guardado em uma struct
  Rust), promova-o a **global reference** com `env.new_global_ref(obj)`.
- Globals **não** são liberadas automaticamente: você deve chamar o equivalente a
  `DeleteGlobalRef` (o `GlobalRef` do crate `jni` faz isso no `Drop`) ou vaza memória.
- Use globals com moderação; cada uma mantém o objeto vivo e impede o GC de coletá-lo.

### Exception handling across boundaries

- Uma exceção Java lançada durante uma chamada JNI fica **pendente**: chamadas subsequentes
  ao `JNIEnv` têm comportamento indefinido até você tratá-la.
- Cheque com `env.exception_check()` e limpe com `env.exception_clear()` antes de continuar.
- Para sinalizar erro do Rust para o Java, use `env.throw_new("java/lang/RuntimeException",
  msg)`. **Nunca** deixe um `panic!` do Rust cruzar a fronteira FFI — isso é UB; capture com
  `std::panic::catch_unwind` e converta em exceção.

### Native memory allocation

- Memória alocada em Rust (ex.: um `Box` cujo ponteiro é devolvido como `jlong` "handle")
  **não é gerenciada pelo GC** — você é responsável por liberá-la.
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
    unsafe { drop(Box::from_raw(h as *mut Estado)) }   // libera
}
```

## Referências

- Crate `jni`: https://docs.rs/jni
- JNI Specification: https://docs.oracle.com/javase/8/docs/technotes/guides/jni/spec/jniTOC.html
- JNI tips (Android): https://developer.android.com/training/articles/perf-jni
