# Planejamento: `rustandroid` CLI

> Objetivo: `cargo install rustandroid` instala uma ferramenta de linha de
> comando que permite criar e gerenciar projetos Android em Rust com um único
> comando.

```
$ rustandroid new meu-app
$ cd meu-app
$ rustandroid run
```

---

## Visão da experiência final

```
$ rustandroid new meu-app
✔ Nome do pacote Android: com.exemplo.meuapp
✔ Periféricos: câmera, bateria, bluetooth
✔ UI: iced (padrão) / sem UI (biblioteca)

Gerando projeto...
  create meu-app/Cargo.toml
  create meu-app/src/lib.rs
  create meu-app/android/AndroidManifest.xml
  create meu-app/Makefile
  create meu-app/README.md

✅ Projeto criado. Próximos passos:
   cd meu-app
   rustandroid setup    ← instala SDK/NDK se necessário
   rustandroid run      ← compila e instala no celular
```

---

## Arquitetura

O projeto se divide em três partes publicadas no crates.io:

```
crates.io
├── rustandroid          ← CLI principal (cargo install rustandroid)
├── rustandroid-core     ← SDK runtime (o que vai dentro do app)
│   ├── android-battery
│   ├── android-sensors
│   └── ...
└── rustandroid-macros   ← proc-macros (android-jni-bridge)
```

### Por que separar `rustandroid` (CLI) de `rustandroid-core` (runtime)?

O nome `rustandroid` no crates.io será ocupado pelo CLI. O SDK que vai dentro
dos apps usa o nome `rustandroid-core`. O usuário instala o CLI uma vez na
máquina e declara `rustandroid-core` como dependência em cada projeto.

```toml
# Cargo.toml de um projeto gerado pelo CLI
[dependencies]
rustandroid-core = { version = "0.1", features = ["battery", "sensors"] }
```

---

## Comandos planejados

### `rustandroid new <nome>`

Cria um novo projeto interativamente.

```
Opções:
  --package    ID do pacote Android  (ex: com.empresa.app)
  --features   Periféricos separados por vírgula (battery,sensors,camera…)
  --no-ui      Gera uma biblioteca sem UI iced (para FFI puro)
  --template   Template alternativo (default: app-iced)
```

Gera a estrutura:
```
meu-app/
├── Cargo.toml
├── Cargo.lock
├── Makefile
├── src/
│   └── lib.rs              ← app mínimo com iced
├── android/
│   └── AndroidManifest.xml ← com permissões dos periféricos escolhidos
├── desktop/
│   ├── Cargo.toml
│   └── src/main.rs         ← runner desktop para dev sem celular
└── .cargo/
    └── config.toml         ← linker configs para cross-compile Android
```

---

### `rustandroid setup`

Detecta e instala o que falta no ambiente.

```
Verificando ambiente...
  Java 17           ✅ já instalado
  Android SDK       ✅ ~/android-sdk
  NDK 25.2          ✅ instalado
  aarch64 target    ✅ rustup
  cargo-apk         ❌ instalando...
  ANDROID_HOME      ❌ adicionando ao ~/.zshrc...

✅ Ambiente configurado.
```

Internamente: checa `JAVA_HOME`, `ANDROID_HOME`, `ANDROID_NDK_ROOT`,
`rustup target list`, `cargo apk --version`. Instala o que falta via
`sdkmanager`, `rustup target add` e `cargo install cargo-apk`.

---

### `rustandroid build` / `rustandroid release`

Wrappers sobre `cargo apk build` e `cargo apk build --release` que garantem
que as variáveis de ambiente estão corretas antes de invocar o cargo-apk.

```
Problema resolvido: sem isso o usuário precisa lembrar de exportar
ANDROID_HOME toda vez ou usar o Makefile. O CLI encapsula isso.
```

---

### `rustandroid run`

Build + install + launch em um único comando. Equivalente ao `make run` do
Makefile atual, mas sem precisar do Make.

```
rustandroid run [--device <serial>] [--release]
```

---

### `rustandroid add <periférico>`

Adiciona um periférico a um projeto existente.

```
$ rustandroid add camera bluetooth

Adicionando android-camera...
  ✏  Cargo.toml → features = ["camera"]
  ✏  AndroidManifest.xml → <uses-permission android:name="android.permission.CAMERA"/>
  ✏  src/lib.rs → exemplo mínimo de uso da câmera

✅ Periféricos adicionados.
```

---

### `rustandroid logcat`

Filtra o logcat pelo PID do app e colore por nível de log.
Resolve o problema de encontrar os logs do app num flood de mensagens Android.

```
rustandroid logcat [--crash]   ← --crash filtra só panics e fatais
```

---

### `rustandroid doctor`

Diagnóstico completo do ambiente. Útil para reportar problemas.

```
$ rustandroid doctor

rustandroid 0.2.0
  Rust          1.78.0
  cargo-apk     0.10.0
  Java          17.0.13 (SDKMAN)
  Android SDK   ~/android-sdk (API 33)
  NDK           25.2.9519653
  ADB           conectado — Pixel 7 (Android 14)
  Keystore      ~/meu-app/release.keystore ✅

Nenhum problema encontrado.
```

---

## Sistema de templates

Dois níveis de template:

### Nível 1 — Templates embutidos no binário

O CLI carrega templates simples via `include_str!` compilados diretamente no
binário. Sem dependências externas, funciona offline.

```
rustandroid-cli/
└── templates/
    ├── app-iced/
    │   ├── Cargo.toml.tmpl
    │   ├── src/lib.rs.tmpl
    │   └── android/AndroidManifest.xml.tmpl
    └── lib-only/
        ├── Cargo.toml.tmpl
        └── src/lib.rs.tmpl
```

O mecanismo de substituição é simples: `{{nome}}`, `{{pacote}}`,
`{{features}}`. Implementado com substituição de string, sem crate de template.

### Nível 2 — Templates externos via `cargo-generate`

Para templates mais elaborados (com lógica condicional por periférico), o CLI
pode delegar para `cargo-generate` se estiver instalado:

```sh
rustandroid new meu-app --template camera-ml
# internamente: cargo generate --git https://github.com/antoniofernandodj/rustandroid-templates
```

Os templates ficam num repositório separado
(`rustandroid-templates`) onde a comunidade pode contribuir.

---

## Publicação no crates.io

### Requisitos antes de publicar

1. **Separar os crates do workspace atual** em repositórios/crates publicáveis
2. **Versionar**: o `rustandroid-core` precisa de uma API estável antes de
   publicar (pelo menos `0.1.0`)
3. **Documentação**: `cargo doc` precisa gerar docs úteis para cada crate
4. **Testes**: ao menos testes unitários para as partes sem JNI

### Processo de publicação

```sh
# Ordem de publicação (dependências primeiro)
cargo publish -p rustandroid-macros   # proc-macros, sem deps internas
cargo publish -p android-battery      # crates de periférico
cargo publish -p android-sensors
# ... demais crates ...
cargo publish -p rustandroid-core     # facade que re-exporta tudo
cargo publish -p rustandroid          # CLI (depende dos acima)
```

### `rustandroid-core` como facade

O usuário não precisa conhecer os nomes internos dos crates. O `rustandroid-core`
re-exporta tudo com feature flags:

```toml
# Cargo.toml de um app usando o SDK publicado
[dependencies]
rustandroid-core = { version = "0.1", features = ["battery", "sensors", "push"] }
```

```rust
// lib.rs do app — imports limpos sem saber os crates internos
use rustandroid::battery::BatteryManager;
use rustandroid::sensors::{Sensor, SamplingRate};
use rustandroid::push::PushManager;
```

---

## Fases de implementação

### Fase A — CLI básico (fundação)

Crate binário `rustandroid` com:
- [ ] `rustandroid new` com template `app-iced` embutido
- [ ] `rustandroid setup` (detecta e instala dependências)
- [ ] `rustandroid build` / `rustandroid release` (wrapper com env vars)
- [ ] `rustandroid run` (build + install + launch)

**Resultado:** `cargo install rustandroid` + `rustandroid new meu-app` funciona.

### Fase B — SDK no crates.io

- [ ] Publicar `rustandroid-macros` (android-jni-bridge)
- [ ] Implementar JNI real nos periféricos restantes
- [ ] Publicar crates individuais de periférico
- [ ] Criar e publicar `rustandroid-core` como facade com feature flags
- [ ] Atualizar o template gerado pelo CLI para usar `rustandroid-core`

**Resultado:** `cargo install rustandroid-core` funciona para quem não quer o CLI.

### Fase C — CLI completo

- [ ] `rustandroid add <periférico>` (edita Cargo.toml + Manifest)
- [ ] `rustandroid doctor` (diagnóstico)
- [ ] `rustandroid logcat` (logcat filtrado e colorido)
- [ ] Sistema de templates externos via `cargo-generate`
- [ ] Repositório `rustandroid-templates` com templates da comunidade

### Fase D — Experiência polida

- [ ] Site de documentação (docs.rs + mdBook)
- [ ] CI/CD no GitHub Actions: testa geração do template + build do APK
- [ ] `rustandroid upgrade` — atualiza a versão do SDK num projeto existente
- [ ] Suporte a múltiplos targets de build (x86_64 para emulador, arm64 para device)

---

## Dependências do CLI

```toml
# rustandroid/Cargo.toml (o CLI)
[dependencies]
clap        = { version = "4", features = ["derive"] }  # parsing de args
dialoguer   = "0.11"   # prompts interativos (qual periférico usar?)
indicatif   = "0.17"   # barra de progresso durante download do SDK
console     = "0.15"   # cores no terminal
which       = "6"      # verifica se java, adb, etc. estão no PATH
dirs        = "5"      # $HOME, config dir multiplataforma
toml_edit   = "0.22"   # editar Cargo.toml sem destruir formatação
```

Sem dependência em `cargo-generate` para o caso básico — o CLI deve funcionar
com `cargo install rustandroid` sem etapas extras.

---

## Problema a resolver: `release.keystore`

O keystore de release não pode ser commitado no template (é um segredo).
Estratégia:

1. `rustandroid new` gera um keystore de **desenvolvimento** automaticamente
   com senha padrão e avisa que é só para testes
2. O `.gitignore` gerado inclui `*.keystore`
3. `rustandroid release --keystore caminho/para/prod.keystore` aceita um
   keystore externo para builds de produção
4. Variáveis de ambiente para CI: `RUSTANDROID_KEYSTORE_PATH`,
   `RUSTANDROID_KEYSTORE_PASS`, `RUSTANDROID_KEY_ALIAS`, `RUSTANDROID_KEY_PASS`

---

## Referências de projetos similares para estudar

| Projeto | O que copiar |
|---|---|
| `create-react-app` / `vite` | UX de scaffolding interativo |
| `cargo-generate` | Sistema de templates com variáveis |
| `flutter` CLI | `flutter create`, `flutter run`, `flutter doctor` |
| `tauri-cli` | CLI Rust que wrapa build nativo + web |
| `wasm-pack` | CLI Rust para publicar crates WASM no npm |
