# Web Crypto API Implementation Plan for jstz

## Document Overview

**Purpose**: Detailed, actionable implementation plan to bridge the Web Crypto API gap in jstz
**Audience**: jstz core contributors, implementation team
**Status**: Proposal - Requires team review and approval
**Last Updated**: 2025-11-18

---

## Executive Summary

### Current Gap
- **Web Crypto API (JS)**: ~1% (0% functional)
- **Protocol Crypto (Rust)**: Excellent (Ed25519, P256, Secp256k1, Blake2b)
- **Opportunity**: Expose existing Rust crypto to JavaScript

### Implementation Strategy
- **3 Phases** over **12-18 weeks**
- **Phase 1 (MVP)**: Deterministic operations only (6-8 weeks)
- **Phase 2**: Extended crypto features (4-6 weeks)
- **Phase 3**: Advanced features + randomness decision (2-4 weeks)

### Success Metrics
- **Phase 1 Complete**: 15-20% WPT pass rate
- **Phase 2 Complete**: 30-40% WPT pass rate
- **Phase 3 Complete**: 40-50% WPT pass rate

---

## Phase 1: MVP - Core Deterministic Operations

**Timeline**: 6-8 weeks
**Goal**: Enable hash functions and signature verification in JavaScript
**Risk Level**: LOW (leveraging existing Rust code)

### Phase 1.1: Foundation & Architecture (Week 1-2)

#### Deliverables

1. **Create jstz_crypto Runtime Extension**
   - **New Files**:
     - `crates/jstz_runtime/src/ext/jstz_crypto/mod.rs`
     - `crates/jstz_runtime/src/ext/jstz_crypto/crypto.js`
     - `crates/jstz_runtime/src/ext/jstz_crypto/subtle.js`
     - `crates/jstz_runtime/src/ext/jstz_crypto/key.rs` (CryptoKey management)

   - **Modified Files**:
     - `crates/jstz_runtime/src/ext/mod.rs` (register extension)
     - `crates/jstz_runtime/src/runtime.rs:490-516` (add to init_base_extensions)
     - `crates/jstz_runtime/Cargo.toml` (dependencies)

2. **Expose crypto Global Object**
   - **Modified Files**:
     - `crates/jstz_runtime/src/ext/jstz_main/98_global_scope.js` (add crypto property)

   - **Implementation**:
     ```javascript
     // In 98_global_scope.js
     import * as crypto from "ext:jstz_crypto/crypto.js";

     const workerGlobalScope = {
       // ... existing APIs ...
       crypto: core.propNonEnumerable(crypto.crypto),
       // ...
     };
     ```

3. **Design CryptoKey Storage System**
   - Store CryptoKey objects in `OpState` with integer handles
   - Implement handle → CryptoKey mapping
   - Define CryptoKey types for Ed25519, P256, HMAC, AES

#### Acceptance Criteria
- [ ] jstz_crypto extension loads without errors
- [ ] `crypto` object accessible in JavaScript
- [ ] `crypto.subtle` returns object (even if empty initially)
- [ ] No regression in existing tests
- [ ] Build succeeds on all platforms

#### Code Template: Extension Structure

```rust
// File: crates/jstz_runtime/src/ext/jstz_crypto/mod.rs

use deno_core::{extension, op2};
use jstz_crypto as crypto;

extension!(
    jstz_crypto,
    ops = [
        op_crypto_digest,
        op_crypto_verify,
        op_crypto_import_key,
    ],
    esm_entry_point = "ext:jstz_crypto/crypto.js",
    esm = [
        dir "src/ext/jstz_crypto",
        "crypto.js",
        "subtle.js",
    ]
);

// CryptoKey handle management
#[derive(Default)]
pub struct CryptoKeyStore {
    next_handle: u32,
    keys: std::collections::HashMap<u32, StoredCryptoKey>,
}

pub enum StoredCryptoKey {
    Ed25519Public(crypto::PublicKey),
    P256Public(crypto::PublicKey),
    // ... more key types
}
```

---

### Phase 1.2: Hash Functions (Week 2-3)

#### Deliverables

1. **Implement crypto.subtle.digest()**
   - **Algorithms**: SHA-256, SHA-384, SHA-512, Blake2b-256
   - **Rust Op**: `op_crypto_digest`
   - **Leverage**: Existing `cryptoxide` dependency

#### Implementation Details

**Rust Op**:
```rust
// File: crates/jstz_runtime/src/ext/jstz_crypto/mod.rs

#[op2(async)]
#[buffer]
async fn op_crypto_digest(
    #[string] algorithm: String,
    #[buffer] data: &[u8],
) -> Result<Vec<u8>, AnyError> {
    use cryptoxide::digest::Digest;

    match algorithm.as_str() {
        "SHA-256" => {
            let mut hasher = cryptoxide::sha2::Sha256::new();
            hasher.input(data);
            let mut output = vec![0u8; 32];
            hasher.result(&mut output);
            Ok(output)
        }
        "SHA-384" => {
            let mut hasher = cryptoxide::sha2::Sha384::new();
            hasher.input(data);
            let mut output = vec![0u8; 48];
            hasher.result(&mut output);
            Ok(output)
        }
        "SHA-512" => {
            let mut hasher = cryptoxide::sha2::Sha512::new();
            hasher.input(data);
            let mut output = vec![0u8; 64];
            hasher.result(&mut output);
            Ok(output)
        }
        "BLAKE2B-256" => {
            use jstz_crypto::hash::Blake2b;
            Ok(Blake2b::from(data).0.to_vec())
        }
        _ => Err(anyhow::anyhow!("Unsupported algorithm: {}", algorithm)),
    }
}
```

**JavaScript Wrapper**:
```javascript
// File: crates/jstz_runtime/src/ext/jstz_crypto/subtle.js

const core = globalThis.Deno.core;
const ops = core.ops;

class SubtleCrypto {
  async digest(algorithm, data) {
    // Normalize algorithm
    const algName = typeof algorithm === "string"
      ? algorithm.toUpperCase()
      : algorithm.name.toUpperCase();

    // Convert data to Uint8Array
    const dataBytes = new Uint8Array(data);

    // Call Rust op
    const hashBytes = await ops.op_crypto_digest(algName, dataBytes);

    return hashBytes.buffer;
  }
}
```

#### Testing Strategy

1. **Unit Tests** (Rust):
   ```rust
   #[test]
   fn test_sha256_digest() {
       let data = b"hello world";
       let result = op_crypto_digest("SHA-256".to_string(), data).unwrap();
       assert_eq!(result.len(), 32);
       // Compare with known hash value
   }
   ```

2. **Integration Tests** (JavaScript):
   ```javascript
   // File: crates/jstz_runtime/tests/crypto_digest.js
   const data = new TextEncoder().encode("hello world");
   const hash = await crypto.subtle.digest("SHA-256", data);
   console.assert(hash.byteLength === 32);
   ```

3. **WPT Tests**:
   - Run WebCryptoAPI digest tests
   - Expect ~10-15 digest tests to pass

#### Acceptance Criteria
- [ ] SHA-256, SHA-384, SHA-512 digest working
- [ ] Blake2b-256 digest working (jstz-specific)
- [ ] WPT digest tests: 10+ passing
- [ ] Example smart function can hash data
- [ ] Documentation written for digest()

---

### Phase 1.3: Signature Verification (Week 3-5)

#### Deliverables

1. **Implement crypto.subtle.importKey()** (Public Keys Only)
   - **Formats**: "raw", "jwk"
   - **Algorithms**: Ed25519, ECDSA-P256
   - **Rust Op**: `op_crypto_import_key`

2. **Implement crypto.subtle.verify()**
   - **Algorithms**: Ed25519, ECDSA-P256
   - **Rust Op**: `op_crypto_verify`
   - **Leverage**: Existing `jstz_crypto` verification

#### Implementation Details

**Step 1: Import Public Key (Raw Format)**

```rust
// File: crates/jstz_runtime/src/ext/jstz_crypto/mod.rs

#[op2(async)]
#[serde]
async fn op_crypto_import_key(
    state: &mut OpState,
    #[string] format: String,
    #[buffer] key_data: &[u8],
    #[serde] algorithm: serde_json::Value,
    extractable: bool,
    #[serde] usages: Vec<String>,
) -> Result<u32, AnyError> {
    let alg_name = algorithm["name"].as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing algorithm name"))?;

    let key = match (format.as_str(), alg_name) {
        ("raw", "Ed25519") => {
            let pk = jstz_crypto::PublicKey::from_bytes(key_data)
                .map_err(|e| anyhow::anyhow!("Invalid Ed25519 key: {}", e))?;
            StoredCryptoKey::Ed25519Public(pk)
        }
        ("raw", "ECDSA") => {
            // Parse P-256 public key
            let curve = algorithm["namedCurve"].as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing namedCurve"))?;

            if curve != "P-256" {
                return Err(anyhow::anyhow!("Unsupported curve: {}", curve));
            }

            let pk = jstz_crypto::PublicKey::from_bytes(key_data)
                .map_err(|e| anyhow::anyhow!("Invalid P-256 key: {}", e))?;
            StoredCryptoKey::P256Public(pk)
        }
        _ => return Err(anyhow::anyhow!("Unsupported format/algorithm: {}/{}", format, alg_name)),
    };

    // Store key and return handle
    let mut key_store = state.borrow_mut::<CryptoKeyStore>();
    let handle = key_store.next_handle;
    key_store.next_handle += 1;
    key_store.keys.insert(handle, key);

    Ok(handle)
}
```

**Step 2: Verify Signature**

```rust
#[op2(async)]
async fn op_crypto_verify(
    state: &mut OpState,
    #[serde] algorithm: serde_json::Value,
    key_handle: u32,
    #[buffer] signature: &[u8],
    #[buffer] data: &[u8],
) -> Result<bool, AnyError> {
    let key_store = state.borrow::<CryptoKeyStore>();
    let key = key_store.keys.get(&key_handle)
        .ok_or_else(|| anyhow::anyhow!("Invalid key handle"))?;

    match key {
        StoredCryptoKey::Ed25519Public(pk) => {
            let sig = jstz_crypto::signature::Ed25519Signature::try_from(signature)
                .map_err(|e| anyhow::anyhow!("Invalid signature: {}", e))?;
            Ok(pk.verify(&sig, data).is_ok())
        }
        StoredCryptoKey::P256Public(pk) => {
            let sig = jstz_crypto::signature::P256Signature::try_from(signature)
                .map_err(|e| anyhow::anyhow!("Invalid signature: {}", e))?;
            Ok(pk.verify(&sig, data).is_ok())
        }
        _ => Err(anyhow::anyhow!("Key type not suitable for verification")),
    }
}
```

**JavaScript Wrapper**:

```javascript
// File: crates/jstz_runtime/src/ext/jstz_crypto/subtle.js

class SubtleCrypto {
  async importKey(format, keyData, algorithm, extractable, keyUsages) {
    const handle = await ops.op_crypto_import_key(
      format,
      new Uint8Array(keyData),
      algorithm,
      extractable,
      keyUsages
    );

    return new CryptoKey(handle, algorithm, extractable, keyUsages, "public");
  }

  async verify(algorithm, key, signature, data) {
    const result = await ops.op_crypto_verify(
      algorithm,
      key.handle,
      new Uint8Array(signature),
      new Uint8Array(data)
    );

    return result;
  }
}

class CryptoKey {
  constructor(handle, algorithm, extractable, usages, type) {
    this.#handle = handle;
    this.algorithm = algorithm;
    this.extractable = extractable;
    this.usages = usages;
    this.type = type;
  }

  get handle() {
    return this.#handle;
  }

  #handle;
}
```

#### Testing Strategy

1. **Test Vector Tests**:
   ```javascript
   // Known Ed25519 test vectors
   const publicKey = await crypto.subtle.importKey(
     "raw",
     new Uint8Array([/* known public key */]),
     { name: "Ed25519" },
     false,
     ["verify"]
   );

   const valid = await crypto.subtle.verify(
     { name: "Ed25519" },
     publicKey,
     knownSignature,
     knownMessage
   );

   console.assert(valid === true);
   ```

2. **Interop Tests**:
   - Sign with jstz CLI (Rust)
   - Verify in smart function (JavaScript)

3. **WPT Tests**:
   - Enable Ed25519 and ECDSA P-256 test suites
   - Expect 15-20 more tests to pass

#### Acceptance Criteria
- [ ] Can import Ed25519 public key (raw format)
- [ ] Can import P-256 public key (raw format)
- [ ] Can verify Ed25519 signatures
- [ ] Can verify P-256 signatures
- [ ] WPT tests: 25-30 total passing
- [ ] Example: JWT signature verification works

---

### Phase 1.4: JWK Support (Week 5-6)

#### Deliverables

1. **Extend importKey() to Support JWK Format**
   - Parse JWK JSON structure
   - Support Ed25519 ("OKP" kty, "Ed25519" crv)
   - Support P-256 ("EC" kty, "P-256" crv)

2. **Implement exportKey() for JWK**
   - Rust Op: `op_crypto_export_key`
   - Only for extractable keys

#### Implementation Details

```rust
// JWK parsing
fn parse_jwk_ed25519(jwk: &serde_json::Value) -> Result<jstz_crypto::PublicKey, AnyError> {
    let kty = jwk["kty"].as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing kty"))?;

    if kty != "OKP" {
        return Err(anyhow::anyhow!("Expected OKP key type"));
    }

    let crv = jwk["crv"].as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing crv"))?;

    if crv != "Ed25519" {
        return Err(anyhow::anyhow!("Expected Ed25519 curve"));
    }

    let x = jwk["x"].as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing x coordinate"))?;

    // Decode base64url
    let bytes = base64::decode_config(x, base64::URL_SAFE_NO_PAD)?;

    jstz_crypto::PublicKey::from_bytes(&bytes)
        .map_err(|e| anyhow::anyhow!("Invalid key: {}", e))
}
```

#### Acceptance Criteria
- [ ] Can import Ed25519 key from JWK
- [ ] Can import P-256 key from JWK
- [ ] Can export public keys to JWK
- [ ] WPT tests: 30-35 total passing
- [ ] Interop with standard JWT libraries

---

### Phase 1.5: Documentation & Examples (Week 6-8)

#### Deliverables

1. **Developer Documentation**
   - File: `docs/api/crypto.md`
   - Sections:
     - Available algorithms
     - Supported operations
     - Limitations (no random, no key generation)
     - Code examples

2. **Example Smart Functions**
   - File: `examples/crypto_hash.js` - Hash data
   - File: `examples/crypto_verify_jwt.js` - Verify JWT
   - File: `examples/crypto_verify_signature.js` - Verify Ed25519 signature

3. **Migration Guide**
   - How to use crypto in smart functions
   - Differences from browser Web Crypto API
   - Best practices for deterministic crypto

#### Example: Hash Data

```javascript
// File: examples/crypto_hash.js

export default async (request) => {
  const { data } = await request.json();

  // Convert string to bytes
  const encoder = new TextEncoder();
  const dataBytes = encoder.encode(data);

  // Hash with SHA-256
  const hashBuffer = await crypto.subtle.digest("SHA-256", dataBytes);

  // Convert to hex string
  const hashArray = Array.from(new Uint8Array(hashBuffer));
  const hashHex = hashArray.map(b => b.toString(16).padStart(2, '0')).join('');

  return new Response(JSON.stringify({ hash: hashHex }));
};
```

#### Example: Verify JWT

```javascript
// File: examples/crypto_verify_jwt.js

// Simplified JWT verification (RS256 with P-256)
export default async (request) => {
  const { token, publicKeyJwk } = await request.json();

  // Parse JWT
  const [headerB64, payloadB64, signatureB64] = token.split('.');

  // Import public key
  const publicKey = await crypto.subtle.importKey(
    "jwk",
    publicKeyJwk,
    { name: "ECDSA", namedCurve: "P-256" },
    false,
    ["verify"]
  );

  // Verify signature
  const encoder = new TextEncoder();
  const data = encoder.encode(`${headerB64}.${payloadB64}`);
  const signature = base64UrlDecode(signatureB64);

  const valid = await crypto.subtle.verify(
    { name: "ECDSA", hash: "SHA-256" },
    publicKey,
    signature,
    data
  );

  return new Response(JSON.stringify({ valid }));
};

function base64UrlDecode(str) {
  // Base64url decode implementation
  const base64 = str.replace(/-/g, '+').replace(/_/g, '/');
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}
```

#### Acceptance Criteria
- [ ] API documentation complete
- [ ] 3+ working examples
- [ ] Tutorial: "Using Crypto in Smart Functions"
- [ ] Security best practices documented
- [ ] Determinism limitations clearly explained

---

### Phase 1: Summary & Success Criteria

**Duration**: 6-8 weeks

**Key Deliverables**:
- ✅ `crypto.subtle.digest()` - SHA-256, SHA-384, SHA-512, Blake2b
- ✅ `crypto.subtle.verify()` - Ed25519, P-256
- ✅ `crypto.subtle.importKey()` - Raw, JWK formats
- ✅ `crypto.subtle.exportKey()` - JWK format
- ✅ CryptoKey interface
- ✅ Complete documentation

**Success Metrics**:
- [ ] WPT pass rate: 15-20% (35-50 tests)
- [ ] Zero determinism regressions
- [ ] 3+ example smart functions working
- [ ] Documentation reviewed and approved
- [ ] Performance: digest < 10ms, verify < 50ms (for typical inputs)

**Files Created** (~15 files):
- `crates/jstz_runtime/src/ext/jstz_crypto/mod.rs`
- `crates/jstz_runtime/src/ext/jstz_crypto/crypto.js`
- `crates/jstz_runtime/src/ext/jstz_crypto/subtle.js`
- `crates/jstz_runtime/src/ext/jstz_crypto/key.rs`
- `crates/jstz_runtime/tests/crypto_digest_test.rs`
- `crates/jstz_runtime/tests/crypto_verify_test.rs`
- `docs/api/crypto.md`
- `examples/crypto_hash.js`
- `examples/crypto_verify_jwt.js`
- `examples/crypto_verify_signature.js`

**Files Modified** (~5 files):
- `crates/jstz_runtime/src/ext/mod.rs`
- `crates/jstz_runtime/src/runtime.rs`
- `crates/jstz_runtime/Cargo.toml`
- `crates/jstz_runtime/src/ext/jstz_main/98_global_scope.js`
- `docs/api/index.md`

---

## Phase 2: Extended Deterministic Operations

**Timeline**: 4-6 weeks
**Goal**: Add HMAC, encryption, and advanced key management
**Risk Level**: MEDIUM (new crypto primitives required)

### Phase 2.1: HMAC Implementation (Week 9-10)

#### Deliverables

1. **Add hmac Crate Dependency**
   - Modify: `crates/jstz_crypto/Cargo.toml`
   - Add: `hmac = "0.12"`
   - Add: `sha2 = "0.10"` (if not already present)

2. **Implement crypto.subtle.sign() for HMAC**
   - Algorithms: HMAC-SHA256, HMAC-SHA384, HMAC-SHA512
   - Rust Op: `op_crypto_sign_hmac`

3. **Implement crypto.subtle.verify() for HMAC**
   - Verify HMAC tags
   - Constant-time comparison

#### Implementation Details

```rust
// File: crates/jstz_crypto/src/hmac.rs (new)

use hmac::{Hmac, Mac};
use sha2::{Sha256, Sha384, Sha512};

pub fn sign_hmac_sha256(key: &[u8], data: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key)
        .map_err(|_| CryptoError::InvalidKeyLength)?;
    mac.update(data);
    Ok(mac.finalize().into_bytes().to_vec())
}

pub fn verify_hmac_sha256(key: &[u8], data: &[u8], tag: &[u8]) -> Result<bool, CryptoError> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key)
        .map_err(|_| CryptoError::InvalidKeyLength)?;
    mac.update(data);
    Ok(mac.verify_slice(tag).is_ok())
}
```

```rust
// File: crates/jstz_runtime/src/ext/jstz_crypto/mod.rs

#[op2(async)]
#[buffer]
async fn op_crypto_sign(
    state: &mut OpState,
    #[serde] algorithm: serde_json::Value,
    key_handle: u32,
    #[buffer] data: &[u8],
) -> Result<Vec<u8>, AnyError> {
    let key_store = state.borrow::<CryptoKeyStore>();
    let key = key_store.keys.get(&key_handle)
        .ok_or_else(|| anyhow::anyhow!("Invalid key handle"))?;

    let alg_name = algorithm["name"].as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing algorithm name"))?;

    match (alg_name, key) {
        ("HMAC", StoredCryptoKey::HmacSecret(key_bytes, hash)) => {
            match hash.as_str() {
                "SHA-256" => jstz_crypto::hmac::sign_hmac_sha256(key_bytes, data)
                    .map_err(|e| anyhow::anyhow!("HMAC failed: {}", e)),
                "SHA-384" => jstz_crypto::hmac::sign_hmac_sha384(key_bytes, data)
                    .map_err(|e| anyhow::anyhow!("HMAC failed: {}", e)),
                "SHA-512" => jstz_crypto::hmac::sign_hmac_sha512(key_bytes, data)
                    .map_err(|e| anyhow::anyhow!("HMAC failed: {}", e)),
                _ => Err(anyhow::anyhow!("Unsupported hash: {}", hash)),
            }
        }
        _ => Err(anyhow::anyhow!("Invalid algorithm/key combination")),
    }
}
```

#### Acceptance Criteria
- [ ] HMAC-SHA256 signing works
- [ ] HMAC-SHA384, HMAC-SHA512 work
- [ ] HMAC verification works (constant-time)
- [ ] Can import HMAC keys (raw format)
- [ ] WPT tests: 40-50 total passing
- [ ] Example: API authentication with HMAC

---

### Phase 2.2: AES-GCM Encryption (Week 10-12)

#### Deliverables

1. **Add aes-gcm Crate Dependency**
   - Modify: `crates/jstz_crypto/Cargo.toml`
   - Add: `aes-gcm = "0.10"`

2. **Implement crypto.subtle.encrypt()**
   - Algorithm: AES-GCM (128, 192, 256-bit keys)
   - Rust Op: `op_crypto_encrypt`

3. **Implement crypto.subtle.decrypt()**
   - Rust Op: `op_crypto_decrypt`

#### Implementation Details

```rust
// File: crates/jstz_crypto/src/aes.rs (new)

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};

pub fn encrypt_aes_gcm_256(
    key: &[u8],
    nonce: &[u8],
    plaintext: &[u8],
    additional_data: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| CryptoError::InvalidKeyLength)?;

    let nonce = Nonce::from_slice(nonce);

    cipher.encrypt(nonce, plaintext)
        .map_err(|_| CryptoError::EncryptionFailed)
}

pub fn decrypt_aes_gcm_256(
    key: &[u8],
    nonce: &[u8],
    ciphertext: &[u8],
    additional_data: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| CryptoError::InvalidKeyLength)?;

    let nonce = Nonce::from_slice(nonce);

    cipher.decrypt(nonce, ciphertext)
        .map_err(|_| CryptoError::DecryptionFailed)
}
```

**Important**: User MUST provide IV/nonce (no random generation)

```javascript
// Example: Deterministic encryption
const key = await crypto.subtle.importKey(
  "raw",
  keyBytes,
  { name: "AES-GCM" },
  false,
  ["encrypt", "decrypt"]
);

// User provides IV (must be unique per message!)
const iv = new Uint8Array(12); // WARNING: In real code, this MUST be unique!
// In deterministic context: derive IV from message hash or other deterministic source

const ciphertext = await crypto.subtle.encrypt(
  { name: "AES-GCM", iv: iv },
  key,
  plaintext
);
```

#### Security Warning Documentation

**CRITICAL**: Document clearly that:
- IV/nonce MUST be unique for each encryption
- In deterministic context, cannot use random IV
- Recommend deriving IV from: `HMAC(master_key, message_hash || counter)`
- Reusing IV with same key = catastrophic security failure

#### Acceptance Criteria
- [ ] AES-GCM-256 encryption works
- [ ] AES-GCM-256 decryption works
- [ ] AES-GCM-128, AES-GCM-192 work
- [ ] Authenticated encryption verified (tag checked)
- [ ] WPT tests: 55-65 total passing
- [ ] Security documentation for IV management
- [ ] Example: Encrypt/decrypt data in smart function

---

### Phase 2.3: Advanced Key Management (Week 12-14)

#### Deliverables

1. **Implement crypto.subtle.generateKey()** (Deterministic Only)
   - **Input**: User-provided entropy/seed
   - **Algorithms**: AES-GCM, HMAC
   - **NOT using random!**

2. **Implement crypto.subtle.deriveKey()** (PBKDF2)
   - Derive keys from passwords
   - Deterministic (same password + salt = same key)

3. **Implement crypto.subtle.deriveBits()** (PBKDF2)

#### Implementation Details

**Option A: Disable generateKey() (Recommended for Phase 2)**
- Keep strict determinism
- Document: "Key generation not supported - import keys instead"

**Option B: Seed-based generateKey() (If randomness decision made)**
- Require explicit seed parameter (non-standard)
- `generateKey(algorithm, seed, extractable, usages)`

**PBKDF2 Implementation**:

```rust
// File: crates/jstz_crypto/src/pbkdf2.rs

use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;

pub fn derive_key_pbkdf2_sha256(
    password: &[u8],
    salt: &[u8],
    iterations: u32,
    key_length: usize,
) -> Vec<u8> {
    let mut key = vec![0u8; key_length];
    pbkdf2_hmac::<Sha256>(password, salt, iterations as usize, &mut key);
    key
}
```

#### Acceptance Criteria
- [ ] PBKDF2 key derivation works
- [ ] Can derive AES keys from passwords
- [ ] Can derive HMAC keys from passwords
- [ ] Iteration count configurable
- [ ] WPT tests: 65-75 total passing
- [ ] Example: Password-based encryption

---

### Phase 2: Summary & Success Criteria

**Duration**: 4-6 weeks

**Key Deliverables**:
- ✅ HMAC-SHA256/384/512 (sign & verify)
- ✅ AES-GCM encryption/decryption
- ✅ PBKDF2 key derivation
- ✅ Security documentation for deterministic crypto

**Success Metrics**:
- [ ] WPT pass rate: 30-40% (75-100 tests)
- [ ] Real-world use case: JWT verification ✓
- [ ] Real-world use case: Data encryption ✓
- [ ] Real-world use case: API authentication (HMAC) ✓
- [ ] Performance benchmarks documented

**New Dependencies**:
- `hmac = "0.12"`
- `aes-gcm = "0.10"`
- `pbkdf2 = "0.12"`

---

## Phase 3: Advanced Features & Randomness Strategy

**Timeline**: 2-4 weeks
**Goal**: Finalize crypto API, decide on randomness, optimize performance
**Risk Level**: MEDIUM-HIGH (architectural decisions required)

### Phase 3.1: Randomness Decision & Implementation (Week 15-16)

#### Critical Decision Point

**Must Choose One**:

**Option A: Strict Determinism (Recommended)**
- `crypto.getRandomValues()` throws error or returns error
- `crypto.randomUUID()` throws error
- `crypto.subtle.generateKey()` requires user-provided seed
- Document clearly: "No random number generation in smart functions"

**Option B: Seeded PRNG**
- Implement ChaCha20 PRNG seeded from transaction context
- `crypto.getRandomValues()` returns deterministic pseudo-random bytes
- Document clearly: "NOT cryptographically random! Deterministic!"
- Seed: `Blake2b(tx_hash || block_hash || nonce)`

**Option C: Oracle-Based Randomness**
- Route `getRandomValues()` through oracle
- Fetch entropy from external source (e.g., drand)
- **Breaks determinism** - not recommended

#### Recommendation: Option A

**Rationale**:
- Maintains rollup integrity
- Clear security model
- Aligns with other smart contract platforms
- Users can generate keys externally and import

**Implementation (Option A)**:

```javascript
// File: crates/jstz_runtime/src/ext/jstz_crypto/crypto.js

class Crypto {
  getRandomValues(array) {
    throw new DOMException(
      "getRandomValues() is not supported in jstz smart functions. " +
      "Smart functions require deterministic execution. " +
      "Generate cryptographic keys externally and import them using crypto.subtle.importKey().",
      "NotSupportedError"
    );
  }

  randomUUID() {
    throw new DOMException(
      "randomUUID() is not supported in jstz smart functions. " +
      "Smart functions require deterministic execution.",
      "NotSupportedError"
    );
  }

  get subtle() {
    return subtle;
  }
}
```

#### If Option B Chosen (Seeded PRNG):

```rust
// File: crates/jstz_runtime/src/ext/jstz_crypto/prng.rs

use chacha20::ChaCha20Rng;
use rand_core::{RngCore, SeedableRng};

pub fn get_deterministic_random(
    tx_hash: &[u8],
    block_hash: &[u8],
    nonce: u64,
    output: &mut [u8],
) {
    // Create seed
    let mut seed_input = Vec::new();
    seed_input.extend_from_slice(tx_hash);
    seed_input.extend_from_slice(block_hash);
    seed_input.extend_from_slice(&nonce.to_le_bytes());

    let seed_bytes = jstz_crypto::hash::Blake2b::from(&seed_input).0;

    // Create PRNG
    let mut rng = ChaCha20Rng::from_seed(seed_bytes);
    rng.fill_bytes(output);
}
```

**Warning**: Must document prominently:
> ⚠️ **WARNING**: `crypto.getRandomValues()` in jstz returns DETERMINISTIC pseudo-random values, NOT cryptographically secure random values. Same transaction context produces same "random" values. Do NOT use for key generation in production without understanding the security implications.

#### Acceptance Criteria
- [ ] Decision made and documented
- [ ] Implementation complete
- [ ] WPT tests updated (skip random-dependent tests if Option A)
- [ ] Security implications documented
- [ ] Developer guide updated

---

### Phase 3.2: Additional Algorithms (Week 16-17)

#### Optional: Add More Signature Schemes

**If resources available**:

1. **Secp256k1 (Bitcoin/Ethereum)**
   - Already in jstz_crypto!
   - Just needs JS binding
   - Useful for Web3 interop

2. **RSA-PSS / RSASSA-PKCS1-v1_5**
   - Lower priority
   - Larger implementation effort
   - Consider for Phase 4

#### Implementation: Secp256k1

```rust
// Minimal changes needed - already implemented!
#[op2(async)]
async fn op_crypto_verify(
    state: &mut OpState,
    #[serde] algorithm: serde_json::Value,
    key_handle: u32,
    #[buffer] signature: &[u8],
    #[buffer] data: &[u8],
) -> Result<bool, AnyError> {
    // ... existing code ...
    match key {
        // ... Ed25519, P256 ...
        StoredCryptoKey::Secp256k1Public(pk) => {
            let sig = jstz_crypto::signature::Secp256k1Signature::try_from(signature)?;
            Ok(pk.verify(&sig, data).is_ok())
        }
        // ...
    }
}
```

#### Acceptance Criteria
- [ ] Secp256k1 verify working (if added)
- [ ] WPT tests: 80-90 total passing
- [ ] Ethereum signature verification example

---

### Phase 3.3: Performance Optimization (Week 17-18)

#### Deliverables

1. **Benchmark Suite**
   - Measure: digest, sign, verify, encrypt, decrypt
   - Compare: different key sizes, algorithms
   - Target: < 100ms for typical operations

2. **Optimizations**
   - Use batch operations where possible
   - Cache CryptoKey objects
   - Optimize buffer copies

3. **Load Testing**
   - Multiple concurrent smart function calls
   - Ensure no memory leaks in CryptoKeyStore
   - Test key handle cleanup

#### Benchmark Example

```rust
#[bench]
fn bench_sha256_digest(b: &mut Bencher) {
    let data = vec![0u8; 1024]; // 1KB
    b.iter(|| {
        op_crypto_digest("SHA-256".to_string(), &data)
    });
}
```

#### Performance Targets

| Operation | Target Latency | Notes |
|-----------|----------------|-------|
| SHA-256 digest (1KB) | < 10ms | |
| Blake2b digest (1KB) | < 5ms | Should be faster than SHA |
| Ed25519 verify | < 50ms | Single signature |
| P-256 verify | < 100ms | ECDSA is slower |
| AES-GCM encrypt (1KB) | < 20ms | |
| HMAC-SHA256 (1KB) | < 10ms | |

#### Acceptance Criteria
- [ ] All benchmarks meet targets
- [ ] No memory leaks detected
- [ ] 1000+ concurrent operations handled
- [ ] Performance report documented

---

### Phase 3: Summary & Success Criteria

**Duration**: 2-4 weeks

**Key Deliverables**:
- ✅ Randomness strategy decided and implemented
- ✅ Secp256k1 support added (optional)
- ✅ Performance optimized and benchmarked
- ✅ Complete security documentation

**Success Metrics**:
- [ ] WPT pass rate: 40-50% (100-120 tests)
- [ ] All performance targets met
- [ ] Security review completed
- [ ] Production-ready quality

---

## Cross-Phase Concerns

### Testing Strategy

#### Test Pyramid

1. **Unit Tests (Rust)** - 80+ tests
   - Test each op in isolation
   - Test error conditions
   - Test edge cases (empty input, large input, invalid keys)

2. **Integration Tests (JavaScript)** - 30+ tests
   - Test full workflows
   - Test Web Crypto API compliance
   - Test smart function scenarios

3. **WPT Tests** - 100-120 passing
   - Official Web Crypto API test suite
   - Compliance verification
   - Regression detection

4. **Example Tests** - All examples must run
   - Verify examples in CI
   - Keep examples up-to-date

#### CI/CD Integration

```yaml
# .github/workflows/crypto-tests.yml
name: Crypto Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: Run Crypto Unit Tests
        run: cargo test --package jstz_runtime --lib jstz_crypto
      - name: Run Crypto Integration Tests
        run: cargo test --package jstz_runtime --test crypto_integration
      - name: Run WPT Crypto Tests
        run: cargo test --package jstz_api --test wpt -- WebCryptoAPI
      - name: Run Example Tests
        run: ./scripts/test-crypto-examples.sh
```

---

### Documentation Strategy

#### Documents to Create/Update

1. **API Reference** (`docs/api/crypto.md`)
   - Complete API surface
   - Algorithm support matrix
   - Parameter descriptions
   - Return value descriptions

2. **Developer Guide** (`docs/guides/using-crypto.md`)
   - Getting started
   - Common patterns
   - Best practices
   - Security considerations

3. **Security Guide** (`docs/guides/crypto-security.md`)
   - Deterministic crypto implications
   - IV/nonce management
   - Key management best practices
   - What NOT to do

4. **Migration Guide** (`docs/guides/crypto-migration.md`)
   - Differences from browser Web Crypto API
   - Porting code from Node.js/browser
   - Common pitfalls

5. **Examples** (`examples/crypto-*.js`)
   - Hashing data
   - Verifying signatures
   - JWT verification
   - Encrypting/decrypting data
   - HMAC authentication

---

### Security Considerations

#### Threat Model

**In Scope**:
- ✅ Signature verification (prevent signature forgery)
- ✅ Data integrity (hash verification)
- ✅ Authenticated encryption (AES-GCM)
- ✅ Message authentication (HMAC)

**Out of Scope** (by design):
- ❌ True random number generation (determinism required)
- ❌ Private key storage in smart functions (security risk)
- ❌ Key generation in smart functions (no randomness)

#### Security Review Checkpoints

**Phase 1 Review**:
- [ ] Signature verification cannot be bypassed
- [ ] No timing attacks in verification
- [ ] Key handles properly scoped (no cross-function access)

**Phase 2 Review**:
- [ ] HMAC comparison is constant-time
- [ ] AES-GCM authentication tag verified
- [ ] IV/nonce handling documented clearly

**Phase 3 Review**:
- [ ] Randomness strategy secure (or clearly documented as non-random)
- [ ] No key material leaked in errors
- [ ] All crypto operations audited

#### External Security Audit

**Recommendation**: Before production release:
- Hire external security firm
- Focus: cryptographic implementation correctness
- Scope: All crypto operations, key management, determinism

---

### Risk Management

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| **Determinism broken** | LOW | CRITICAL | Extensive testing, formal verification of seed |
| **Timing attacks** | MEDIUM | HIGH | Use constant-time libraries, audit |
| **Key handle leaks** | LOW | HIGH | OpState isolation, access control checks |
| **IV reuse (AES-GCM)** | MEDIUM | CRITICAL | Strong documentation, examples, linting |
| **WPT tests fail** | MEDIUM | MEDIUM | Incremental testing, early feedback |
| **Performance targets missed** | LOW | MEDIUM | Benchmarking throughout, Rust optimization |
| **Randomness decision stalled** | MEDIUM | HIGH | Force decision at Phase 3 start, document trade-offs |

---

## Resource Estimates

### Team Composition

**Recommended**:
- 1 Senior Rust Developer (lead)
- 1 Cryptography Expert (consultant)
- 1 JavaScript/TypeScript Developer
- 1 Technical Writer (documentation)
- 1 QA Engineer (testing)

**Minimum**:
- 1 Rust Developer with crypto knowledge
- Part-time technical writer

### Time Estimates by Role

| Phase | Rust Dev | Crypto Expert | JS Dev | Tech Writer | QA | Total Weeks |
|-------|----------|---------------|--------|-------------|-----|-------------|
| Phase 1.1 | 40h | 8h | 16h | - | - | 2 weeks |
| Phase 1.2 | 32h | 8h | 16h | 8h | 16h | 2 weeks |
| Phase 1.3 | 48h | 16h | 24h | 8h | 24h | 3 weeks |
| Phase 1.4 | 24h | 8h | 16h | - | 16h | 2 weeks |
| Phase 1.5 | 8h | - | 16h | 40h | 16h | 2 weeks |
| **Phase 1 Total** | **152h** | **40h** | **88h** | **56h** | **72h** | **6-8 weeks** |
| Phase 2.1 | 32h | 16h | 16h | 8h | 16h | 2 weeks |
| Phase 2.2 | 48h | 24h | 24h | 16h | 24h | 3 weeks |
| Phase 2.3 | 40h | 16h | 16h | 8h | 16h | 2 weeks |
| **Phase 2 Total** | **120h** | **56h** | **56h** | **32h** | **56h** | **4-6 weeks** |
| Phase 3.1 | 24h | 40h | 16h | 16h | 16h | 2 weeks |
| Phase 3.2 | 16h | 8h | 8h | - | 16h | 1 week |
| Phase 3.3 | 32h | - | - | 8h | 24h | 2 weeks |
| **Phase 3 Total** | **72h** | **48h** | **24h** | **24h** | **56h** | **2-4 weeks** |
| **GRAND TOTAL** | **344h** | **144h** | **168h** | **112h** | **184h** | **12-18 weeks** |

### Budget Estimate

**Assumptions**:
- Senior Rust Dev: $150/hour
- Crypto Expert: $200/hour
- JS Developer: $100/hour
- Technical Writer: $80/hour
- QA Engineer: $90/hour

| Role | Hours | Rate | Cost |
|------|-------|------|------|
| Rust Development | 344h | $150/h | $51,600 |
| Crypto Consultation | 144h | $200/h | $28,800 |
| JS Development | 168h | $100/h | $16,800 |
| Documentation | 112h | $80/h | $8,960 |
| QA/Testing | 184h | $90/h | $16,560 |
| **Total** | **952h** | | **$122,720** |

**Additional Costs**:
- Security audit: $20,000-$50,000
- Tools/infrastructure: $2,000
- Contingency (20%): $24,544

**Grand Total**: ~$150,000-$200,000

---

## Success Criteria & KPIs

### Phase Completion Criteria

**Phase 1 Complete**:
- ✅ WPT pass rate ≥ 15%
- ✅ All TIER 1 hash algorithms working
- ✅ Ed25519 + P-256 verification working
- ✅ 3+ example smart functions
- ✅ API documentation complete

**Phase 2 Complete**:
- ✅ WPT pass rate ≥ 30%
- ✅ HMAC + AES-GCM working
- ✅ Real-world use cases demonstrated
- ✅ Security documentation complete

**Phase 3 Complete**:
- ✅ WPT pass rate ≥ 40%
- ✅ Randomness strategy finalized
- ✅ Performance benchmarks met
- ✅ Production-ready quality

### Developer Experience Metrics

**Measurement** (after Phase 1):
- Developer survey: "How easy is crypto API to use?" (target: 4/5)
- Number of support questions (target: < 10/month)
- Time to implement JWT verification (target: < 30 minutes)

### Performance Metrics

**Tracked Throughout**:
- Latency: p50, p95, p99 for each operation
- Throughput: operations/second
- Memory: peak usage, leak detection
- Smart function execution time impact

---

## Rollout Plan

### Beta Release (After Phase 1)

**Target**: Select group of developers
**Purpose**: Gather feedback, find bugs
**Duration**: 2-4 weeks

**Activities**:
- Private beta announcement
- Developer support channel
- Weekly feedback sessions
- Bug bounty program

### Public Preview (After Phase 2)

**Target**: All developers (non-production use)
**Purpose**: Wide testing, documentation improvement
**Duration**: 4-6 weeks

**Activities**:
- Public announcement
- Tutorial videos
- Sample applications
- Developer office hours

### Production Release (After Phase 3)

**Target**: Production workloads
**Prerequisites**:
- Security audit complete
- Documentation finalized
- WPT pass rate ≥ 40%
- No critical bugs

**Activities**:
- Release announcement
- Migration guides
- Production support
- Monitoring & alerts

---

## Appendix A: Detailed File Structure

```
crates/jstz_runtime/
├── src/
│   └── ext/
│       └── jstz_crypto/
│           ├── mod.rs              # Extension definition, ops registration
│           ├── crypto.js           # Crypto interface (global.crypto)
│           ├── subtle.js           # SubtleCrypto implementation
│           ├── key.rs              # CryptoKey management
│           ├── key.js              # CryptoKey JavaScript wrapper
│           └── algorithms/
│               ├── digest.rs       # Hashing ops
│               ├── sign.rs         # Signature ops
│               ├── encrypt.rs      # Encryption ops
│               └── derive.rs       # Key derivation ops
└── tests/
    ├── crypto_digest_test.rs
    ├── crypto_sign_verify_test.rs
    ├── crypto_encrypt_test.rs
    └── crypto_integration_test.rs

crates/jstz_crypto/
└── src/
    ├── lib.rs
    ├── hmac.rs                     # HMAC implementation (new)
    ├── aes.rs                      # AES-GCM implementation (new)
    └── pbkdf2.rs                   # PBKDF2 implementation (new)

docs/
├── api/
│   ├── crypto.md                   # Crypto API reference (new)
│   └── crypto-key.md               # CryptoKey interface docs (new)
├── guides/
│   ├── using-crypto.md             # Developer guide (new)
│   ├── crypto-security.md          # Security best practices (new)
│   └── crypto-migration.md         # Migration guide (new)
└── tutorials/
    └── jwt-verification.md         # Step-by-step tutorial (new)

examples/
├── crypto_hash.js                  # Hashing example (new)
├── crypto_verify_signature.js      # Signature verification (new)
├── crypto_verify_jwt.js            # JWT verification (new)
├── crypto_hmac_auth.js             # HMAC authentication (new)
└── crypto_encrypt_data.js          # Encryption example (new)
```

---

## Appendix B: Decision Log Template

Use this template to track key decisions:

| Date | Decision | Options Considered | Rationale | Decider | Status |
|------|----------|-------------------|-----------|---------|--------|
| 2025-XX-XX | Randomness Strategy | A) Strict determinism<br>B) Seeded PRNG<br>C) Oracle | Maintains rollup integrity | [Name] | Approved |
| 2025-XX-XX | Extension approach | A) deno_crypto<br>B) Custom jstz_crypto | Full control over determinism | [Name] | Approved |

---

## Appendix C: Questions for Stakeholders

Before starting implementation, get answers to:

1. **Randomness Strategy**:
   - Q: Is deterministic PRNG acceptable for key generation?
   - Q: Should we allow ANY key generation in smart functions?
   - Decision deadline: Before Phase 3 starts

2. **Algorithm Priority**:
   - Q: Is Secp256k1 support required for Phase 1? (Ethereum/Bitcoin compatibility)
   - Q: Are RSA algorithms needed? (larger implementation effort)
   - Decision deadline: Before Phase 2 starts

3. **Performance Requirements**:
   - Q: What are acceptable latency targets for crypto operations?
   - Q: How many concurrent smart functions will use crypto?
   - Decision deadline: Before Phase 3 performance optimization

4. **Security Audit**:
   - Q: Which audit firm to use?
   - Q: What is the budget for external audit?
   - Q: When should audit be scheduled? (recommend: after Phase 2)
   - Decision deadline: Before Phase 3 starts

5. **Breaking Changes**:
   - Q: Can we make breaking changes to API between phases?
   - Q: What is the deprecation policy?
   - Decision deadline: Before Phase 1 starts

---

## Document Metadata

- **Version**: 1.0
- **Date**: 2025-11-18
- **Author**: Claude Code Analysis
- **Status**: Draft - Requires Review
- **Next Review**: [To be scheduled]
- **Approvers**: [To be assigned]

---

**END OF IMPLEMENTATION PLAN**
