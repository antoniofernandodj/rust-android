# Cross Platform Logic

> **Seção:** Platform Abstraction
> **Status:** ✅ preenchido

## Visão geral

A motivação central de usar Rust em mobile é o **núcleo compartilhado**: escrever a lógica de
negócio uma única vez em Rust e reaproveitá-la em Android, iOS, desktop e web (WASM). Para
isso a arquitetura precisa **isolar a lógica pura** das particularidades de cada plataforma
(UI, I/O, APIs do SO) atrás de interfaces. Esta etapa cobre os padrões que tornam o mesmo
crate portável sem `#[cfg]` espalhado por toda parte.

O caso de negócio é forte: equipes como a da Mozilla (Firefox/app-services), Signal,
1Password, Dropbox e outras compartilham 70–90% do código entre plataformas escrevendo o
núcleo em Rust. O ganho não é só "menos código": é **uma única fonte da verdade para a
lógica**, testada uma vez, com comportamento idêntico em todo lugar — sem o clássico bug que
existe só no Android porque a regra foi reimplementada de leve diferente em Kotlin e em
Swift.

A arquitetura é, em essência, uma **arquitetura hexagonal (ports & adapters)** aplicada a
linguagens:

```
        ┌──────────── núcleo Rust (lógica pura) ────────────┐
        │  modelos · regras · fluxos · validação            │
        │  ↑ define traits (ports): Storage, Net, Clock…     │
        └───────────────────────────────────────────────────┘
              ▲                ▲                 ▲
        Android (Kotlin)   iOS (Swift)    Desktop/CLI (Rust)
        injeta adapters    injeta adapters    injeta adapters
```

O núcleo define *o que* precisa do mundo (traits); cada plataforma fornece *como* (impls).
O núcleo nunca importa nada específico de plataforma.

## Subtópicos

### Shared logic layer architecture

- Estruture em camadas: um **crate de núcleo** com a lógica de domínio (regras, modelos,
  fluxos) que **não conhece** Android nem iOS, e crates finos de *binding* por plataforma
  (ver *Automated Bindings → Interface abstraction layers*).
- O núcleo expõe uma API de alto nível; cada plataforma fornece sua UI e injeta as
  capacidades do SO. Assim 80–90% do código é compartilhado e testável em qualquer host.
- Organização típica em workspace:

```
workspace/
├── core/        # lógica pura, zero deps de plataforma — testável com `cargo test`
├── platform/    # traits (ports) + tipos de domínio compartilhados
├── android/     # crate cdylib: bindings UniFFI/JNI + impls Android (via JNI)
├── ios/         # crate staticlib: bindings + impls iOS
└── desktop/     # binário de teste/dev que roda o núcleo no host
```

- Regra prática: se você precisa de um `use jni::...` ou `#[cfg(target_os=...)]` no crate
  `core`, algo vazou — empurre para um crate de fronteira.

### Platform specific backend injection

- Capacidades que variam por plataforma (storage seguro, rede, relógio, geolocalização,
  logging) são definidas como **traits** no núcleo e **implementadas** por cada plataforma,
  sendo injetadas no núcleo (inversão de dependência / dependency injection):

```rust
// no núcleo: define o "port"
pub trait SecureStorage: Send + Sync {
    fn get(&self, chave: &str) -> Option<String>;
    fn set(&self, chave: &str, valor: &str);
}

// o núcleo recebe a capacidade por construção, sem saber a impl concreta
pub struct App { storage: Arc<dyn SecureStorage> }
impl App {
    pub fn new(storage: Arc<dyn SecureStorage>) -> Self { Self { storage } }
}
// Android injeta uma impl que usa o Keystore via JNI;
// iOS injeta uma que usa o Keychain;
// os testes injetam um HashMap em memória.
```

- Com **UniFFI**, essas traits viram **callback interfaces**: o Kotlin implementa
  `SecureStorage` chamando o Keystore e passa a instância ao construtor do núcleo (ver
  *Automated Bindings* e *Cryptographic Implementation → Hardware backed keystore access*).
- O trait exige `Send + Sync` porque a impl será chamada de threads do núcleo.

### Feature flag management

- As **features** do Cargo ligam/desligam funcionalidades em tempo de compilação
  (`[features]` no `Cargo.toml`), permitindo builds enxutos por plataforma ou por edição
  (free/pro):

```toml
[features]
default = []
android = ["dep:jni"]
desktop-mock = []     # impls fake para rodar no host
telemetry = ["dep:opentelemetry"]
```

- Combine com flags de **runtime** (config remota / *feature flags* tipo LaunchDarkly) para
  experimentos e *rollout* gradual — features de Cargo são estáticas, decididas no build;
  flags de runtime são dinâmicas.
- Mantenha as features **aditivas**: ligar uma feature nunca deve *remover* ou quebrar
  funcionalidade de outra, pois o Cargo unifica features de todos os consumidores num build
  (o "feature unification"). Violar isso causa builds que funcionam isolados mas quebram
  combinados.

### Conditional compilation attributes

- `#[cfg(target_os = "android")]`, `#[cfg(feature = "...")]`, `#[cfg(target_arch = "...")]`
  e a macro `cfg!()` selecionam código por plataforma/feature. Útil para os poucos pontos
  genuinamente específicos.
- **Boa prática:** concentrar os `#[cfg]` em **módulos de fronteira bem delimitados**
  (`platform/android.rs`, `platform/ios.rs`) com a *mesma assinatura pública*, não
  pulverizá-los pela lógica de domínio — caso contrário a portabilidade vira ilusão e o
  núcleo fica ilegível.

```rust
// platform/mod.rs
#[cfg(target_os = "android")] mod android;
#[cfg(target_os = "android")] pub use android::logger;
#[cfg(not(target_os = "android"))] mod stub;
#[cfg(not(target_os = "android"))] pub use stub::logger;
```

- Prefira **traits + injeção** (acima) a `#[cfg]` sempre que possível: o `#[cfg]` é resolvido
  no build e não é testável no host, enquanto uma trait com impl mock roda em `cargo test`.

### Interface oriented design

- Programe contra **traits/interfaces**, não contra implementações concretas. Isso permite
  trocar o backend por plataforma e usar *mocks* nos testes (ver *Unit Testing → Mocking
  native dependencies*).
- A fachada pública (a API exportada via UniFFI/JNI) deve ser orientada a **casos de uso de
  alto nível** ("fazer login", "sincronizar"), escondendo detalhes — o que também estabiliza
  a ABI entre versões e minimiza a superfície de FFI.
- Evite vazar tipos de implementação na fachada (ex.: um `tokio::Runtime` ou um tipo de uma
  dependência interna); exponha tipos de domínio próprios e estáveis.

### Portability testing strategies

- Rode a suíte de testes do núcleo no **host** (CI x86_64/ARM64) — rápido e independente de
  emulador — e mantenha testes de integração por plataforma só para a camada de binding.
- Use CI **multiplataforma** (matriz Android/iOS/desktop) para pegar regressões de
  portabilidade cedo: às vezes um código compila no host mas quebra no target Android (ex.:
  `usize` de 32 bits em `armv7`, diferenças de endianness, APIs de tempo). A matriz garante
  que *todas* as ABIs ao menos compilam (ver *Cross Compilation Targets → Armadilhas*).
- Estratégia em camadas:
  - **Host:** a maior parte (lógica, com mocks das traits).
  - **Cross-compile check:** `cargo ndk ... check` para cada ABI, garantindo compilação.
  - **Emulador/device:** só a fina camada FFI e o que depende de APIs reais (ver
    *Instrumented Testing*).

## Armadilhas comuns

- **`#[cfg]` espalhado** pela lógica de domínio — sintoma de abstração faltando; mata a
  testabilidade no host. Refatore para trait + injeção.
- **Feature não-aditiva** — uma feature que desliga algo essencial quebra builds combinados
  por causa da unificação de features do Cargo.
- **Dependência de plataforma vazando no núcleo** (`std::time::Instant` direto, paths
  absolutos, locale do SO) — injete `Clock`/`Storage` como traits para manter o núcleo puro e
  determinístico nos testes.
- **Assumir 64-bit** — `usize` é 32-bit em `armv7`; código que assume ponteiros/índices de
  64 bits quebra só nessa ABI.

## Assuntos correlatos

- **Bindings por plataforma:** *Automated Bindings* (UniFFI gera Kotlin *e* Swift da mesma
  fonte — o coração do compartilhamento).
- **Mocks de traits nos testes:** *Unit Testing → Mocking native dependencies*.
- **Injeção do Keystore como `SecureStorage`:** *Cryptographic Implementation*.
- **Estudo de caso real:** Mozilla application-services compartilha um núcleo Rust entre
  Firefox Android e iOS.

## Referências

- Cargo features: https://doc.rust-lang.org/cargo/reference/features.html
- Conditional compilation: https://doc.rust-lang.org/reference/conditional-compilation.html
- Estudo de caso (Mozilla, app-services): https://mozilla.github.io/application-services/
- Hexagonal architecture (ports & adapters): https://alistair.cockburn.us/hexagonal-architecture/
