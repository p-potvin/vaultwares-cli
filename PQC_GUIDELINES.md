# VaultWares Post-Quantum Cryptography (PQC) Guidelines

This document outlines the strict requirements for implementing Post-Quantum Cryptography and Key Encapsulation Mechanisms (KEM) within the VaultWares ecosystem, specifically for the `vaultwares-cli`.

## 1. Primary Algorithm: ML-KEM (FIPS 203)

All new cryptographic handshakes and data-at-rest protection MUST use **ML-KEM** (Module-Lattice-Based Key-Encapsulation Mechanism), as standardized by NIST in FIPS 203.

- **Default Security Level**: `ML-KEM-768` (Equivalent to AES-192 security, providing a robust balance between performance and safety).
- **Implementation**: Use the `fips203` Rust crate for standardized compliance.

## 2. Key Encapsulation Mechanism (KEM) Workflow

KEMs MUST be used to establish shared secrets. Direct encryption with public keys is prohibited; always encapsulate a symmetric key.

1.  **Key Generation**: Generate an `EncapsulationKey` (EK) and a `DecapsulationKey` (DK).
2.  **Encapsulation**: The sender uses the EK to produce a `Ciphertext` (CT) and a `SharedSecret` (SS).
3.  **Decapsulation**: The receiver uses the DK and the CT to recover the identical `SharedSecret` (SS).
4.  **Symmetric Encryption**: Use the derived `SharedSecret` with **AES-256-GCM** or **ChaCha20-Poly1305** for actual data encryption.

## 3. Client-Side Only Architecture

VaultWares follows a **Client-Side Only Encryption** model for sensitive user data.

- **Non-Exportable Keys**: Decapsulation Keys (DK) MUST NEVER be transmitted to any server.
- **Persistence**: DKs must be encrypted at rest using a key derived from the user's PIN/Password (via Argon2id or Scrypt) or stored in the OS-native secure enclave (e.g., Windows TPM/DPAPI, macOS Keychain).
- **Session Security**: Every session file should ideally be wrapped with a unique ML-KEM-derived key.

## 4. Homomorphic Encryption (Future-Proofing)

While not currently mandatory for all operations, the architecture MUST support future integration of Fully Homomorphic Encryption (FHE) for private metrics processing.

- **Planned Schemes**: CKKS (for floating point/metrics) or BFV/BGV (for integer arithmetic).
- **Usage**: Calculating aggregate HUD metrics (e.g., total token cost across all users) without decrypting individual session data.

## 5. Implementation Checklist

- [x] Use `fips203::ml_kem_768` for all key pairs.
- [x] Ensure `DecapsulationKey` is never serialized in plaintext.
- [x] Use a cryptographically secure random number generator (CSPRNG) like `rand::rngs::OsRng`.
- [x] Verify that all PQC operations are performed on the client-side before any data reaches the orchestration layer (Redis).

---
*VaultWares Security Team - 2026*
