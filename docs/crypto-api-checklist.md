# Web Crypto API Technical Checklist

## Overview

This document provides a comprehensive, tiered checklist of the Web Crypto API features based on the W3C Web Cryptography API Level 2 specification (Editor's Draft, August 15, 2025).

## Feature Tiers

Features are categorized into three tiers based on their importance for a JavaScript platform:

- **TIER 1 (MUST HAVE)**: Essential features required for basic cryptographic operations
- **TIER 2 (SHOULD HAVE)**: Important features that significantly enhance utility
- **TIER 3 (OPTIONAL)**: Advanced or specialized features

---

## 1. Core Interfaces

### 1.1 Crypto Interface

| Feature | Tier | Description | Specification Reference |
|---------|------|-------------|------------------------|
| `crypto` global object | TIER 1 | Global access point for cryptographic operations | W3C WebCrypto §10 |
| `crypto.subtle` property | TIER 1 | Access to SubtleCrypto interface | W3C WebCrypto §10.1 |
| `crypto.getRandomValues()` | TIER 1 | Fill typed array with cryptographically secure random bytes (max 65,536 bytes) | W3C WebCrypto §10.2 |
| `crypto.randomUUID()` | TIER 2 | Generate RFC 4122 version 4 UUID as string | W3C WebCrypto §10.3 |

**Secure Context Requirements:**
- TIER 1: Available only in secure contexts (HTTPS)
- TIER 3: Support in Web Workers

**Notes on Deterministic Environments:**
- `getRandomValues()` requires special consideration in deterministic execution environments
- May need to be implemented with seeded PRNG rather than true CSPRNG

### 1.2 SubtleCrypto Interface

| Feature | Tier | Description | Return Type |
|---------|------|-------------|-------------|
| SubtleCrypto interface | TIER 1 | Low-level cryptographic primitives | Interface |
| All methods return Promises | TIER 1 | Asynchronous API pattern | Promise-based |

---

## 2. Cryptographic Operations

### 2.1 Hashing (Digest)

| Operation | Tier | Algorithm | Key Sizes/Parameters | Use Cases |
|-----------|------|-----------|---------------------|-----------|
| `digest()` | TIER 1 | SHA-256 | N/A | General-purpose hashing, content integrity |
| `digest()` | TIER 1 | SHA-384 | N/A | Enhanced security hashing |
| `digest()` | TIER 1 | SHA-512 | N/A | High-security hashing |
| `digest()` | TIER 2 | SHA-1 | N/A | Legacy compatibility (deprecated for security) |

**Method Signature:**
```javascript
crypto.subtle.digest(algorithm, data) → Promise<ArrayBuffer>
```

**Priority Rationale:**
- SHA-256 is the industry standard and MUST be supported
- SHA-384/512 are essential for applications requiring higher security
- SHA-1 is included for backward compatibility but should not be recommended

### 2.2 Encryption/Decryption

#### 2.2.1 Symmetric Encryption

| Operation | Tier | Algorithm | Key Sizes | Parameters | Use Cases |
|-----------|------|-----------|-----------|------------|-----------|
| `encrypt()` / `decrypt()` | TIER 1 | AES-GCM | 128, 192, 256 bits | iv, additionalData, tagLength | Authenticated encryption (recommended) |
| `encrypt()` / `decrypt()` | TIER 2 | AES-CBC | 128, 192, 256 bits | iv | Legacy encryption mode |
| `encrypt()` / `decrypt()` | TIER 2 | AES-CTR | 128, 192, 256 bits | counter, length | Counter mode encryption |

**Method Signatures:**
```javascript
crypto.subtle.encrypt(algorithm, key, data) → Promise<ArrayBuffer>
crypto.subtle.decrypt(algorithm, key, data) → Promise<ArrayBuffer>
```

**Priority Rationale:**
- AES-GCM is TIER 1 as it provides authenticated encryption (confidentiality + integrity)
- AES-CBC and AES-CTR are TIER 2 for compatibility with existing systems

#### 2.2.2 Asymmetric Encryption

| Operation | Tier | Algorithm | Key Sizes | Parameters | Use Cases |
|-----------|------|-----------|-----------|------------|-----------|
| `encrypt()` / `decrypt()` | TIER 2 | RSA-OAEP | 2048, 3072, 4096 bits | hash (SHA-256, SHA-384, SHA-512), label | Asymmetric encryption |

**Priority Rationale:**
- RSA-OAEP is TIER 2 as asymmetric encryption is less common in smart contract contexts
- Larger key sizes (3072, 4096) provide future-proof security

### 2.3 Digital Signatures

#### 2.3.1 Signing and Verification

| Operation | Tier | Algorithm | Key Sizes/Curves | Parameters | Use Cases |
|-----------|------|-----------|------------------|------------|-----------|
| `sign()` / `verify()` | TIER 1 | Ed25519 | 256-bit | None | Modern signature scheme (recommended) |
| `sign()` / `verify()` | TIER 1 | ECDSA | P-256 | hash (SHA-256, SHA-384, SHA-512) | NIST standard curves |
| `sign()` / `verify()` | TIER 2 | ECDSA | P-384, P-521 | hash | Enhanced security curves |
| `sign()` / `verify()` | TIER 1 | HMAC | Variable | hash (SHA-256, SHA-384, SHA-512) | Message authentication |
| `sign()` / `verify()` | TIER 2 | RSA-PSS | 2048, 3072, 4096 bits | hash, saltLength | Modern RSA signing |
| `sign()` / `verify()` | TIER 2 | RSASSA-PKCS1-v1_5 | 2048, 3072, 4096 bits | hash | Legacy RSA signing |

**Method Signatures:**
```javascript
crypto.subtle.sign(algorithm, key, data) → Promise<ArrayBuffer>
crypto.subtle.verify(algorithm, key, signature, data) → Promise<boolean>
```

**Priority Rationale:**
- Ed25519 is TIER 1 as the modern standard for signatures (performance + security)
- ECDSA P-256 is TIER 1 for broad compatibility
- HMAC is TIER 1 for message authentication codes
- RSA algorithms are TIER 2 for legacy system compatibility

### 2.4 Key Derivation

| Operation | Tier | Algorithm | Parameters | Use Cases |
|-----------|------|-----------|------------|-----------|
| `deriveBits()` / `deriveKey()` | TIER 1 | PBKDF2 | hash, salt, iterations | Password-based key derivation |
| `deriveBits()` / `deriveKey()` | TIER 2 | HKDF | hash, salt, info | HMAC-based key derivation |
| `deriveBits()` / `deriveKey()` | TIER 2 | ECDH | public key (P-256, P-384, P-521) | Elliptic Curve Diffie-Hellman |
| `deriveBits()` / `deriveKey()` | TIER 2 | X25519 | public key | Modern ECDH with Curve25519 |

**Method Signatures:**
```javascript
crypto.subtle.deriveKey(algorithm, baseKey, derivedKeyType, extractable, keyUsages) → Promise<CryptoKey>
crypto.subtle.deriveBits(algorithm, baseKey, length?) → Promise<ArrayBuffer>
```

**Priority Rationale:**
- PBKDF2 is TIER 1 as the standard for password-based key derivation
- HKDF, ECDH, X25519 are TIER 2 for advanced key management scenarios

### 2.5 Key Management

#### 2.5.1 Key Generation

| Operation | Tier | Description | Algorithms |
|-----------|------|-------------|------------|
| `generateKey()` | TIER 1 | Generate symmetric keys | AES-GCM (128, 192, 256 bits) |
| `generateKey()` | TIER 1 | Generate HMAC keys | HMAC (variable length) |
| `generateKey()` | TIER 2 | Generate asymmetric key pairs | Ed25519, ECDSA (P-256, P-384, P-521), RSA-PSS, RSASSA-PKCS1-v1_5, RSA-OAEP |
| `generateKey()` | TIER 2 | Generate key agreement pairs | ECDH (P-256, P-384, P-521), X25519 |

**Method Signature:**
```javascript
crypto.subtle.generateKey(algorithm, extractable, keyUsages) → Promise<CryptoKey | CryptoKeyPair>
```

**Priority Rationale:**
- Symmetric key generation (AES, HMAC) is TIER 1 for common encryption/authentication needs
- Asymmetric key generation is TIER 2 (less common in smart contracts, requires randomness)

#### 2.5.2 Key Import/Export

| Operation | Tier | Formats | Description |
|-----------|------|---------|-------------|
| `importKey()` | TIER 1 | raw | Unformatted byte sequence (symmetric keys) |
| `importKey()` | TIER 1 | jwk | JSON Web Key format (all key types) |
| `importKey()` | TIER 2 | spki | SubjectPublicKeyInfo (public keys) |
| `importKey()` | TIER 2 | pkcs8 | PrivateKeyInfo (private keys) |
| `exportKey()` | TIER 1 | raw | Export symmetric keys as bytes |
| `exportKey()` | TIER 1 | jwk | Export in JSON Web Key format |
| `exportKey()` | TIER 2 | spki | Export public keys in SPKI format |
| `exportKey()` | TIER 2 | pkcs8 | Export private keys in PKCS#8 format |

**Method Signatures:**
```javascript
crypto.subtle.importKey(format, keyData, algorithm, extractable, keyUsages) → Promise<CryptoKey>
crypto.subtle.exportKey(format, key) → Promise<ArrayBuffer | JsonWebKey>
```

**Priority Rationale:**
- Raw and JWK formats are TIER 1 (most common, simplest to implement)
- SPKI and PKCS#8 are TIER 2 (interoperability with other systems)

#### 2.5.3 Key Wrapping

| Operation | Tier | Algorithm | Key Sizes | Use Cases |
|-----------|------|-----------|-----------|-----------|
| `wrapKey()` / `unwrapKey()` | TIER 2 | AES-KW | 128, 192, 256 bits | Secure key storage/transport |
| `wrapKey()` / `unwrapKey()` | TIER 3 | AES-GCM | 128, 192, 256 bits | Alternative key wrapping |
| `wrapKey()` / `unwrapKey()` | TIER 3 | RSA-OAEP | 2048+ bits | Asymmetric key wrapping |

**Method Signatures:**
```javascript
crypto.subtle.wrapKey(format, key, wrappingKey, wrapAlgorithm) → Promise<ArrayBuffer>
crypto.subtle.unwrapKey(format, wrappedKey, unwrappingKey, unwrapAlgorithm, unwrappedKeyAlgorithm, extractable, keyUsages) → Promise<CryptoKey>
```

**Priority Rationale:**
- Key wrapping is TIER 2/3 (specialized use case, not essential for basic crypto)
- AES-KW is the standard algorithm but less commonly needed

---

## 3. Supporting Types and Structures

### 3.1 CryptoKey Interface

| Feature | Tier | Property | Description |
|---------|------|----------|-------------|
| CryptoKey type | TIER 1 | `type` | Key type: "public", "private", or "secret" |
| CryptoKey extractable | TIER 1 | `extractable` | Whether key can be exported |
| CryptoKey algorithm | TIER 1 | `algorithm` | Object reflecting key algorithm |
| CryptoKey usages | TIER 1 | `usages` | Array of permitted operations |

**Key Usages:**
- TIER 1: `encrypt`, `decrypt`, `sign`, `verify`, `deriveKey`, `deriveBits`
- TIER 2: `wrapKey`, `unwrapKey`

### 3.2 CryptoKeyPair Dictionary

| Feature | Tier | Description |
|---------|------|-------------|
| CryptoKeyPair | TIER 2 | Contains `publicKey` and `privateKey` properties |

### 3.3 Algorithm Parameters

All algorithm-specific parameter dictionaries are required when implementing the corresponding algorithms. Priority follows the algorithm tier.

---

## 4. Algorithm Support Matrix

### 4.1 Algorithm-Operation Compatibility

| Algorithm | generateKey | sign/verify | encrypt/decrypt | deriveBits/deriveKey | digest | Tier |
|-----------|-------------|-------------|-----------------|---------------------|--------|------|
| **SHA-256** | ❌ | ❌ | ❌ | ❌ | ✅ | 1 |
| **SHA-384** | ❌ | ❌ | ❌ | ❌ | ✅ | 1 |
| **SHA-512** | ❌ | ❌ | ❌ | ❌ | ✅ | 1 |
| SHA-1 | ❌ | ❌ | ❌ | ❌ | ✅ | 2 |
| **AES-GCM** | ✅ | ❌ | ✅ | ❌ | ❌ | 1 |
| AES-CBC | ✅ | ❌ | ✅ | ❌ | ❌ | 2 |
| AES-CTR | ✅ | ❌ | ✅ | ❌ | ❌ | 2 |
| AES-KW | ✅ | ❌ | ❌ | ❌ | ❌ | 2 |
| **HMAC** | ✅ | ✅ | ❌ | ❌ | ❌ | 1 |
| **PBKDF2** | ❌ | ❌ | ❌ | ✅ | ❌ | 1 |
| HKDF | ❌ | ❌ | ❌ | ✅ | ❌ | 2 |
| **Ed25519** | ✅ | ✅ | ❌ | ❌ | ❌ | 1 |
| **ECDSA (P-256)** | ✅ | ✅ | ❌ | ❌ | ❌ | 1 |
| ECDSA (P-384, P-521) | ✅ | ✅ | ❌ | ❌ | ❌ | 2 |
| ECDH | ✅ | ❌ | ❌ | ✅ | ❌ | 2 |
| X25519 | ✅ | ❌ | ❌ | ✅ | ❌ | 2 |
| RSA-OAEP | ✅ | ❌ | ✅ | ❌ | ❌ | 2 |
| RSA-PSS | ✅ | ✅ | ❌ | ❌ | ❌ | 2 |
| RSASSA-PKCS1-v1_5 | ✅ | ✅ | ❌ | ❌ | ❌ | 2 |

**Legend:**
- ✅ = Algorithm supports this operation
- ❌ = Algorithm does not support this operation
- **Bold** = TIER 1 (Must Have)
- Normal = TIER 2 (Should Have)
- *Italic* = TIER 3 (Optional)

---

## 5. Minimum Viable Implementation (TIER 1 Only)

A minimal Web Crypto API implementation for a JavaScript platform should include:

### Core Interfaces
- ✅ `crypto` global object
- ✅ `crypto.subtle` property
- ✅ `crypto.getRandomValues()` (with determinism considerations)
- ✅ `CryptoKey` interface

### Essential Operations
- ✅ `crypto.subtle.digest()` for SHA-256, SHA-384, SHA-512
- ✅ `crypto.subtle.encrypt()` / `decrypt()` for AES-GCM (128, 256 bits)
- ✅ `crypto.subtle.sign()` / `verify()` for:
  - Ed25519
  - ECDSA with P-256
  - HMAC with SHA-256
- ✅ `crypto.subtle.generateKey()` for AES-GCM and HMAC
- ✅ `crypto.subtle.importKey()` / `exportKey()` for raw and JWK formats
- ✅ `crypto.subtle.deriveKey()` / `deriveBits()` for PBKDF2

### Supporting Types
- ✅ CryptoKey with all properties (type, extractable, algorithm, usages)
- ✅ Algorithm parameter dictionaries for supported algorithms

**Estimated Implementation Scope:**
- **TIER 1**: ~60% of full Web Crypto API functionality
- **TIER 1 + TIER 2**: ~90% of practical use cases
- **All Tiers**: 100% specification compliance

---

## 6. Browser Compatibility Reference

Based on MDN documentation (as of 2025):
- Web Crypto API has been "well established and works across many devices and browser versions" since September 2017
- Secure context (HTTPS) required for full functionality
- Web Worker support available
- Ed25519/X25519 support is newer (Chrome 123+, Safari, Node.js 19+)

---

## 7. Implementation Considerations

### 7.1 Deterministic Execution Environments

For platforms requiring deterministic execution (like smart contracts, rollups):

**Challenges:**
1. `crypto.getRandomValues()` is non-deterministic by design
2. `crypto.randomUUID()` requires randomness
3. `generateKey()` requires CSPRNG for security

**Solutions:**
1. **Seeded PRNG**: Use transaction hash + block data as seed
2. **Explicit Documentation**: Clearly document deterministic behavior
3. **Limited Scope**: Implement only deterministic operations (digest, sign/verify with provided keys, derive from deterministic inputs)
4. **Oracle Integration**: External randomness via oracle (breaks determinism but enables full functionality)

### 7.2 Security Considerations

From W3C specification:
> "The Web Crypto API provides a number of low-level cryptographic primitives. It's very easy to misuse them, and the pitfalls involved can be very subtle."

**Recommendations:**
- Implement comprehensive input validation
- Follow algorithm-specific security guidelines
- Provide clear documentation on proper usage
- Consider implementing safe defaults (e.g., minimum key sizes, recommended algorithms)

### 7.3 Performance Considerations

- All SubtleCrypto methods are asynchronous (Promise-based)
- Operations should be implemented with native/Rust bindings for performance
- Consider streaming APIs for large data processing

---

## 8. Testing Requirements

The W3C maintains Web Platform Tests (WPT) for the Web Crypto API:
- Test suite available at: https://github.com/web-platform-tests/wpt/tree/master/WebCryptoAPI
- Comprehensive coverage of all algorithms and operations
- Should be used to validate implementation compliance

---

## References

1. W3C Web Cryptography API Level 2 (Editor's Draft): https://w3c.github.io/webcrypto/
2. W3C Web Cryptography API (Recommendation): https://www.w3.org/TR/webcrypto/
3. MDN Web Crypto API: https://developer.mozilla.org/en-US/docs/Web/API/Web_Crypto_API
4. MDN SubtleCrypto: https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto
5. WICG Secure Curves: https://wicg.github.io/webcrypto-secure-curves/
6. Node.js Web Crypto API: https://nodejs.org/api/webcrypto.html

---

## Version History

- **Version 1.0** (2025-11-18): Initial checklist based on W3C Web Cryptography API Level 2 specification
