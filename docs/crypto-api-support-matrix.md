# jstz Crypto API Support Matrix

## Executive Summary

This document provides a comprehensive analysis of Web Crypto API support in jstz smart functions as of **November 18, 2025**.

### Current Status: ❌ **NO WEB CRYPTO API SUPPORT**

jstz smart functions currently have **zero** access to Web Crypto API functionality from JavaScript. All cryptographic operations are available only at the protocol level (Rust implementation) and are not exposed to smart function code.

### Coverage Metrics

| Category | TIER 1 (Must Have) | TIER 2 (Should Have) | TIER 3 (Optional) | Total |
|----------|-------------------|---------------------|-------------------|-------|
| **Implemented** | 0% | 0% | 0% | 0% |
| **Partially Available (Protocol Level)** | 33% | 25% | 0% | 29% |
| **Not Implemented** | 67% | 75% | 100% | 71% |

**Note**: "Partially Available (Protocol Level)" refers to features implemented in Rust (`jstz_crypto` crate) but not exposed to JavaScript smart functions.

---

## 1. Core Interfaces

### 1.1 Crypto Interface

| Feature | Tier | jstz Status | Reference | Notes |
|---------|------|-------------|-----------|-------|
| `crypto` global object | 1 | ❌ Not Implemented | - | Not exposed in global scope |
| `crypto.subtle` | 1 | ❌ Not Implemented | - | SubtleCrypto interface not available |
| `crypto.getRandomValues()` | 1 | ❌ **Intentionally Disabled** | `jstz_core/src/runtime.rs:59-66` | Returns error for determinism |
| `crypto.randomUUID()` | 2 | ❌ Not Implemented | - | Would require randomness |

**Evidence:**
```rust
// File: crates/jstz_core/src/runtime.rs (lines 59-66)
const GETRANDOM_ERROR_CODE: u32 = RandomError::CUSTOM_START + 42;
fn always_fail(_: &mut [u8]) -> std::result::Result<(), RandomError> {
    let code = NonZeroU32::new(GETRANDOM_ERROR_CODE).unwrap();
    Err(RandomError::from(code))
}
register_custom_getrandom!(always_fail);
```

**Global Scope Check:**
```javascript
// File: crates/jstz_runtime/src/ext/jstz_main/98_global_scope.js
// No 'crypto' object defined in workerGlobalScope
```

**WPT Test Results:**
```
// File: crates/jstz_runtime/tests/wptreport.json
Error: "crypto is not defined"
Error: "Cannot read properties of undefined (reading 'getRandomValues')"
```

### 1.2 SubtleCrypto Interface

| Feature | Tier | jstz Status | Reference | Notes |
|---------|------|-------------|-----------|-------|
| SubtleCrypto interface | 1 | ❌ Not Implemented | - | No deno_crypto extension included |

---

## 2. Cryptographic Operations

### 2.1 Hashing (Digest)

| Algorithm | Tier | JavaScript API | Protocol Level (Rust) | References |
|-----------|------|----------------|----------------------|------------|
| SHA-256 | 1 | ❌ Not Available | ⚠️ Via cryptoxide | `jstz_crypto/Cargo.toml:14` (cryptoxide dependency) |
| SHA-384 | 1 | ❌ Not Available | ⚠️ Via cryptoxide | `jstz_crypto/Cargo.toml:14` |
| SHA-512 | 1 | ❌ Not Available | ⚠️ Via cryptoxide | `jstz_crypto/Cargo.toml:14` |
| SHA-1 | 2 | ❌ Not Available | ⚠️ Via cryptoxide | `jstz_crypto/Cargo.toml:14` |
| Blake2b | N/A | ❌ Not Available | ✅ **Implemented** | `jstz_crypto/src/hash.rs` |

**Protocol-Level Implementation:**
```rust
// File: crates/jstz_crypto/src/hash.rs
pub struct Blake2b(pub [u8; 32]);

impl Blake2b {
    pub fn from(data: &[u8]) -> Self {
        Self(blake2b(data, 32).try_into().unwrap())
    }
}
```

**Notes:**
- Blake2b is actively used for Tezos address hashing (smart rollup addresses)
- SHA family available through `cryptoxide` dependency but not exposed to JS
- `crypto.subtle.digest()` not implemented for any algorithm

### 2.2 Encryption/Decryption

| Algorithm | Tier | JavaScript API | Protocol Level | Status |
|-----------|------|----------------|----------------|--------|
| AES-GCM (128) | 1 | ❌ Not Available | ❌ Not Implemented | - |
| AES-GCM (192) | 1 | ❌ Not Available | ❌ Not Implemented | - |
| AES-GCM (256) | 1 | ❌ Not Available | ❌ Not Implemented | - |
| AES-CBC | 2 | ❌ Not Available | ❌ Not Implemented | - |
| AES-CTR | 2 | ❌ Not Available | ❌ Not Implemented | - |
| RSA-OAEP | 2 | ❌ Not Available | ❌ Not Implemented | - |

**Status:** Zero encryption/decryption support at any level.

### 2.3 Digital Signatures

| Algorithm | Tier | JavaScript API | Protocol Level (Rust) | References |
|-----------|------|----------------|----------------------|------------|
| **Ed25519** | 1 | ❌ Not Available | ✅ **Fully Implemented** | `jstz_crypto/src/public_key.rs:95-121` |
| **ECDSA P-256** | 1 | ❌ Not Available | ✅ **Fully Implemented** | `jstz_crypto/src/public_key.rs:123-150` |
| ECDSA P-384 | 2 | ❌ Not Available | ❌ Not Implemented | - |
| ECDSA P-521 | 2 | ❌ Not Available | ❌ Not Implemented | - |
| **HMAC** | 1 | ❌ Not Available | ❌ Not Implemented | - |
| RSA-PSS | 2 | ❌ Not Available | ❌ Not Implemented | - |
| RSASSA-PKCS1-v1_5 | 2 | ❌ Not Available | ❌ Not Implemented | - |
| **Secp256k1** | N/A | ❌ Not Available | ✅ **Fully Implemented** | `jstz_crypto/src/public_key.rs:152-179` |

**Protocol-Level Implementation Examples:**

```rust
// File: crates/jstz_crypto/src/public_key.rs (lines 95-121)
impl Ed25519 {
    pub fn verify(
        &self,
        signature: &Ed25519Signature,
        msg: &[u8],
    ) -> Result<(), CryptoError> {
        // Ed25519 signature verification implementation
    }
}

// File: crates/jstz_crypto/src/public_key.rs (lines 123-150)
impl P256 {
    pub fn verify(
        &self,
        signature: &P256Signature,
        msg: &[u8],
    ) -> Result<(), CryptoError> {
        // P256 ECDSA signature verification
    }
}

// File: crates/jstz_crypto/src/public_key.rs (lines 152-179)
impl Secp256k1 {
    pub fn verify(
        &self,
        signature: &Secp256k1Signature,
        msg: &[u8],
    ) -> Result<(), CryptoError> {
        // Secp256k1 signature verification
    }
}
```

**Signature Operations:**
```rust
// File: crates/jstz_crypto/src/secret_key.rs
impl SecretKey {
    pub fn sign(&self, msg: &[u8]) -> Result<Signature, CryptoError> {
        // Signing implementation for Ed25519, Secp256k1, P256
    }
}
```

**Notes:**
- Ed25519, P-256, and Secp256k1 are fully implemented at the protocol level
- Used for transaction signing and verification
- Not accessible from JavaScript smart functions via `crypto.subtle.sign()` or `verify()`
- Secp256k1 is a Tezos/blockchain-specific addition (not part of Web Crypto spec)

### 2.4 Key Derivation

| Algorithm | Tier | JavaScript API | Protocol Level | References |
|-----------|------|----------------|----------------|------------|
| PBKDF2 | 1 | ❌ Not Available | ⚠️ Via bip39 | `jstz_crypto/Cargo.toml:15` |
| HKDF | 2 | ❌ Not Available | ❌ Not Implemented | - |
| ECDH | 2 | ❌ Not Available | ❌ Not Implemented | - |
| X25519 | 2 | ❌ Not Available | ❌ Not Implemented | - |

**BIP39 Mnemonic Support (Protocol Level):**
```rust
// File: crates/jstz_crypto/src/secret_key.rs
impl SecretKey {
    pub fn from_mnemonic(mnemonic: &str, derivation_path: &str, passphrase: &str)
        -> Result<Self, CryptoError> {
        // BIP39 mnemonic to secret key derivation
    }
}
```

**Notes:**
- BIP39 key derivation available at protocol level (uses PBKDF2 internally)
- Standard Web Crypto `deriveBits()` / `deriveKey()` not available

### 2.5 Key Management

#### 2.5.1 Key Generation

| Feature | Tier | JavaScript API | Protocol Level | Status |
|---------|------|----------------|----------------|--------|
| `generateKey()` for AES-GCM | 1 | ❌ Not Available | ❌ Not Implemented | - |
| `generateKey()` for HMAC | 1 | ❌ Not Available | ❌ Not Implemented | - |
| `generateKey()` for Ed25519 | 2 | ❌ Not Available | ⚠️ Limited Support | Protocol uses derived keys |
| `generateKey()` for ECDSA | 2 | ❌ Not Available | ⚠️ Limited Support | Protocol uses derived keys |
| `generateKey()` for RSA | 2 | ❌ Not Available | ❌ Not Implemented | - |

**Notes:**
- Key generation would require random number generation (currently disabled for determinism)
- Protocol generates keys from mnemonics/seeds rather than random generation

#### 2.5.2 Key Import/Export

| Format | Tier | JavaScript API | Protocol Level | References |
|--------|------|----------------|----------------|------------|
| `raw` | 1 | ❌ Not Available | ⚠️ Partial | Keys stored as raw bytes internally |
| `jwk` | 1 | ❌ Not Available | ❌ Not Implemented | - |
| `spki` | 2 | ❌ Not Available | ❌ Not Implemented | - |
| `pkcs8` | 2 | ❌ Not Available | ❌ Not Implemented | - |
| Base58 (Tezos) | N/A | ❌ Not Available | ✅ **Implemented** | `jstz_crypto/src/public_key.rs` |

**Base58 Encoding (Protocol Level):**
```rust
// File: crates/jstz_crypto/src/public_key.rs
impl PublicKey {
    pub fn to_base58(&self) -> String {
        // Tezos Base58 encoding
    }

    pub fn from_base58(data: &str) -> Result<Self, CryptoError> {
        // Tezos Base58 decoding
    }
}
```

**Notes:**
- Tezos-specific Base58 encoding/decoding implemented
- Standard Web Crypto key formats (JWK, SPKI, PKCS#8) not supported
- No `importKey()` or `exportKey()` methods available in JavaScript

#### 2.5.3 Key Wrapping

| Feature | Tier | JavaScript API | Protocol Level | Status |
|---------|------|----------------|----------------|--------|
| `wrapKey()` / `unwrapKey()` | 2-3 | ❌ Not Available | ❌ Not Implemented | - |
| AES-KW | 2 | ❌ Not Available | ❌ Not Implemented | - |

---

## 3. Supporting Types

### 3.1 CryptoKey Interface

| Feature | Tier | JavaScript API | Protocol Level | Notes |
|---------|------|----------------|----------------|-------|
| CryptoKey type | 1 | ❌ Not Available | ⚠️ Implicit | Keys typed as Ed25519, Secp256k1, P256 |
| `type` property | 1 | ❌ Not Available | - | - |
| `extractable` property | 1 | ❌ Not Available | - | - |
| `algorithm` property | 1 | ❌ Not Available | - | - |
| `usages` property | 1 | ❌ Not Available | - | - |

### 3.2 CryptoKeyPair Dictionary

| Feature | Tier | JavaScript API | Protocol Level | Status |
|---------|------|----------------|----------------|--------|
| CryptoKeyPair | 2 | ❌ Not Available | ❌ Not Implemented | - |

---

## 4. Determinism Considerations

### 4.1 Intentionally Disabled Features

jstz is designed for deterministic execution in a smart rollup environment. The following features are **intentionally disabled**:

| Feature | Location | Reason |
|---------|----------|--------|
| `getrandom` syscall | `jstz_core/src/runtime.rs:59-66` | Non-deterministic random generation |
| `Math.random()` | `jstz_runtime/src/ext/jstz_main/98_global_scope.js:32-35` | Returns constant `0.42` |
| `Date.now()` | Runtime configuration | Returns fixed timestamp |

**Math.random Override:**
```javascript
// File: crates/jstz_runtime/src/ext/jstz_main/98_global_scope.js (lines 32-35)
let GlobalMath = Math;
GlobalMath.random = () => {
  return 0.42;  // Constant value for determinism
};
```

### 4.2 Impact on Web Crypto API

The following Web Crypto operations are **incompatible with deterministic execution** without modifications:

| Operation | Reason | Possible Solution |
|-----------|--------|------------------|
| `crypto.getRandomValues()` | Requires CSPRNG | Seeded PRNG from transaction hash |
| `crypto.randomUUID()` | Requires randomness | Seeded UUID generation |
| `generateKey()` (symmetric) | Requires random key material | Seeded key generation or user-provided entropy |
| `generateKey()` (asymmetric) | Requires random key material | Seeded key generation or user-provided entropy |

---

## 5. Protocol-Level Crypto Architecture

### 5.1 jstz_crypto Crate Structure

```
crates/jstz_crypto/
├── src/
│   ├── lib.rs              - Public exports
│   ├── public_key.rs       - ✅ Ed25519, Secp256k1, P256 public keys
│   ├── secret_key.rs       - ✅ Secret key management and signing
│   ├── signature.rs        - ✅ Signature types and verification
│   ├── hash.rs             - ✅ Blake2b hashing
│   ├── error.rs            - Error types
│   └── verifier/
│       ├── mod.rs          - Verifier trait
│       └── passkey.rs      - ✅ WebAuthn/Passkey verification
└── Cargo.toml              - Dependencies
```

### 5.2 Dependencies

| Dependency | Version | Purpose | Reference |
|------------|---------|---------|-----------|
| `tezos_crypto_rs` | 0.6 | Core Tezos crypto primitives | `Cargo.toml:12` |
| `libsecp256k1` | 0.7 | Secp256k1 operations | `Cargo.toml:13` |
| `cryptoxide` | 0.4 | SHA2, Blake2 hashing | `Cargo.toml:14` |
| `bip39` | 2.1 | BIP39 mnemonic support | `Cargo.toml:15` |
| `p256` | 0.13 | P256 curve operations | `Cargo.toml:18` |

**Cargo.toml excerpt:**
```toml
# File: crates/jstz_crypto/Cargo.toml (lines 12-18)
tezos_crypto_rs = { version = "0.6", default-features = false }
libsecp256k1 = "0.7"
cryptoxide = "0.4"
bip39 = "2.1"
p256 = { version = "0.13", features = ["ecdsa"] }
```

### 5.3 Current Usage

The `jstz_crypto` crate is used for:

1. **Transaction Signing & Verification** (`jstz_proto`)
   - Smart function deployment signatures
   - Operation signatures
   - Batch operation verification

2. **Account Management** (`jstz_cli`, `jstz_node`)
   - Key generation from mnemonics
   - Address derivation
   - Account authentication

3. **Smart Rollup Operations** (`jstz_proto`)
   - Address hashing (Blake2b)
   - Public key hashing
   - Operation hashing

**Example Usage:**
```rust
// File: Various protocol-level files
use jstz_crypto::{PublicKey, SecretKey, Signature};

// Sign operation
let secret_key = SecretKey::from_mnemonic(...)?;
let signature = secret_key.sign(&data)?;

// Verify operation
let public_key = PublicKey::from_base58(...)?;
public_key.verify(&signature, &data)?;
```

---

## 6. Web Platform Tests (WPT) Status

### 6.1 Test Configuration

**Location:** `crates/jstz_api/tests/wpt.rs`

```rust
// File: crates/jstz_api/tests/wpt.rs (line 371)
wpt_test!("WebCryptoAPI", crypto);
```

### 6.2 Current Test Results

**Status:** ❌ **ALL TESTS FAILING**

**Common Errors:**
- `ReferenceError: crypto is not defined`
- `TypeError: Cannot read properties of undefined (reading 'getRandomValues')`
- `TypeError: Cannot read properties of undefined (reading 'subtle')`

**Sample from `crates/jstz_runtime/tests/wptreport.json`:**
```json
{
  "test": "/WebCryptoAPI/derive_bits_keys/pbkdf2.https.any.js",
  "status": "ERROR",
  "message": "ReferenceError: crypto is not defined"
}
```

### 6.3 Test Coverage

The WPT suite includes tests for:
- ❌ `getRandomValues()`
- ❌ `randomUUID()`
- ❌ All SubtleCrypto methods
- ❌ All algorithms (SHA, AES, RSA, ECDSA, Ed25519, etc.)
- ❌ Key import/export
- ❌ Key derivation
- ❌ Key wrapping

**Total Coverage:** 0% passing

---

## 7. Gap Analysis

### 7.1 TIER 1 (Must Have) - Critical Gaps

| Feature | Status | Blocker | Impact |
|---------|--------|---------|--------|
| `crypto` global object | ❌ Missing | No deno_crypto extension | **CRITICAL** - No crypto access at all |
| `crypto.subtle` | ❌ Missing | No deno_crypto extension | **CRITICAL** - No crypto operations |
| `crypto.getRandomValues()` | ❌ Disabled | Determinism requirement | **CRITICAL** - Needs seeded PRNG solution |
| SHA-256/384/512 digest | ❌ Missing | No JS API | **HIGH** - Common hashing needs |
| AES-GCM encryption | ❌ Missing | No implementation | **HIGH** - Data encryption |
| Ed25519 sign/verify | ⚠️ Partial | Protocol-level only | **HIGH** - Signature verification in smart functions |
| ECDSA P-256 sign/verify | ⚠️ Partial | Protocol-level only | **HIGH** - Signature verification |
| HMAC | ❌ Missing | No implementation | **HIGH** - Message authentication |
| PBKDF2 derivation | ⚠️ Partial | BIP39 only, no JS API | **MEDIUM** - Password-based crypto |
| Key import/export (raw, JWK) | ❌ Missing | No implementation | **HIGH** - Key management |
| CryptoKey interface | ❌ Missing | No implementation | **CRITICAL** - Core type |

**TIER 1 Implementation Rate: 0% (0/12 features available in JavaScript)**

### 7.2 TIER 2 (Should Have) - Important Gaps

| Feature | Status | Impact |
|---------|--------|--------|
| `crypto.randomUUID()` | ❌ Missing | **MEDIUM** - UUID generation |
| AES-CBC/CTR | ❌ Missing | **MEDIUM** - Legacy encryption compatibility |
| RSA-OAEP | ❌ Missing | **LOW** - Asymmetric encryption |
| ECDSA P-384/P-521 | ❌ Missing | **LOW** - Enhanced security curves |
| RSA-PSS / RSASSA-PKCS1-v1_5 | ❌ Missing | **LOW** - RSA signatures |
| HKDF | ❌ Missing | **MEDIUM** - Key derivation |
| ECDH / X25519 | ❌ Missing | **MEDIUM** - Key agreement |
| Key wrapping (AES-KW) | ❌ Missing | **LOW** - Secure key storage |
| SPKI/PKCS#8 formats | ❌ Missing | **MEDIUM** - Interoperability |

**TIER 2 Implementation Rate: 0% (0/9 features available)**

### 7.3 Available Assets (Protocol Level)

Despite zero JavaScript exposure, jstz has strong protocol-level crypto:

| Asset | Quality | Location | Reusability |
|-------|---------|----------|-------------|
| Ed25519 implementation | ✅ Production-ready | `jstz_crypto/src/public_key.rs:95-121` | High |
| Secp256k1 implementation | ✅ Production-ready | `jstz_crypto/src/public_key.rs:152-179` | High |
| P256 implementation | ✅ Production-ready | `jstz_crypto/src/public_key.rs:123-150` | High |
| Blake2b hashing | ✅ Production-ready | `jstz_crypto/src/hash.rs` | High |
| Signature verification | ✅ Production-ready | `jstz_crypto/src/signature.rs` | High |
| Base58 encoding/decoding | ✅ Production-ready | `jstz_crypto/src/public_key.rs` | Medium |
| BIP39 key derivation | ✅ Production-ready | `jstz_crypto/src/secret_key.rs` | Medium |
| SHA family (via cryptoxide) | ✅ Available | `Cargo.toml` dependency | High |

---

## 8. Comparative Analysis

### 8.1 Comparison with Other JavaScript Runtimes

| Runtime | Web Crypto API | Secure Contexts | Determinism | Notes |
|---------|----------------|-----------------|-------------|-------|
| **Browser** | ✅ Full support | HTTPS required | Non-deterministic | Standard implementation |
| **Node.js** | ✅ Full support (v15+) | No restriction | Non-deterministic | Global `crypto.webcrypto` |
| **Deno** | ✅ Full support | No restriction | Non-deterministic | Global `crypto` |
| **Cloudflare Workers** | ✅ Full support | HTTPS only | Non-deterministic | Web standard APIs |
| **jstz** | ❌ **Not implemented** | N/A | Deterministic | Protocol-level crypto only |

### 8.2 Smart Contract Platform Comparison

| Platform | Crypto Primitives | Determinism | Random Generation |
|----------|------------------|-------------|-------------------|
| **Ethereum (EVM)** | Keccak-256, ecrecover | Deterministic | No random (uses blockhash) |
| **Solana** | Ed25519, SHA-256 | Deterministic | Seeded random via Sysvar |
| **Tezos (Michelson)** | Ed25519, Blake2b | Deterministic | No random |
| **jstz** | None (from JS) | Deterministic | No random (disabled) |

**Observation:** Most smart contract platforms provide **limited but essential** crypto primitives for signature verification and hashing, but not full Web Crypto API support.

---

## 9. Recommendations

### 9.1 Priority 1: Minimum Viable Crypto API

Implement a **determinism-safe subset** of Web Crypto API:

**Phase 1A: Core Hashing**
- ✅ `crypto.subtle.digest()` for SHA-256, SHA-384, SHA-512, Blake2b
- Reuse existing `cryptoxide` dependency
- Zero randomness required ✓

**Phase 1B: Signature Verification**
- ✅ `crypto.subtle.verify()` for Ed25519, ECDSA P-256
- Expose existing `jstz_crypto` implementations
- ✅ `crypto.subtle.importKey()` for raw public keys
- Zero randomness required ✓

**Phase 1C: HMAC**
- ✅ `crypto.subtle.sign()` / `verify()` for HMAC-SHA256
- User provides key material
- Zero randomness required ✓

### 9.2 Priority 2: Deterministic Key Operations

**Phase 2A: Key Import**
- ✅ `crypto.subtle.importKey()` for JWK format
- Support Ed25519, ECDSA P-256, HMAC keys
- No generation, import only

**Phase 2B: PBKDF2**
- ✅ `crypto.subtle.deriveKey()` / `deriveBits()` for PBKDF2
- Deterministic password-based derivation
- Reuse existing BIP39 infrastructure

### 9.3 Priority 3: Seeded Randomness (Controversial)

**Option A: Transaction-Seeded PRNG**
- Implement `crypto.getRandomValues()` using deterministic PRNG
- Seed from: `transaction_hash || block_hash || nonce`
- **Pros:** Enables key generation, UUID, etc.
- **Cons:** Not cryptographically secure random; same inputs = same outputs

**Option B: Oracle-Based Randomness**
- Route random generation through fetch oracle
- External randomness source (e.g., drand)
- **Pros:** True randomness available
- **Cons:** Breaks determinism; adds latency; oracle dependency

**Option C: User-Provided Entropy**
- Require users to provide entropy in function calls
- `generateKey(algorithm, entropy, extractable, usages)`
- **Pros:** Maintains determinism with user control
- **Cons:** Non-standard API; complexity for developers

### 9.4 Architectural Decision Required

**CRITICAL QUESTION:** Should jstz support operations requiring randomness?

| Approach | Determinism | Web Crypto Compliance | Developer Experience | Recommendation |
|----------|-------------|----------------------|---------------------|----------------|
| **Strict Determinism** (Phase 1-2 only) | ✅ Preserved | ⚠️ Partial API | Good for common cases | **Recommended for MVP** |
| **Seeded PRNG** (Add Phase 3A) | ✅ Preserved | ⚠️ Non-compliant behavior | Easy but misleading | Consider with clear docs |
| **Oracle Random** (Add Phase 3B) | ❌ Broken | ✅ Compliant behavior | Complex, slower | Not recommended |
| **User Entropy** (Add Phase 3C) | ✅ Preserved | ❌ Non-standard API | Complex for developers | Consider for advanced use |

---

## 10. Conclusion

### Current State Summary

jstz has **zero Web Crypto API support** for JavaScript smart functions, despite having robust protocol-level cryptographic capabilities in Rust. The platform's deterministic execution model is fundamentally incompatible with the random number generation required by many Web Crypto operations.

### Immediate Action Items

1. **Decide on determinism policy** for crypto operations (strict vs. seeded)
2. **Implement Phase 1A-C** (deterministic subset) as MVP
3. **Expose existing crypto assets** (Ed25519, P256, Blake2b) to JavaScript
4. **Document limitations** clearly for smart function developers
5. **Update WPT tests** to reflect supported subset

### Success Metrics

**Minimal Success (3-6 months):**
- ✅ 40-50% TIER 1 coverage (hashing + signature verification)
- ✅ WPT pass rate: 20-30% (deterministic tests only)
- ✅ Developer documentation for crypto usage

**Full Success (6-12 months):**
- ✅ 80-90% TIER 1 coverage (all deterministic operations)
- ✅ 30-40% TIER 2 coverage (key management, advanced features)
- ✅ WPT pass rate: 40-50%
- ✅ Clear migration path for Web Crypto code

---

## References

### jstz Codebase References

**Core Crypto:**
- `crates/jstz_crypto/src/public_key.rs` - Public key implementations
- `crates/jstz_crypto/src/secret_key.rs` - Secret key and signing
- `crates/jstz_crypto/src/signature.rs` - Signature types
- `crates/jstz_crypto/src/hash.rs` - Blake2b hashing
- `crates/jstz_crypto/Cargo.toml` - Crypto dependencies

**Runtime:**
- `crates/jstz_runtime/src/runtime.rs` - Runtime initialization
- `crates/jstz_runtime/src/ext/jstz_main/98_global_scope.js` - Global scope
- `crates/jstz_core/src/runtime.rs:59-66` - Random number disabling

**Tests:**
- `crates/jstz_api/tests/wpt.rs:371` - WebCryptoAPI test enablement
- `crates/jstz_runtime/tests/wptreport.json` - Test failure results

### External References

- W3C Web Cryptography API: https://w3c.github.io/webcrypto/
- MDN Web Crypto API: https://developer.mozilla.org/en-US/docs/Web/API/Web_Crypto_API
- Deno Crypto: https://github.com/denoland/deno/tree/main/ext/crypto
- Web Platform Tests: https://github.com/web-platform-tests/wpt/tree/master/WebCryptoAPI

---

## Document Metadata

- **Version:** 1.0
- **Date:** November 18, 2025
- **Author:** Claude Code Analysis
- **jstz Commit:** 1d583d8 (fix(docs): remove sidebar padding)
- **Branch:** claude/research-crypto-api-01SXTsLTRCv5BfQC1PGhe2XD
