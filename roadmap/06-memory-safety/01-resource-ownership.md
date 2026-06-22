# Resource Ownership

> **Seção:** Memory Safety
> **Status:** ✅ preenchido

## Visão geral

A maior vantagem do Rust em mobile é a **segurança de memória sem garbage collector**: o
sistema de *ownership* e *borrowing* garante, em tempo de compilação, ausência de
use-after-free, double-free e data races. Mas na fronteira FFI com o Android essas garantias
**não atravessam automaticamente** — quando um ponteiro Rust é entregue ao Java como handle,
a posse precisa ser gerenciada manualmente. Esta etapa cobre como preservar a segurança de
memória ao expor recursos Rust para a JVM.

## Subtópicos

### Boxed pointer usage

- `Box<T>` aloca no heap e dá um ponteiro de posse única — a base para entregar objetos
  Rust ao Java como *handle opaco*.
- Padrão: `Box::into_raw(Box::new(x))` devolve um `*mut T` (passado como `jlong`); mais
  tarde `Box::from_raw(ptr)` reassume a posse para dropar. **Cada `into_raw` exige
  exatamente um `from_raw`.**
- O Java trata o handle como número opaco e nunca o desreferencia.

### Null pointer prevention

- Rust elimina null por design: referências (`&T`) nunca são nulas e a ausência é modelada
  com `Option<T>`. Isso remove uma classe inteira de crashes.
- Na fronteira FFI, porém, ponteiros crus **podem** chegar nulos vindos do Java; valide-os
  (`if ptr.is_null()`) e use `NonNull<T>` internamente para documentar a invariante.
- Nunca faça `from_raw`/desreferência de um ponteiro sem checar a origem.

### Memory leak detection

- Vazamentos ocorrem tipicamente quando o Java perde o handle sem chamar o `destroy`, ou
  quando uma `GlobalRef` JNI não é liberada.
- Ferramentas: **Miri** (detecta UB/leaks em testes), **AddressSanitizer/LeakSanitizer**
  no NDK, e o **Memory Profiler** do Android Studio para o lado nativo.
- Garanta liberação determinística amarrando o `destroy` nativo ao ciclo de vida do objeto
  Kotlin (ex.: `AutoCloseable`/`use {}`).

### Reference counting implementation

- `Rc<T>` (single-thread) e `Arc<T>` (thread-safe) compartilham posse por contagem de
  referências; `Weak<T>` quebra ciclos que de outra forma vazariam.
- `Arc` é essencial para compartilhar estado entre as threads de trabalho do Rust e a
  fronteira FFI. Ciclos `Arc`↔`Arc` **vazam**; use `Weak` para o lado "de volta".

### Automatic drop trait execution

- O trait `Drop` roda automaticamente quando um valor sai de escopo — é o **RAII** do Rust:
  arquivos, sockets, locks e memória são liberados sem `free` manual.
- Cuidado: ao usar `Box::into_raw`/`mem::forget`, o `Drop` **não** roda — a liberação passa
  a ser sua responsabilidade. Por isso todo handle exportado precisa de um par `destroy`.
- Implemente `Drop` em wrappers de recursos nativos para garantir limpeza mesmo em caminhos
  de erro.

### Zero cost abstraction safety

- As abstrações de segurança do Rust (ownership, `Option`, iterators, RAII) são **zero-cost**:
  compilam para o mesmo código que C escrito à mão, sem overhead de runtime nem GC pauses.
- Isso é decisivo em mobile: você ganha segurança de memória **e** desempenho previsível,
  sem pausas de coletor que prejudicariam frame rate ou latência.
- Mantenha o `unsafe` confinado a pequenos blocos bem auditados na fronteira FFI; todo o
  resto do núcleo permanece seguro por construção.

## Referências

- Ownership (The Rust Book): https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html
- Rc/Arc: https://doc.rust-lang.org/book/ch15-04-rc.html
- Miri: https://github.com/rust-lang/miri
