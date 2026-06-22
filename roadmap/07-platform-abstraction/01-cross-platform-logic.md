# Cross Platform Logic

> **Seção:** Platform Abstraction
> **Status:** ✅ preenchido

## Visão geral

A motivação central de usar Rust em mobile é o **núcleo compartilhado**: escrever a lógica de
negócio uma única vez em Rust e reaproveitá-la em Android, iOS, desktop e web. Para isso a
arquitetura precisa **isolar a lógica pura** das particularidades de cada plataforma (UI,
I/O, APIs do SO) atrás de interfaces. Esta etapa cobre os padrões que tornam o mesmo crate
portável sem `#[cfg]` espalhado por toda parte.

## Subtópicos

### Shared logic layer architecture

- Estruture em camadas: um **crate de núcleo** com a lógica de domínio (regras, modelos,
  fluxos) que **não conhece** Android nem iOS, e crates finos de *binding* por plataforma.
- O núcleo expõe uma API de alto nível; cada plataforma fornece sua UI e injeta as
  capacidades do SO. Assim 80–90% do código é compartilhado e testável em qualquer host.

### Platform specific backend injection

- Capacidades que variam por plataforma (storage seguro, rede, relógio, geolocalização)
  são definidas como **traits** no núcleo e **implementadas** por cada plataforma, sendo
  injetadas no núcleo (inversão de dependência).

```rust
pub trait SecureStorage: Send + Sync {
    fn get(&self, chave: &str) -> Option<String>;
    fn set(&self, chave: &str, valor: &str);
}
// Android injeta uma impl que usa o Keystore via JNI;
// iOS injeta uma que usa o Keychain.
```

### Feature flag management

- As **features** do Cargo ligam/desligam funcionalidades em tempo de compilação
  (`[features]` no `Cargo.toml`), permitindo builds enxutos por plataforma ou por edição
  (free/pro).
- Combine com flags de runtime (config remota) para experimentos. Mantenha as features
  *aditivas* (ligar uma feature nunca deve quebrar outra), conforme recomenda o Cargo.

### Conditional compilation attributes

- `#[cfg(target_os = "android")]`, `#[cfg(feature = "...")]` e `cfg!()` selecionam código
  por plataforma/feature. Útil para os poucos pontos genuinamente específicos.
- **Boa prática:** concentrar os `#[cfg]` em módulos de fronteira bem delimitados, não
  pulverizá-los pela lógica de domínio — caso contrário a portabilidade vira ilusão.

### Interface oriented design

- Programe contra **traits/interfaces**, não contra implementações concretas. Isso permite
  trocar o backend por plataforma e usar *mocks* nos testes.
- A fachada pública (a API exportada via UniFFI/JNI) deve ser orientada a casos de uso de
  alto nível, escondendo detalhes — o que também estabiliza a ABI entre versões.

### Portability testing strategies

- Rode a suíte de testes do núcleo no **host** (CI x86_64) — rápido e independente de
  emulador — e mantenha testes de integração por plataforma para a camada de binding.
- Use CI multiplataforma (matriz Android/iOS/desktop) para pegar regressões de portabilidade
  cedo. Ferramentas como `cargo test` no host cobrem a maior parte; o emulador Android valida
  só a fina camada FFI.

## Referências

- Cargo features: https://doc.rust-lang.org/cargo/reference/features.html
- Conditional compilation: https://doc.rust-lang.org/reference/conditional-compilation.html
- Estudo de caso (Mozilla, app-services): https://mozilla.github.io/application-services/
