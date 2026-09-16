# Implementation Notes

## Milestone 1

The first working milestone established the state-machine and persistence boundaries that cryptography now plugs into.

## Milestone 2

Identity and recipient-box cryptography are implemented. Canonical CBOR uses numeric keys from Protocol §79. The Ed25519 signature covers the canonical map of fields 1–11 (immutable header plus encrypted payload) and does not cover hop count.

Direct-message key derivation:

```text
HKDF-SHA256
  IKM  = X25519(ephemeral_private, recipient_encryption_public)
  salt = "MeshProtocol-1.0"
  info = "direct-message-key-v1"
AEAD   = ChaCha20-Poly1305 (12-byte nonce, 16-byte tag appended to ciphertext)
```

Protocol version in a Bundle is the unsigned integer `(major << 8) | minor`, so 1.0 encodes as `0x0100`.

## Critical invariants

1. Bundle IDs and message IDs are distinct types.
2. A transport never decides which bundle to relay.
3. A successful radio write is not a delivery receipt.
4. Relay state is persisted before a bundle becomes eligible for forwarding.
5. Tombstoned bundles are never reinserted into the relay queue.
6. All foreign/native events enter the Rust core through one serialized event stream.
7. Relays must never obtain DIRECT_MESSAGE plaintext.
8. User IDs are SHA-256(Ed25519 public key)[0..15], never random in production.
