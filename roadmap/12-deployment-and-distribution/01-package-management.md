# Package Management

> **Seção:** Deployment and Distribution
> **Status:** ✅ preenchido

## Visão geral

A etapa final é **empacotar e publicar** o app com seu código nativo Rust de forma otimizada
e segura. Isso envolve gerar o **Android App Bundle (AAB)** — formato exigido pela Play Store
— assinar o pacote, configurar ofuscação/redução de código, e gerenciar versionamento. Com
Rust no app, há cuidados extras com o **tamanho dos `.so` por ABI**, com a integridade da
assinatura e com a preservação dos símbolos para depurar crashes nativos.

Vale entender a diferença entre os formatos, porque ela muda tudo para apps com vários `.so`:

- **APK** — o pacote instalável. Um **APK universal** contém os `.so` de *todas* as ABIs →
  pesado.
- **AAB** — não é instalável; é o que você sobe na Play Store. O Google gera, a partir dele,
  **APKs otimizados por device** (split por ABI, densidade e idioma). O usuário baixa só o
  `.so` da ABI dele. Para apps com Rust, isso pode cortar pela metade ou mais o tamanho de
  download.

A regra: **publique AAB na Play Store**; gere APK só para distribuição direta (sideload, CI,
lojas alternativas) e, mesmo aí, prefira splits por ABI a um universal.

## Subtópicos

### Android App Bundle creation

- O **AAB** (`.aab`) é o formato de publicação na Play Store. O Google gera APKs otimizados
  por dispositivo a partir dele (**split APKs** por ABI, densidade e idioma) via *Play
  Dynamic Delivery*.
- Para apps com Rust isso é ótimo: cada usuário baixa **apenas o `.so` da sua ABI**, em vez
  de todas. Gere com `./gradlew bundleRelease`.
- Teste o que o usuário **realmente** recebe usando o **`bundletool`** para gerar e instalar
  os APKs de um device específico a partir do `.aab` — assim você valida que o `.so` certo
  está presente e funciona, antes de publicar:

```bash
bundletool build-apks --bundle=app.aab --output=app.apks \
  --connected-device   # gera só para o device conectado
bundletool install-apks --apks=app.apks
```

- **Dynamic Feature Modules** permitem entregar partes do app (inclusive `.so` grandes, ex.:
  um modelo de ML pesado) **sob demanda**, reduzindo o tamanho do download inicial.

### Proguard rule configuration

- **R8/ProGuard** removem e ofuscam código Kotlin/Java não usado. É preciso **manter
  (`-keep`) as classes/métodos `external`** chamados via JNI, senão o link nativo quebra em
  runtime (o método nativo é "achado" pelo nome — se o R8 renomeia a classe/método, o
  `Java_...` no `.so` não casa mais):

```proguard
# Mantém todos os métodos nativos e a classe que os declara
-keepclasseswithmembernames class * { native <methods>; }

# Mantém a fachada chamada pela JVM (ajuste ao seu pacote)
-keep class com.exemplo.app.RustCore { *; }
```

- As classes **geradas pela UniFFI** também precisam de regras `-keep` apropriadas (a
  documentação da UniFFI fornece o bloco pronto) — esquecer isso gera crashes só no build de
  release com minificação, que passam no debug (ver *Automated Bindings*).
- Bug clássico: "funciona em debug, crasha em release" — quase sempre é regra `-keep`
  faltando. Teste sempre o **build de release minificado** antes de publicar.

### Signature verification process

- Todo APK/AAB deve ser **assinado** — a assinatura prova a autoria e garante que o pacote
  não foi adulterado após a publicação. Use **Play App Signing**: você sobe com uma *upload
  key* e o Google reassina com a *app signing key* final (que ele guarda com segurança).
- Esquemas de assinatura: **v2/v3/v4** verificam a integridade do **pacote inteiro, incluindo
  os `.so`** (o v1, baseado em JAR, não cobria tudo — não dependa só dele). Isso significa que
  um atacante não consegue trocar seu `.so` por um malicioso sem invalidar a assinatura.
- Guarde o keystore/upload key **com segurança e backup** (nunca no repo! use secrets do CI).
  Perder a *upload key* é recuperável com o Google; perder a *signing key* sem Play App
  Signing era catastrófico (impossível atualizar o app) — mais uma razão para usar Play App
  Signing.
- **Não confundir** com assinar *dados* dentro do app (Ed25519, validar payloads) — isso é o
  *Cryptographic Implementation → Signature verification protocols*; aqui é a assinatura do
  **pacote**.

### Play Store deployment steps

- Fluxo: criar app no **Play Console** → configurar ficha (descrição, capturas) → subir o AAB
  em um *track* → preencher classificação etária, **política de privacidade e Data Safety**
  (declarar o que o app coleta) → *rollout*.
- **Tracks** permitem validar antes de produção: `internal` (rápido, poucos testers) →
  `closed` (alpha/beta fechado) → `open` (beta aberto) → `production`. Trilhas internas são
  ideais para **validar o build nativo em devices reais** (todas as ABIs carregam? sem crash
  nativo?) antes de chegar ao público.
- **Staged rollout:** lance para uma % dos usuários (ex.: 5% → 20% → 100%) e monitore o
  **Android Vitals** (taxa de crash/ANR, inclusive crashes nativos do `.so`); pause/role para
  trás se a taxa subir.
- Automatize com **Fastlane** (`supply`) ou a **Play Developer API** no CI para subir,
  promover entre tracks e publicar release notes sem cliques manuais.

### Binary obfuscation techniques

- O código Rust compilado já é difícil de reverter (sem metadados ricos como o bytecode JVM),
  e `strip` remove símbolos. Para reforçar:
  - Ocultar símbolos não-FFI (visibility/version script; ver *Shared Object Creation → Symbol
    visibility control*).
  - Remover nomes de panic/backtrace e mensagens em release (`panic_immediate_abort`).
  - **Não embutir segredos** no binário — eles **não** são seguros mesmo "ofuscados":
    qualquer um com o `.so` extrai strings/constantes (ver *Cryptographic Implementation →
    Armadilhas*).
- Ofuscação dificulta engenharia reversa, mas **não substitui** segurança real (Keystore,
  validação server-side, TLS). Trate-a como **defesa em profundidade**, não como a defesa.
- Para anti-tamper/anti-fraude sério, há a **Play Integrity API** (atesta que o app/dispositivo
  é genuíno) — uma camada server-side bem mais forte que qualquer ofuscação de cliente.

### Versioning control systems

- Gerencie `versionCode` (inteiro **estritamente crescente**, exigido pela Play a cada upload)
  e `versionName` (string semântica visível ao usuário, ex.: `1.4.2`). Mantenha-os
  sincronizados com a versão do crate Rust (`Cargo.toml`) para que "qual versão do núcleo está
  em produção?" tenha resposta clara.
- **Rastreabilidade do build nativo:** registre o **commit/hash git** e a versão do crate no
  build (ex.: via `build.rs` expondo uma constante, ou `vergen`), e logue-o no startup. Quando
  um crash nativo aparecer no Vitals, você sabe exatamente qual `.so` o gerou — e tem os
  símbolos arquivados daquele build (ver *Shared Object Creation → Strip debug symbols*).
- Automatize o *bump* na CI a partir de **tags git** (`v1.4.2` → `versionName`; um contador
  monotônico → `versionCode`), gere **changelog** das tags e suba os **símbolos nativos de
  debug** (`android.buildTypes.release.ndk.debugSymbolLevel = "FULL"`) para o Play
  desimbolizar os crashes automaticamente.

## Armadilhas comuns

- **Crash só em release** por falta de `-keep` (R8 renomeou a classe/método nativo). Teste o
  release minificado antes de publicar.
- **Símbolos não arquivados/subidos** → crashes nativos no Vitals viram endereços hex
  inúteis. Configure `debugSymbolLevel` e/ou guarde o `.so` não-strippado por versão.
- **`versionCode` não crescente** — a Play recusa o upload. Automatize o incremento.
- **APK universal pesado** quando bastava AAB — usuários baixam ABIs que não usam. Publique
  AAB.
- **Segredo embutido no `.so`** achando que está protegido — extraível; mova para
  servidor/Keystore.
- **Keystore no repositório** — comprometimento da chave. Use secrets do CI + Play App
  Signing.

## Assuntos correlatos

- **Tamanho e símbolos do `.so`:** *Shared Object Creation* (strip, visibility, size flags).
- **Quais ABIs empacotar:** *Gradle Build Integration → ABI filter specification*.
- **Sincronizar `versionName` com a tag e o crate:** *Build Environment* (reprodutibilidade).
- **Segurança real vs. ofuscação:** *Cryptographic Implementation* (modelo de ameaça).
- **Símbolos para crashes:** *Instrumented Testing → Native crash analysis*.

## Referências

- Android App Bundle: https://developer.android.com/guide/app-bundle
- bundletool: https://developer.android.com/tools/bundletool
- Sign your app / Play App Signing: https://developer.android.com/studio/publish/app-signing
- R8/Shrink your code: https://developer.android.com/build/shrink-code
- Play Integrity API: https://developer.android.com/google/play/integrity
