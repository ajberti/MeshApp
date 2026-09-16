# Protocol test vectors

Canonical Mesh Protocol 1.0 cryptographic vectors. Seeds increment from a start byte:

- Alice signing `00..1f`, encryption `20..3f`
- Bob signing `40..5f`, encryption `60..7f`
- Ephemeral X25519 `80..9f`
- ChaCha20-Poly1305 nonce `a0..ab`

Key derivation for direct messages:

```text
HKDF-SHA256
  salt = "MeshProtocol-1.0"
  info = "direct-message-key-v1"
```

Files:

- `identity.json` — Ed25519/X25519 public keys, derived User IDs and fingerprints
- `message-encryption.json` — recipient-box of `Are you safe?`
- `signature.json` — Ed25519 over `mesh-signed-bytes-v1`

Ciphertext includes the 16-byte Poly1305 tag. A third identity (Charlie) must fail to decrypt the message vector.
