# Unit Testing

> **Seção:** Testing Methodologies
> **Status:** ✅ preenchido

## Visão geral

A grande vantagem de testes para um núcleo Rust é que a **lógica pura roda no host** (sua
máquina/CI x86_64), sem emulador, com feedback em segundos. O `cargo test` cobre testes
unitários e de integração do crate; a fronteira FFI exige cuidados extras (mocks, testes de
borda). O alvo é manter a maior parte da cobertura no nível rápido do host e reservar o
emulador só para o que é genuinamente específico do Android.

## Subtópicos

### Rust cargo test suite

- Testes unitários ficam no mesmo arquivo, em `#[cfg(test)] mod tests`, e rodam com
  `cargo test`. Testes de integração ficam em `tests/`.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn soma_funciona() { assert_eq!(soma(2, 3), 5); }
}
```

- Rode no host (sem `--target` Android) para velocidade máxima. Use `cargo test --workspace`
  para cobrir todos os crates do projeto.

### Mocking native dependencies

- Dependências da plataforma (Keystore, rede, relógio) são abstraídas por **traits** (ver
  *Cross Platform Logic*) e substituídas por **mocks** nos testes.
- Crates: **`mockall`** (gera mocks de traits) e injeção manual de implementações fake.
  Assim o núcleo é testado sem Android real.

### FFI boundary testing

- A camada `extern "C"`/JNI é onde mora o `unsafe` — teste-a com atenção: round-trips de
  conversão de tipos, ponteiros nulos, e o par `create`/`destroy` (sem leaks/double-free).
- **Miri** (`cargo +nightly miri test`) detecta UB nessas regiões. Para UniFFI, há testes
  de fixture que exercitam os bindings gerados.

### Deterministic test scenarios

- Testes devem ser **determinísticos**: sem depender de relógio real, rede ou ordem de
  threads. Injete tempo (`Clock` trait), use *seeds* fixas para RNG e dados de entrada
  controlados.
- Flakiness em concorrência: use ferramentas como `loom` para explorar interleavings de
  threads de forma exaustiva em testes de estruturas concorrentes.

### Integration test coverage

- Testes de integração em `tests/` exercitam a **API pública** do crate como um consumidor
  externo — exatamente a fachada exposta ao Android.
- Meça cobertura com **`cargo-llvm-cov`** ou `tarpaulin`. Foque cobertura na lógica de
  domínio; não persiga 100% em código de cola gerado.

### Continuous integration pipelines

- Pipeline típico (GitHub Actions/GitLab CI):
  1. `cargo fmt --check` e `cargo clippy -- -D warnings` (lint).
  2. `cargo test --workspace` no host.
  3. (Opcional) build cross para os targets Android, garantindo que cada ABI compila.
  4. (Opcional) testes instrumentados em emulador (ver doc seguinte).
- Cache de `~/.cargo` e `target/` acelera bastante. Rode o passo de host em cada push e o
  emulador em merges/nightly por ser mais lento.

## Referências

- Testing (The Rust Book): https://doc.rust-lang.org/book/ch11-00-testing.html
- mockall: https://docs.rs/mockall
- cargo-llvm-cov: https://github.com/taiki-e/cargo-llvm-cov
