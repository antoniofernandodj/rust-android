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

Os três pilares do modelo, recapitulados, porque tudo abaixo deriva deles:

1. **Ownership** — cada valor tem exatamente um dono; quando o dono sai de escopo, o valor é
   liberado (sem `free` manual, sem GC).
2. **Borrowing** — você pode emprestar referências imutáveis (`&T`, várias) **ou** uma
   mutável (`&mut T`, exclusiva), nunca os dois ao mesmo tempo. É o que elimina data races em
   tempo de compilação.
3. **Lifetimes** — o compilador prova que nenhuma referência sobrevive ao dado que aponta.

O ponto-chave para Android: a JVM tem um **modelo de memória oposto** (GC, referências
compartilhadas mutáveis). Na fronteira, o Rust "abre mão" temporariamente das suas garantias
(num bloco `unsafe`) para entregar um ponteiro cru, e a responsabilidade de manter as
invariantes passa a ser **sua, manualmente**. A disciplina deste capítulo é confinar esse
`unsafe` a uma camada fina e auditada, deixando 99% do núcleo seguro por construção.

## Subtópicos

### Boxed pointer usage

- `Box<T>` aloca no heap e dá um ponteiro de posse única — a base para entregar objetos
  Rust ao Java como *handle opaco* (ver *Java Native Interface → Native memory allocation*).
- Padrão: `Box::into_raw(Box::new(x))` devolve um `*mut T` (passado como `jlong`); mais
  tarde `Box::from_raw(ptr)` reassume a posse para dropar. **Cada `into_raw` exige
  exatamente um `from_raw`** — nem zero (vaza), nem dois (double-free).
- O Java trata o handle como número opaco e nunca o desreferencia; ele só o devolve nas
  chamadas seguintes (`metodo(handle, ...)`).
- **Detalhe sutil de tipo:** o ponteiro precisa voltar como **exatamente o mesmo tipo `T`**
  no `from_raw`. Se você fizer `Box<dyn Trait>` (um *fat pointer*, com vtable), ele não cabe
  num único `jlong` — embrulhe num `Box<Box<dyn Trait>>` ou numa struct concreta. Trocar o
  tipo entre `into_raw` e `from_raw` é UB.

### Null pointer prevention

- Rust elimina null por design: referências (`&T`) nunca são nulas e a ausência é modelada
  com `Option<T>` — o que remove uma classe inteira de crashes (o "erro de bilhão de
  dólares").
- Na fronteira FFI, porém, ponteiros crus **podem** chegar nulos vindos do Java (handle não
  inicializado, já destruído, bug no Kotlin); valide-os antes de usar:

```rust
pub unsafe fn como_ref<'a, T>(h: jlong) -> Option<&'a T> {
    (h as *const T).as_ref()   // None se for null
}
```

- Use **`NonNull<T>`** internamente para *documentar e impor* a invariante "este ponteiro
  nunca é nulo" depois da validação de borda — o tipo carrega a garantia.
- Nunca faça `from_raw`/desreferência de um ponteiro sem checar a origem; trate `0`/null como
  erro recuperável (retorne, lance exceção) em vez de crashar.

### Memory leak detection

- Vazamentos ocorrem tipicamente quando: o Java perde o handle sem chamar o `destroy`; uma
  `GlobalRef` JNI não é liberada (ver *Java Native Interface → Global reference lifecycle*);
  ou um ciclo de `Arc` se forma (abaixo).
- **Importante:** vazar memória **não é `unsafe`** em Rust (`mem::forget`, ciclos de `Arc` e
  handles esquecidos são "seguros" pela definição da linguagem). O compilador **não** te
  protege de leaks — só de *unsafety*. Logo, leaks exigem ferramentas e disciplina.
- Ferramentas:
  - **Miri** — detecta UB e alguns leaks em testes no host (`cargo +nightly miri test`).
  - **AddressSanitizer/LeakSanitizer** via NDK (`-Z sanitizer` / HWASan no Android) para o
    `.so` no device.
  - **Memory Profiler** do Android Studio para acompanhar o heap nativo e detectar
    crescimento sem teto.
- Garanta liberação **determinística** amarrando o `destroy` nativo ao ciclo de vida do
  objeto Kotlin: implemente `AutoCloseable` e use `use { }`, ou deixe a UniFFI gerar o
  `Disposable` por você (ver *Automated Bindings → Foreign function declaration*).

### Reference counting implementation

- `Rc<T>` (single-thread) e `Arc<T>` (atômico, thread-safe) compartilham posse por contagem
  de referências; `Weak<T>` quebra ciclos que de outra forma vazariam.
- `Arc` é essencial para compartilhar estado entre as threads de trabalho do Rust e a
  fronteira FFI (ver *Data Synchronization*). Para Android, quase sempre é `Arc`, não `Rc`,
  porque o estado costuma ser tocado por mais de uma thread.
- **Ciclos vazam:** `Arc`↔`Arc` (A aponta para B e B para A) nunca chega a refcount zero →
  nenhum dos dois é liberado. Use `Weak` para o lado "de volta" (ex.: filho→pai, observador→
  sujeito):

```rust
struct No { pai: Weak<No>, filhos: Vec<Arc<No>> }  // pai é Weak: sem ciclo
```

- A UniFFI usa `Arc<Self>` para os `Object` exportados — entender refcount ajuda a raciocinar
  sobre quando o objeto nativo é realmente liberado (quando o último handle Kotlin fecha *e*
  o último `Arc` Rust some).

### Automatic drop trait execution

- O trait `Drop` roda automaticamente quando um valor sai de escopo — é o **RAII** do Rust:
  arquivos, sockets, locks e memória são liberados sem `free` manual, inclusive em caminhos
  de erro (`?`, early return) e durante *unwinding* de panic.
- Cuidado: ao usar `Box::into_raw`/`mem::forget`, o `Drop` **não** roda — a liberação passa
  a ser sua responsabilidade. Por isso todo handle exportado precisa de um par `destroy`.
- Implemente `Drop` em wrappers de recursos nativos para garantir limpeza:

```rust
struct ArquivoTemp { caminho: PathBuf }
impl Drop for ArquivoTemp {
    fn drop(&mut self) { let _ = std::fs::remove_file(&self.caminho); }
}
```

- **Ordem de drop:** campos de uma struct são dropados na ordem de declaração; variáveis
  locais, na ordem inversa de criação. Importa quando há dependência (ex.: um lock que
  protege um recurso deve ser solto depois do recurso).
- `Drop` **não pode** ser chamado manualmente (`x.drop()` não compila); use
  `std::mem::drop(x)` para liberar cedo.

### Zero cost abstraction safety

- As abstrações de segurança do Rust (ownership, `Option`, iterators, RAII, generics
  monomorfizados) são **zero-cost**: compilam para o mesmo código que C escrito à mão, sem
  overhead de runtime nem pausas de GC.
- Isso é decisivo em mobile: você ganha segurança de memória **e** desempenho previsível,
  sem pausas de coletor que prejudicariam frame rate ou latência de áudio — diferentemente da
  JVM, onde o GC pode introduzir *stutter* em momentos ruins (ver *Performance
  Optimization*).
- "Zero-cost" tem nuances: `Arc` paga atomics em clone/drop; `Box`/`Vec` pagam alocação;
  `dyn Trait` paga indireção de vtable. São custos *explícitos e escolhidos*, não impostos
  por um runtime — você só paga pelo que usa.
- **Mantenha o `unsafe` confinado** a pequenos blocos bem auditados na fronteira FFI; todo o
  resto do núcleo permanece seguro por construção. Documente cada `unsafe` com um comentário
  `// SAFETY: ...` explicando por que as invariantes são respeitadas — é a convenção da
  comunidade e ajuda revisões e auditoria.

## Armadilhas comuns

- **`from_raw` sem `into_raw` correspondente** (ou com tipo diferente) — UB. Centralize
  create/destroy num único módulo de fronteira e teste o par com Miri.
- **`destroy` chamado duas vezes** — double-free. Zere o handle no Kotlin após `close()` e
  cheque `== 0` no Rust (ver *Java Native Interface → Native memory allocation*).
- **Ciclo de `Arc`** — leak silencioso, sem crash, só memória subindo. Use `Weak` no lado de
  volta; cace com o Memory Profiler.
- **Guardar `&T` além do tempo de vida do dado via ponteiro cru** — o borrow checker não vê
  através do `*const T`; um use-after-free pode passar despercebido. Minimize a janela e
  reanexe a um lifetime explícito o quanto antes.

## Assuntos correlatos

- **Handles e a mecânica do `jlong`:** *Java Native Interface → Native memory allocation*.
- **`Arc`/`Weak` no contexto de threads e compartilhamento:** *Data Synchronization →
  Reference counting / Lock free*.
- **`Drop` para liberar recursos GPU/surface:** *Rendering Integration*.
- **`unsafe` e UB em geral:** o Rustonomicon é a referência canônica.

## Referências

- Ownership (The Rust Book): https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html
- Rc/Arc: https://doc.rust-lang.org/book/ch15-04-rc.html
- The Rustonomicon (unsafe): https://doc.rust-lang.org/nomicon/
- Miri: https://github.com/rust-lang/miri
