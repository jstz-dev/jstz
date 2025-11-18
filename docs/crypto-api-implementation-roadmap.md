# Web Crypto API Implementation Roadmap for jstz

## Current State

- **JavaScript API**: ❌ None (0% Web Crypto API support)
- **Protocol Level**: ✅ Ed25519, P256, Secp256k1, Blake2b (Rust only)
- **Blocker**: No `deno_crypto` extension integrated
- **Constraint**: Deterministic execution required (no true randomness)

---

## Phase 1: Core Deterministic Operations (MVP)

### 1.1 Add deno_crypto Extension

**What:** Integrate Deno's crypto extension into jstz runtime

**Technical Context:**
- Add `deno_crypto` dependency to `crates/jstz_runtime/Cargo.toml`
- Register extension in `crates/jstz_runtime/src/runtime.rs` (~line 490-516)
- Expose `crypto` object in `crates/jstz_runtime/src/ext/jstz_main/98_global_scope.js`

**Implementation:**
```rust
// In crates/jstz_runtime/Cargo.toml
deno_crypto = "0.x"  // Add to dependencies

// In crates/jstz_runtime/src/runtime.rs
use deno_crypto::deno_crypto;

extensions.push(
    deno_crypto::deno_crypto::init_ops_and_esm(/* config */),
);
```

```javascript
// In crates/jstz_runtime/src/ext/jstz_main/98_global_scope.js
import * as crypto from "ext:deno_crypto/00_crypto.js";
workerGlobalScope.crypto = crypto;
```

**Challenges:**
- `deno_crypto` uses getrandom internally (conflicts with determinism)
- Need to patch or configure to disable random operations

---

### 1.2 Implement crypto.subtle.digest()

**What:** Hash functions (SHA-256, SHA-384, SHA-512, Blake2b)

**Technical Context:**
- Reuse existing `cryptoxide` dependency from `jstz_crypto`
- Expose Blake2b from `jstz_crypto/src/hash.rs`
- Zero randomness needed ✓

**Implementation Approach:**

**Option A:** Use deno_crypto's digest (if it works without getrandom)
- Leverage existing implementation
- May already support SHA family

**Option B:** Custom op binding to jstz_crypto
```rust
// In crates/jstz_runtime/src/ext/jstz_crypto/mod.rs (new)
#[op2]
#[serde]
fn op_crypto_digest(
    #[string] algorithm: String,
    #[buffer] data: &[u8],
) -> Result<Vec<u8>, AnyError> {
    match algorithm.as_str() {
        "SHA-256" => Ok(sha256(data).to_vec()),
        "SHA-384" => Ok(sha384(data).to_vec()),
        "SHA-512" => Ok(sha512(data).to_vec()),
        "BLAKE2B-256" => Ok(Blake2b::from(data).0.to_vec()),
        _ => Err(anyhow!("Unsupported algorithm")),
    }
}
```

**Files to Create/Modify:**
- `crates/jstz_runtime/src/ext/jstz_crypto/mod.rs` (new extension)
- `crates/jstz_runtime/src/ext/jstz_crypto/crypto.js` (new, JS bindings)
- `crates/jstz_runtime/src/ext/mod.rs` (register extension)

---

### 1.3 Implement crypto.subtle.verify() for Signatures

**What:** Ed25519 and P256 signature verification (no signing yet)

**Technical Context:**
- Expose existing `jstz_crypto` verification functions
- `jstz_crypto/src/public_key.rs:95-179` already implements verify
- Only needs JS bindings

**Implementation:**
```rust
// In crates/jstz_runtime/src/ext/jstz_crypto/mod.rs
#[op2(async)]
#[serde]
async fn op_crypto_subtle_verify(
    #[string] algorithm: String,
    #[buffer] public_key: &[u8],
    #[buffer] signature: &[u8],
    #[buffer] data: &[u8],
) -> Result<bool, AnyError> {
    match algorithm.as_str() {
        "Ed25519" => {
            let pk = Ed25519::try_from(public_key)?;
            let sig = Ed25519Signature::try_from(signature)?;
            Ok(pk.verify(&sig, data).is_ok())
        }
        "ECDSA-P256" => {
            let pk = P256::try_from(public_key)?;
            let sig = P256Signature::try_from(signature)?;
            Ok(pk.verify(&sig, data).is_ok())
        }
        _ => Err(anyhow!("Unsupported algorithm")),
    }
}
```

**Files to Modify:**
- `crates/jstz_runtime/src/ext/jstz_crypto/mod.rs` (add op)
- `crates/jstz_runtime/src/ext/jstz_crypto/crypto.js` (wrap in SubtleCrypto API)
- `crates/jstz_crypto/Cargo.toml` (may need to expose more public APIs)

---

### 1.4 Implement crypto.subtle.importKey()

**What:** Import public keys (raw and JWK formats)

**Technical Context:**
- Parse raw bytes into `PublicKey` types
- Parse JWK JSON into keys
- No randomness needed ✓

**Implementation:**
```rust
#[op2(async)]
#[serde]
async fn op_crypto_subtle_import_key(
    #[string] format: String,
    #[serde] key_data: serde_json::Value,
    #[string] algorithm: String,
    extractable: bool,
    #[serde] usages: Vec<String>,
) -> Result<CryptoKeyHandle, AnyError> {
    // Parse and validate key data
    // Return opaque handle to key stored in OpState
}
```

**Files to Create/Modify:**
- Add `CryptoKeyHandle` type (opaque key reference)
- Store keys in `OpState` with handles
- Implement format parsers (raw, JWK)

---

### 1.5 Handle crypto.getRandomValues() Deterministically

**What:** Seeded PRNG instead of true randomness

**Technical Context:**
- Current: `getrandom` always fails (`jstz_core/src/runtime.rs:59-66`)
- Need: Deterministic PRNG seeded from transaction context

**Implementation Options:**

**Option A: Transaction-Seeded PRNG**
```rust
// In crates/jstz_runtime/src/ext/jstz_crypto/mod.rs
#[op2]
fn op_crypto_get_random_values(
    op_state: &mut OpState,
    #[buffer] buf: &mut [u8],
) -> Result<(), AnyError> {
    if buf.len() > 65536 {
        return Err(anyhow!("Buffer too large"));
    }

    // Get deterministic seed from transaction context
    let RuntimeContext { tx, .. } = op_state.borrow();
    let seed = tx.hash(); // or tx.hash() + block.hash()

    // Use ChaCha20 or similar PRNG
    let mut rng = ChaCha20Rng::from_seed(seed);
    rng.fill_bytes(buf);
    Ok(())
}
```

**Option B: Always Fail (Strict Determinism)**
- Keep current behavior (always fails)
- Document clearly in API docs
- Only support operations that don't need randomness

**Recommendation:** Start with Option B, add Option A later if needed

**Files to Modify:**
- `crates/jstz_core/src/runtime.rs:59-66` (if changing behavior)
- OR: `crates/jstz_runtime/src/ext/jstz_crypto/mod.rs` (override in extension)

---

## Phase 2: Key Management & Advanced Operations

### 2.1 Implement HMAC (sign/verify)

**What:** HMAC-SHA256/384/512 for message authentication

**Technical Context:**
- Use `cryptoxide` or add `hmac` crate
- User provides key material (no generation)
- Deterministic ✓

**Implementation:**
```rust
#[op2(async)]
#[buffer]
async fn op_crypto_subtle_sign_hmac(
    #[buffer] key: &[u8],
    #[string] hash: String,
    #[buffer] data: &[u8],
) -> Result<Vec<u8>, AnyError> {
    // HMAC implementation
}
```

**New Dependency:**
- Add `hmac = "0.12"` to `jstz_crypto/Cargo.toml`

---

### 2.2 Implement AES-GCM Encryption/Decryption

**What:** Authenticated encryption

**Technical Context:**
- Add AES-GCM implementation (e.g., `aes-gcm` crate)
- User provides key and IV
- Deterministic with provided inputs ✓

**Implementation:**
```rust
#[op2(async)]
#[buffer]
async fn op_crypto_subtle_encrypt(
    #[string] algorithm: String,
    #[buffer] key: &[u8],
    #[serde] params: EncryptParams,
    #[buffer] data: &[u8],
) -> Result<Vec<u8>, AnyError> {
    match algorithm.as_str() {
        "AES-GCM" => {
            // AES-GCM encryption
        }
        _ => Err(anyhow!("Unsupported")),
    }
}
```

**New Dependencies:**
- Add `aes-gcm = "0.10"` to `jstz_crypto/Cargo.toml`

---

### 2.3 Implement crypto.subtle.exportKey()

**What:** Export keys in raw and JWK formats

**Technical Context:**
- Serialize CryptoKey handles to portable formats
- Only for extractable keys

**Implementation:**
```rust
#[op2(async)]
#[serde]
async fn op_crypto_subtle_export_key(
    #[string] format: String,
    key_handle: u32,
    op_state: &mut OpState,
) -> Result<serde_json::Value, AnyError> {
    // Retrieve key from OpState
    // Serialize based on format
}
```

---

### 2.4 Implement PBKDF2 Key Derivation

**What:** Password-based key derivation

**Technical Context:**
- Leverage existing BIP39/PBKDF2 in `jstz_crypto`
- Deterministic ✓

**Implementation:**
```rust
#[op2(async)]
async fn op_crypto_subtle_derive_bits_pbkdf2(
    #[buffer] password: &[u8],
    #[buffer] salt: &[u8],
    iterations: u32,
    #[string] hash: String,
    length: u32,
) -> Result<Vec<u8>, AnyError> {
    // PBKDF2 derivation
}
```

---

## Phase 3: Advanced Features (Optional)

### 3.1 Ed25519/P256 Signing (crypto.subtle.sign())

**What:** Generate signatures (not just verify)

**Technical Context:**
- Expose `jstz_crypto/src/secret_key.rs` sign functions
- **Challenge:** Smart functions shouldn't have access to private keys directly
- **Solution:** Key must be imported/derived, not generated

**Implementation:**
```rust
#[op2(async)]
#[buffer]
async fn op_crypto_subtle_sign(
    #[string] algorithm: String,
    key_handle: u32,
    #[buffer] data: &[u8],
    op_state: &mut OpState,
) -> Result<Vec<u8>, AnyError> {
    // Retrieve private key from OpState
    // Sign data
}
```

**Security Consideration:** Need clear documentation on key management

---

### 3.2 ECDH Key Agreement (X25519, P-256)

**What:** Derive shared secrets

**Technical Context:**
- Add `x25519-dalek` or similar
- Deterministic with provided keys ✓

**New Dependencies:**
- `x25519-dalek = "2.0"`
- `p256` already available in `jstz_crypto`

---

### 3.3 Additional Algorithms

Lower priority algorithms to add if needed:

- **AES-CBC/CTR**: Similar to AES-GCM implementation
- **RSA-OAEP**: Add `rsa` crate, larger implementation
- **RSA-PSS**: Add `rsa` crate
- **HKDF**: Add `hkdf` crate

---

## Implementation Order (Recommended)

### Sprint 1: Foundation (2-3 weeks)
1. ✅ Integrate `deno_crypto` extension (or create custom `jstz_crypto` extension)
2. ✅ Implement `crypto.subtle.digest()` (SHA-256, SHA-384, SHA-512, Blake2b)
3. ✅ Expose `crypto` global object
4. ✅ Update WPT tests configuration

### Sprint 2: Signature Verification (1-2 weeks)
5. ✅ Implement `crypto.subtle.verify()` (Ed25519, P256)
6. ✅ Implement `crypto.subtle.importKey()` (raw format, public keys)
7. ✅ Add CryptoKey type and handle management

### Sprint 3: Encryption & Authentication (2-3 weeks)
8. ✅ Implement HMAC sign/verify
9. ✅ Implement AES-GCM encrypt/decrypt
10. ✅ Implement `crypto.subtle.exportKey()` (raw format)

### Sprint 4: Key Derivation (1-2 weeks)
11. ✅ Implement PBKDF2 derivation
12. ✅ Implement JWK import/export format

### Sprint 5: Advanced Features (2-3 weeks)
13. ✅ Implement `crypto.subtle.sign()` for Ed25519/P256
14. ✅ Implement ECDH/X25519
15. ✅ Handle `crypto.getRandomValues()` with seeded PRNG (if desired)

### Sprint 6: Polish & Testing (1-2 weeks)
16. ✅ Full WPT test suite validation
17. ✅ Documentation and examples
18. ✅ Performance optimization

**Total Estimated Time:** 9-15 weeks for full TIER 1 + partial TIER 2 implementation

---

## Key Technical Decisions Needed

### Decision 1: Extension Approach

**Option A: Use deno_crypto**
- ✅ Pros: Battle-tested, full spec compliance
- ❌ Cons: May have getrandom dependencies, larger bundle

**Option B: Custom jstz_crypto extension**
- ✅ Pros: Full control, optimized for determinism
- ❌ Cons: More implementation work, maintenance burden

**Recommendation:** Try Option A first, fall back to Option B if conflicts arise

---

### Decision 2: Randomness Strategy

**Option A: Strict Determinism (No randomness)**
- Implement only deterministic operations
- `getRandomValues()` remains disabled
- `generateKey()` not supported

**Option B: Seeded PRNG**
- Transaction/block hash as seed
- `getRandomValues()` returns deterministic values
- `generateKey()` supported with deterministic generation

**Option C: User-Provided Entropy**
- Custom API requiring entropy parameter
- Non-standard but maintains determinism

**Recommendation:** Start with Option A, document limitations clearly

---

### Decision 3: Private Key Handling

**Question:** Should smart functions have access to private keys for signing?

**Concern:** Private keys in smart function code = major security risk

**Options:**
1. **No signing from smart functions** - Only verification (recommended)
2. **Ephemeral keys only** - Generate/use/discard in single execution
3. **Imported keys with clear warnings** - Allow but document security implications

**Recommendation:** Option 1 initially, consider Option 2 for advanced use cases

---

## Files to Create

```
crates/jstz_runtime/src/ext/jstz_crypto/
├── mod.rs                 # Extension definition and ops
├── crypto.js              # JavaScript bindings for crypto object
└── subtle.js              # SubtleCrypto implementation

docs/
├── crypto-api.md          # User-facing crypto API documentation
└── crypto-security.md     # Security considerations and best practices
```

## Files to Modify

```
crates/jstz_runtime/
├── Cargo.toml             # Add deno_crypto or crypto dependencies
├── src/runtime.rs         # Register crypto extension
└── src/ext/
    ├── mod.rs             # Register jstz_crypto extension
    └── jstz_main/
        └── 98_global_scope.js  # Expose crypto global

crates/jstz_crypto/
├── Cargo.toml             # Add HMAC, AES, etc. dependencies
└── src/
    ├── lib.rs             # Expose new crypto ops
    └── (new modules for AES, HMAC, etc.)

crates/jstz_api/tests/
└── wpt.rs                 # Update test expectations
```

---

## Testing Strategy

### Unit Tests
- Test each crypto op in isolation
- Test error conditions (invalid keys, bad parameters)
- Test determinism (same inputs = same outputs)

### Integration Tests
- End-to-end crypto workflows in smart functions
- Key import → sign/verify → export
- Multiple crypto operations in single function

### WPT Tests
- Run Web Platform Test suite
- Track passing percentage as metric
- Skip tests requiring true randomness (document why)

### Performance Tests
- Benchmark crypto operations
- Ensure native performance (not slow JS polyfills)

---

## Documentation Requirements

### For Developers (Smart Function Authors)

1. **Quick Start Guide**
   - How to use crypto.subtle in jstz
   - Example: verify signature
   - Example: hash data

2. **API Reference**
   - Supported algorithms matrix
   - jstz-specific limitations
   - Determinism implications

3. **Security Best Practices**
   - Never hardcode private keys
   - Key management patterns
   - When to use which algorithm

### For Contributors (jstz Developers)

1. **Architecture Document**
   - How crypto extension works
   - Op design patterns
   - Adding new algorithms

2. **Testing Guide**
   - How to run crypto tests
   - WPT integration
   - Adding new test cases

---

## Success Metrics

### MVP (Phase 1 Complete)
- [ ] `crypto.subtle.digest()` working for SHA-256, SHA-512, Blake2b
- [ ] `crypto.subtle.verify()` working for Ed25519, P256
- [ ] `crypto.subtle.importKey()` working for raw public keys
- [ ] WPT pass rate: 20-30% (deterministic tests)
- [ ] Zero regression in deterministic execution

### Full TIER 1 (Phase 1-4 Complete)
- [ ] All TIER 1 operations implemented
- [ ] HMAC, AES-GCM, PBKDF2 working
- [ ] JWK import/export working
- [ ] WPT pass rate: 40-50%
- [ ] Documentation complete

### TIER 2 Features (Phase 5+)
- [ ] 50%+ TIER 2 operations
- [ ] ECDH, additional algorithms
- [ ] WPT pass rate: 60%+
- [ ] Performance benchmarks published

---

## References

- **Web Crypto Spec:** https://w3c.github.io/webcrypto/
- **Deno Crypto Source:** https://github.com/denoland/deno/tree/main/ext/crypto
- **jstz Crypto Crate:** `crates/jstz_crypto/`
- **jstz Runtime:** `crates/jstz_runtime/`
- **WPT Tests:** https://github.com/web-platform-tests/wpt/tree/master/WebCryptoAPI

---

## Next Steps

1. **Review this roadmap** with jstz team
2. **Make key decisions** (extension approach, randomness strategy)
3. **Create tracking issues** for each implementation phase
4. **Start Sprint 1** with deno_crypto integration
5. **Set up CI** for crypto tests

---

**Document Version:** 1.0
**Date:** 2025-11-18
**Status:** Proposal - Awaiting Team Review
