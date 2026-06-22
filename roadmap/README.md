# Android Rust Development — Roadmap

Baseado no roadmap **Android Rust Development** (roadmap.sh). Cada seção é uma pasta e
cada tópico (caixa amarela) é um documento aprofundado.

## Como ler

Cada documento segue a mesma estrutura, do conceito ao detalhe prático:

- **Visão geral** — o porquê do tópico, o contexto e o modelo mental que organiza o resto.
- **Subtópicos** — cada um com explicação, exemplo de código e os trade-offs envolvidos.
- **Armadilhas comuns** — os erros que de fato acontecem na prática e como evitá-los.
- **Assuntos correlatos** — ligações com outros capítulos (o livro é fortemente interligado).
- **Referências** — fontes primárias para se aprofundar.

A linha condutora do livro é a arquitetura recomendada: um **núcleo Rust puro e portável**
(lógica, testável no host) exposto por uma **fachada fina via UniFFI/JNI** a uma **UI fina em
Compose** — com o Rust como única fonte da verdade. Os capítulos podem ser lidos em ordem
(toolchain → fundamentos → FFI → integração → … → distribuição) ou de forma avulsa, seguindo
os links de *Assuntos correlatos*.

## Índice

### 01 — Rust Toolchain Setup
- [Cross Compilation Targets](01-rust-toolchain-setup/01-cross-compilation-targets.md)
- [Build Environment](01-rust-toolchain-setup/02-build-environment.md)

### 02 — Android Fundamentals
- [Application Lifecycle](02-android-fundamentals/01-application-lifecycle.md)
- [UI Layer Basics](02-android-fundamentals/02-ui-layer-basics.md)

### 03 — FFI and Bindings
- [Java Native Interface](03-ffi-and-bindings/01-java-native-interface.md)
- [Automated Bindings](03-ffi-and-bindings/02-automated-bindings.md)

### 04 — Native Library Integration
- [Shared Object Creation](04-native-library-integration/01-shared-object-creation.md)
- [Gradle Build Integration](04-native-library-integration/02-gradle-build-integration.md)

### 05 — State Management
- [Data Synchronization](05-state-management/01-data-synchronization.md)

### 06 — Memory Safety
- [Resource Ownership](06-memory-safety/01-resource-ownership.md)

### 07 — Platform Abstraction
- [Cross Platform Logic](07-platform-abstraction/01-cross-platform-logic.md)

### 08 — Security and Encryption
- [Cryptographic Implementation](08-security-and-encryption/01-cryptographic-implementation.md)

### 09 — Compute Heavy Processing
- [Performance Optimization](09-compute-heavy-processing/01-performance-optimization.md)

### 10 — Testing Methodologies
- [Unit Testing](10-testing-methodologies/01-unit-testing.md)
- [Instrumented Testing](10-testing-methodologies/02-instrumented-testing.md)

### 11 — GUI with Rust
- [Rendering Integration](11-gui-with-rust/01-rendering-integration.md)

### 12 — Deployment and Distribution
- [Package Management](12-deployment-and-distribution/01-package-management.md)
