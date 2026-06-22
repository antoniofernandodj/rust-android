# Cryptographic Implementation

> **Seção:** Security and Encryption
> **Status:** ✅ preenchido

## Visão geral

Rust é uma excelente linguagem para **criptografia** em mobile: sem buffer overflows, com
crates auditados e desempenho nativo. O objetivo é proteger dados em repouso e em trânsito
**sem reinventar primitivas** — usar bibliotecas estabelecidas (`ring`, `RustCrypto`,
`rustls`) e delegar o armazenamento de chaves ao hardware seguro do dispositivo (Android
**Keystore**, respaldado por TEE/StrongBox). Regra de ouro: *don't roll your own crypto*.

Antes do "como", o "o quê" — defina seu **modelo de ameaça**. Criptografia não é um
ingrediente mágico; ela protege contra ameaças *específicas*:

- **Dispositivo perdido/roubado** → criptografia em repouso com chave no Keystore (atacante
  com o aparelho não lê os dados sem desbloquear).
- **Rede hostil (Wi-Fi público, MITM)** → TLS (rustls) com validação de certificado, talvez
  *pinning*.
- **App malicioso no mesmo device** → sandbox do Android + permissões; Keystore impede
  extração de chaves.
- **Atacante com acesso root / análise de binário** → aqui as garantias enfraquecem: segredos
  embutidos no `.so` **não** são seguros (ver *Package Management → Binary obfuscation*); a
  defesa real é não ter segredo no cliente e usar Keystore com `StrongBox`/biometria.

Cada técnica abaixo mira uma dessas ameaças. Saber qual você está mitigando evita
"criptografia teatral" que não protege nada.

## Subtópicos

### Symmetric encryption primitives

- Criptografia simétrica protege dados em repouso (cache, banco local). Use **AEAD**
  (*Authenticated Encryption with Associated Data* — cifra **e** autentica de uma vez) como
  **AES-256-GCM** ou **ChaCha20-Poly1305**.
- Por que AEAD e não "só cifrar": cifrar sem autenticar (ex.: AES-CBC puro) deixa o
  ciphertext **maleável** — um atacante altera bytes e você não percebe. AEAD detecta
  qualquer adulteração na decifragem.
- Crates: `aes-gcm`, `chacha20poly1305` (RustCrypto) ou `ring`. **AES-GCM** é mais rápido em
  CPUs com aceleração de hardware (AES-NI/ARMv8 Crypto Extensions); **ChaCha20-Poly1305** é
  mais rápido e mais seguro contra *timing* em hardware sem essa aceleração — comum em
  Androids baratos.

```rust
use chacha20poly1305::{aead::{Aead, KeyInit, OsRng, AeadCore}, ChaCha20Poly1305};

let cipher = ChaCha20Poly1305::new(chave.into());      // chave de 32 bytes
let nonce  = ChaCha20Poly1305::generate_nonce(&mut OsRng); // 12 bytes, ÚNICO por mensagem
let ct = cipher.encrypt(&nonce, texto.as_ref())?;
// armazene nonce || ciphertext juntos
```

- **Regra crítica do nonce:** nunca reutilize o mesmo nonce com a mesma chave (em GCM, isso é
  catastrófico — vaza a chave de autenticação). Gere aleatório (`OsRng`) ou use um contador
  monotônico persistido. Armazene o nonce **junto** ao ciphertext (ele não é secreto).
- **AAD (associated data):** dados autenticados mas não cifrados (ex.: um cabeçalho de versão,
  um ID) — amarram o ciphertext a um contexto, impedindo ataques de "troca de blocos".

### Asymmetric key generation

- Criptografia assimétrica (par chave pública/privada) para troca de chaves e assinaturas:
  **Ed25519** (assinatura), **X25519** (*key agreement*/ECDH), **RSA** (legado/interop).
- Crates: `ed25519-dalek`, `x25519-dalek`, `rsa`. Prefira curvas modernas (Curve25519) a
  RSA quando possível, por desempenho, tamanho de chave menor e simplicidade (sem escolher
  padding como no RSA, onde erros são fáceis).
- **Geração de chaves precisa de aleatoriedade segura:** sempre `OsRng` (que no Android puxa
  do `/dev/urandom`/getrandom do kernel), **nunca** um RNG previsível (`rand::thread_rng` é
  ok, `StdRng::seed_from_u64` para segredo real **não**).
- A chave privada **nunca** deve sair do dispositivo; idealmente é gerada e mantida no
  Keystore (ver abaixo), onde nem o seu próprio app consegue lê-la.

### Secure storage integration

- Dados sensíveis (tokens, chaves derivadas) devem ir para storage protegido:
  **EncryptedSharedPreferences**/**DataStore** com chave do Keystore, ou um arquivo cifrado
  pelo núcleo Rust.
- O padrão **envelope encryption**: o Rust gera uma DEK (*data encryption key*) aleatória,
  cifra os dados com ela (AES-GCM), e então **envelopa (wrap)** a DEK com uma KEK (*key
  encryption key*) que vive no Keystore. Você guarda `{dados cifrados, DEK envelopada}`; só o
  Keystore consegue desenvelopar a DEK.
- A chave de criptografia **não** fica no código nem em texto plano. Derivar chave de senha
  do usuário? Use uma **KDF lenta e com sal**: **Argon2id** (preferida), `scrypt` ou
  PBKDF2 — `argon2`/`scrypt` crates. Nunca use SHA-256 simples como KDF de senha.

### Hardware backed keystore access

- O **Android Keystore** armazena chaves em hardware seguro (TEE — *Trusted Execution
  Environment* — ou **StrongBox**, um secure element dedicado), onde elas **nunca são
  expostas** à memória do app — você só pede operações (assinar, decifrar), os bits da chave
  jamais chegam ao seu processo.
- O Rust acessa o Keystore **via JNI**, chamando a API Java (`KeyStore`,
  `KeyGenParameterSpec`, `Cipher`) — é o caso clássico onde UniFFI não basta e você precisa
  de JNI manual ou de uma callback interface implementada em Kotlin (ver *Java Native
  Interface* e *Cross Platform Logic → Platform specific backend injection*).
- Recursos poderosos do Keystore para o seu modelo de ameaça:
  - `setUserAuthenticationRequired(true)` — exige desbloqueio/biometria para usar a chave
    (liga ao `BiometricPrompt`).
  - `setUnlockedDeviceRequired(true)` — chave só usável com o device desbloqueado.
  - `setIsStrongBoxBacked(true)` — força o secure element (onde houver).
  - **Key attestation** — o Keystore emite um certificado provando, para o seu servidor, que
    a chave vive em hardware genuíno (anti-fraude).
- **Arquitetura recomendada:** chaves protegidas por hardware (Keystore) + a criptografia de
  *bulk* (cifrar arquivos/DB) feita em Rust com a DEK desenvelopada na hora. O Keystore
  guarda a chave-mestra; o Rust faz o trabalho pesado, rápido e portável.

### Message authentication codes

- **MAC** garante integridade e autenticidade de dados: **HMAC-SHA256** (`hmac` + `sha2`) ou
  Poly1305 (já embutido nos AEAD acima — se você usa AES-GCM/ChaCha20-Poly1305, já tem
  autenticação e *não* precisa de HMAC separado).
- Use HMAC para casos fora de AEAD: derivar sub-chaves (via HKDF), autenticar uma mensagem
  não cifrada, *cookies*/tokens assinados.
- **Compare MACs/tags em tempo constante** (`subtle::ConstantTimeEq`) para evitar *timing
  attacks*: comparar byte a byte com `==` retorna mais cedo no primeiro byte diferente,
  vazando informação que permite forjar a tag por tentativa. Nunca use `==` comum para
  comparar segredos/tags/hashes de senha.

### Signature verification protocols

- Assinaturas digitais verificam **origem e integridade** (ex.: validar payloads de servidor,
  atualizações OTA, licenças, mensagens entre usuários). Use **Ed25519** para verificar
  rápido com a chave pública embutida no app.
- Valide sempre o resultado de `verify(...)` e trate erro como **rejeição total** — não
  prossiga com dado não verificado, não logue o dado, não "tente mesmo assim". *Fail closed*.

```rust
use ed25519_dalek::{Signature, VerifyingKey, Verifier};

fn verificar(pubkey: &VerifyingKey, msg: &[u8], assinatura: &Signature) -> bool {
    pubkey.verify(msg, assinatura).is_ok()   // só prossiga se true
}
```

- Para **TLS/HTTPS**, prefira **`rustls`** (TLS em Rust puro, memory-safe, sem a superfície de
  CVE histórica do OpenSSL) a wrappers de OpenSSL quando viável no Android. Para *certificate
  pinning*, configure os certificados/raízes aceitos no `rustls` ou use o
  `Network Security Config` do Android.

## Armadilhas comuns

- **Nonce/IV reutilizado** — quebra AES-GCM completamente. Gere único por mensagem.
- **RNG inseguro para chaves** — usar um RNG semeado/previsível torna a chave adivinhável.
  Sempre `OsRng`.
- **Segredo embutido no binário** — chaves de API "escondidas" no `.so` são extraíveis;
  ofuscação não é segurança (ver *Package Management → Binary obfuscation*). Mova o segredo
  para o servidor ou para o Keystore.
- **Comparação não constante** de tags/hashes — timing attack. Use `subtle`.
- **SHA-256 como "hash de senha"** — rápido demais; um GPU testa bilhões/s. Use Argon2id.
- **Confiar no app cliente** — qualquer validação client-side pode ser contornada por um
  atacante com o binário. Validação que importa é server-side.

## Assuntos correlatos

- **Onde guardar a chave-mestra:** Keystore via injeção de `SecureStorage` (*Cross Platform
  Logic*).
- **Acesso ao Keystore via JNI:** *Java Native Interface*.
- **Verificação de assinatura do próprio pacote (app signing):** *Package Management →
  Signature verification process* — não confundir com assinar *dados* (este capítulo).
- **Custo de performance da cripto:** *Performance Optimization* (AES-NI/NEON aceleram).

## Referências

- RustCrypto: https://github.com/RustCrypto
- ring: https://github.com/briansmith/ring
- rustls: https://github.com/rustls/rustls
- Android Keystore: https://developer.android.com/training/articles/keystore
- OWASP MASVS (Mobile App Security): https://mas.owasp.org/MASVS/
