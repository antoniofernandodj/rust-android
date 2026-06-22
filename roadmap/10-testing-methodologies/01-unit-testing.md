# Unit Testing

> **Seção:** Testing Methodologies
> **Status:** ✅ preenchido

## Visão geral

A grande vantagem de testes para um núcleo Rust é que a **lógica pura roda no host** (sua
máquina/CI x86_64 ou ARM64), sem emulador, com feedback em segundos. O `cargo test` cobre
testes unitários e de integração do crate; a fronteira FFI exige cuidados extras (mocks,
testes de borda, detecção de UB). O alvo é manter a maior parte da cobertura no nível rápido
do host e reservar o emulador só para o que é genuinamente específico do Android.

Pense numa **pirâmide de testes** adaptada a apps Rust+Android, da base (muitos, rápidos,
baratos) ao topo (poucos, lentos, caros):

```
            ▲  poucos / lentos / caros
   /  UI instrumentada (emulador)  \      ← só o fluxo de ponta a ponta
  /   integração FFI / bindings     \     ← round-trips, leaks, no device
 /  testes de integração do núcleo   \    ← API pública no host
/   testes unitários da lógica pura    \  ← a maioria, no host, em ms
            ▼  muitos / rápidos / baratos
```

Quanto mais peso na base (host), mais rápido o CI e mais cedo os bugs aparecem. O emulador
(ver *Instrumented Testing*) é caro — use-o só para o que *só* pode ser validado no Android.

## Subtópicos

### Rust cargo test suite

- Testes unitários ficam no mesmo arquivo do código, em `#[cfg(test)] mod tests`, e têm
  acesso a itens privados do módulo (`use super::*`). Testes de integração ficam em `tests/`
  e veem só a API pública.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soma_funciona() {
        assert_eq!(soma(2, 3), 5);
    }

    #[test]
    fn divisao_por_zero_retorna_erro() {
        assert!(divide(1, 0).is_err());
    }

    #[test]
    #[should_panic(expected = "índice fora")]
    fn indice_invalido_entra_em_panic() {
        acessa(&[], 5);
    }
}
```

- Rode no host (sem `--target` Android) para velocidade máxima. Use `cargo test --workspace`
  para cobrir todos os crates do projeto, e `cargo nextest run` para uma execução paralela e
  com saída melhor em projetos grandes.
- Para casos parametrizados, `rstest` reduz boilerplate; para gerar entradas aleatórias e
  encontrar contra-exemplos, **property-based testing** com `proptest`/`quickcheck` é
  poderoso (ótimo para parsers, serialização *round-trip*, invariantes).

### Mocking native dependencies

- Dependências da plataforma (Keystore, rede, relógio, geolocalização) são abstraídas por
  **traits** (ver *Cross Platform Logic → Platform specific backend injection*) e
  substituídas por **mocks/fakes** nos testes — é o que permite testar o núcleo inteiro sem
  Android real.
- Crates: **`mockall`** (gera mocks de traits, com expectativas sobre chamadas), e a injeção
  manual de implementações *fake* (ex.: um `HashMap` no lugar do Keystore):

```rust
struct StorageFake(std::sync::Mutex<std::collections::HashMap<String, String>>);
impl SecureStorage for StorageFake {
    fn get(&self, k: &str) -> Option<String> { self.0.lock().unwrap().get(k).cloned() }
    fn set(&self, k: &str, v: &str) { self.0.lock().unwrap().insert(k.into(), v.into()); }
}

#[test]
fn login_persiste_token() {
    let app = App::new(Arc::new(StorageFake::default()));
    app.login("ana", "senha");
    assert!(app.token().is_some());
}
```

- *Fakes* simples (impls em memória) costumam ser mais legíveis e robustos que mocks com
  expectativas; use `mockall` quando precisa **verificar interações** (que tal método foi
  chamado, com quais args, quantas vezes).

### FFI boundary testing

- A camada `extern "C"`/JNI é onde mora o `unsafe` — teste-a com atenção especial:
  - **Round-trips de conversão** de tipos (String UTF-8↔UTF-16, structs↔Record).
  - **Ponteiros nulos** e handles inválidos passados de propósito.
  - O par **`create`/`destroy`** sem *leaks* nem *double-free* (ver *Resource Ownership*).
- **Miri** (`cargo +nightly miri test`) interpreta o código detectando UB
  (use-after-free, *out-of-bounds*, *unaligned access*, violação de aliasing) — rode-o sobre
  a lógica de fronteira. Limitação: Miri não atravessa `extern` real nem chama a JVM, então
  teste a *mecânica de ponteiros* em Rust puro, isolada da JVM.
- Para UniFFI, há o padrão de **fixtures**: você escreve testes em Kotlin/Python que
  exercitam os bindings gerados, validando que a tradução de tipos funciona ponta a ponta.

### Deterministic test scenarios

- Testes devem ser **determinísticos**: sem depender de relógio real, rede, sistema de
  arquivos global ou ordem de threads. Caso contrário viram *flaky* (falham
  intermitentemente) e a equipe começa a ignorá-los.
- Técnicas:
  - **Injete o tempo** — um trait `Clock` com impl real em produção e uma `FakeClock`
    controlável nos testes (nada de `SystemTime::now()` espalhado).
  - **Seeds fixas** para RNG nos testes (mas `OsRng` em produção para cripto — ver
    *Cryptographic Implementation*).
  - **`tempfile`** para isolar I/O de disco em diretórios temporários descartáveis.
- Flakiness em **concorrência**: use **`loom`** para explorar exaustivamente os
  *interleavings* de threads em testes de estruturas concorrentes (ver *Data Synchronization
  → Lock free concurrency*) — ele encontra deadlocks/races que um teste normal só pegaria 1
  em 10000 execuções.

### Integration test coverage

- Testes de integração em `tests/` exercitam a **API pública** do crate como um consumidor
  externo — exatamente a fachada exposta ao Android (ver *Cross Platform Logic → Interface
  oriented design*). São o melhor proxy, rodando no host, para "o app vai funcionar".
- Meça cobertura com **`cargo-llvm-cov`** (preciso, baseado em instrumentação do LLVM) ou
  `tarpaulin`. Gere relatório HTML/LCOV e publique no CI.
- **Foque cobertura na lógica de domínio**; não persiga 100% em código de cola gerado
  (UniFFI) nem em `Drop`/handlers de erro triviais. Cobertura é um sinal, não uma meta —
  100% de cobertura com asserções fracas não prova nada.

### Continuous integration pipelines

- Pipeline típico (GitHub Actions/GitLab CI):
  1. `cargo fmt --check` e `cargo clippy --all-targets -- -D warnings` (formatação + lint;
     o Clippy pega muitos bugs reais, não só estilo).
  2. `cargo test --workspace` (ou `cargo nextest run`) no host.
  3. `cargo +nightly miri test` na camada de fronteira (opcional mas valioso).
  4. **Cross-compile check** dos targets Android (`cargo ndk -t arm64-v8a -t x86_64 check`),
     garantindo que cada ABI compila — pega regressões de portabilidade (ver *Cross
     Compilation Targets*).
  5. (Opcional) testes instrumentados em emulador (ver doc seguinte) e cobertura.
- **Cache** de `~/.cargo/registry`, `~/.cargo/git` e `target/` acelera muito (ver *Build
  Environment*). Rode os passos rápidos (1–3) em cada push e os lentos (emulador, matriz
  completa de ABIs) em merges/nightly.
- Considere `cargo-deny` (auditoria de licenças e vulnerabilidades de dependências) e
  `cargo-audit` (RustSec advisory DB) como gates de segurança da cadeia de dependências.

## Armadilhas comuns

- **Testes flaky por tempo/threads** — injete `Clock`, use seeds fixas, `loom` para
  concorrência. Um teste flaky ignorado é pior que nenhum teste.
- **Testar só o caminho feliz** — adicione casos de erro, entradas vazias, limites
  (overflow, índices), e dados malformados (importante em parsers).
- **`unwrap()` em testes mascarando o real erro** — use `assert!`/`expect("msg")` com
  mensagens que expliquem a expectativa.
- **Cobertura como meta vaidosa** — 100% sem boas asserções é falso conforto. Meça, mas mire
  qualidade das asserções.
- **Rodar Miri sobre código que chama a JVM** — Miri não simula a JVM; isole a mecânica de
  ponteiros.

## Assuntos correlatos

- **Onde os mocks vêm:** as traits de plataforma de *Cross Platform Logic*.
- **O que NÃO dá para testar no host** (carregamento do `.so`, JNI real, UI): *Instrumented
  Testing*.
- **Benchmarks (≠ testes de correção):** `criterion` em *Performance Optimization →
  Profiling*.
- **Determinismo de cripto/RNG:** *Cryptographic Implementation* (seed fixa em teste, `OsRng`
  em produção).

## Referências

- Testing (The Rust Book): https://doc.rust-lang.org/book/ch11-00-testing.html
- mockall: https://docs.rs/mockall
- cargo-llvm-cov: https://github.com/taiki-e/cargo-llvm-cov
- proptest: https://github.com/proptest-rs/proptest
- cargo-nextest: https://nexte.st/
