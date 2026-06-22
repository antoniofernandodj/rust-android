# Package Management

> **Seção:** Deployment and Distribution
> **Status:** ✅ preenchido

## Visão geral

A etapa final é **empacotar e publicar** o app com seu código nativo Rust de forma otimizada
e segura. Isso envolve gerar o **Android App Bundle (AAB)** — formato exigido pela Play Store
— assinar o pacote, configurar ofuscação/redução de código, e gerenciar versionamento. Com
Rust no app, há cuidados extras com o tamanho dos `.so` por ABI e com a assinatura/integridade.

## Subtópicos

### Android App Bundle creation

- O **AAB** (`.aab`) é o formato de publicação na Play Store. O Google gera APKs otimizados
  por dispositivo a partir dele (**split APKs** por ABI, densidade e idioma) via *Play
  Asset/Dynamic Delivery*.
- Para apps com Rust isso é ótimo: cada usuário baixa **apenas o `.so` da sua ABI**, em vez
  de todas. Gere com `./gradlew bundleRelease`.

### Proguard rule configuration

- **R8/ProGuard** removem e ofuscam código Kotlin/Java não usado. É preciso **manter
  (`-keep`) as classes/métodos `external`** chamados via JNI, senão o link nativo quebra.

```proguard
# Mantém os métodos nativos e a classe que os declara
-keepclasseswithmembernames class * { native <methods>; }
-keep class com.exemplo.app.RustCore { *; }
```

- As classes geradas pela UniFFI também precisam de regras `-keep` apropriadas.

### Signature verification process

- Todo APK/AAB deve ser **assinado**. Use **Play App Signing**: você sobe com uma *upload
  key* e o Google assina com a chave final. Esquemas v2/v3/v4 verificam a integridade do
  pacote inteiro, incluindo os `.so`.
- Guarde o keystore com segurança (nunca no repo!). Internamente, o app pode ainda **verificar
  assinaturas de dados** com o núcleo Rust (ver *Signature verification protocols*).

### Play Store deployment steps

- Fluxo: criar app no **Play Console** → configurar ficha → subir o AAB em um *track*
  (internal → closed → open → production) → preencher classificação/política de dados →
  *rollout* (possivelmente gradual por %).
- Automatize com **Fastlane** (`supply`) ou a Play Developer API na CI. Trilhas internas
  permitem validar o build nativo em devices reais antes de produção.

### Binary obfuscation techniques

- O código Rust compilado já é difícil de reverter, e `strip` remove símbolos. Para reforçar:
  ocultar símbolos não-FFI (visibility), remover nomes de panic/backtrace em release, e
  evitar embutir segredos no binário (eles **não** são seguros mesmo ofuscados).
- Ofuscação dificulta engenharia reversa, mas **não substitui** segurança real (Keystore,
  validação server-side). Trate-a como defesa em profundidade.

### Versioning control systems

- Gerencie `versionCode` (inteiro crescente, exigido pela Play) e `versionName` (semântico,
  ex.: `1.4.2`). Mantenha-os sincronizados com a versão do crate Rust (`Cargo.toml`).
- Automatize o bump na CI (a partir de tags git) e registre o commit/hash no build para
  rastrear qual versão do núcleo Rust foi publicada. Use changelog e tags semânticas.

## Referências

- Android App Bundle: https://developer.android.com/guide/app-bundle
- Sign your app / Play App Signing: https://developer.android.com/studio/publish/app-signing
- R8/Shrink your code: https://developer.android.com/build/shrink-code
