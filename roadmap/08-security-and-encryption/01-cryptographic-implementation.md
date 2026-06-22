# Cryptographic Implementation

> **Seção:** Security and Encryption
> **Status:** ✅ preenchido

## Visão geral

Rust é uma excelente linguagem para **criptografia** em mobile: sem buffer overflows, com
crates auditados e desempenho nativo. O objetivo é proteger dados em repouso e em trânsito
**sem reinventar primitivas** — usar bibliotecas estabelecidas (`ring`, `RustCrypto`,
`rustls`) e delegar o armazenamento de chaves ao hardware seguro do dispositivo (Android
**Keystore**, respaldado por TEE/StrongBox). Regra de ouro: *don't roll your own crypto*.

## Subtópicos

### Symmetric encryption primitives

- Criptografia simétrica protege dados em repouso (cache, banco local). Use **AEAD**
  (autenticação + cifra) como **AES-256-GCM** ou **ChaCha20-Poly1305**.
- Crates: `aes-gcm`, `chacha20poly1305` (RustCrypto) ou `ring`. Gere um **nonce único** por
  mensagem (nunca reutilize com a mesma chave) e armazene-o junto ao ciphertext.

```rust
use chacha20poly1305::{aead::{Aead, KeyInit}, ChaCha20Poly1305, Nonce};
let cipher = ChaCha20Poly1305::new(chave.into());
let ct = cipher.encrypt(Nonce::from_slice(&nonce), texto.as_ref())?;
```

### Asymmetric key generation

- Criptografia assimétrica (par chave pública/privada) para troca de chaves e assinaturas:
  **Ed25519** (assinatura), **X25519** (key agreement), **RSA** (legado/interop).
- Crates: `ed25519-dalek`, `x25519-dalek`, `rsa`. Prefira curvas modernas (Curve25519) a
  RSA quando possível, por desempenho e simplicidade.
- A chave privada **nunca** deve sair do dispositivo; idealmente é gerada e mantida no
  Keystore (ver abaixo).

### Secure storage integration

- Dados sensíveis (tokens, chaves derivadas) devem ir para storage protegido:
  **EncryptedSharedPreferences**/**DataStore** com chave do Keystore, ou um arquivo cifrado
  pelo núcleo Rust.
- A chave de criptografia **não** fica no código nem em texto plano — é envolvida (*wrapped*)
  por uma chave do Keystore. O Rust cifra/decifra; o Keystore guarda a chave-mestra.

### Hardware backed keystore access

- O **Android Keystore** armazena chaves em hardware seguro (TEE ou StrongBox), onde elas
  **nunca são expostas** à memória do app — você só pede operações (assinar, decifrar).
- O Rust acessa o Keystore **via JNI**, chamando a API Java (`KeyStore`, `KeyGenParameterSpec`).
  É possível exigir autenticação biométrica (`setUserAuthenticationRequired`) para usar a chave.
- Esse é o pilar do modelo: chaves protegidas por hardware + criptografia feita em Rust.

### Message authentication codes

- **MAC** garante integridade e autenticidade: **HMAC-SHA256** (`hmac` + `sha2`) ou
  Poly1305 (já embutido nos AEAD acima).
- Compare MACs em **tempo constante** (`subtle::ConstantTimeEq`) para evitar *timing
  attacks*. Nunca use `==` comum para comparar segredos/tags.

### Signature verification protocols

- Assinaturas digitais verificam origem e integridade (ex.: validar payloads de servidor,
  atualizações, licenças). Use **Ed25519** para verificar rapidamente com a chave pública.
- Valide sempre o resultado de `verify(...)` e trate erro como rejeição total — não
  prossiga com dado não verificado.
- Para TLS/HTTPS, prefira **`rustls`** (TLS em Rust puro, memory-safe) a wrappers de
  OpenSSL quando viável no Android.

## Referências

- RustCrypto: https://github.com/RustCrypto
- ring: https://github.com/briansmith/ring
- Android Keystore: https://developer.android.com/training/articles/keystore
