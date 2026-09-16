# Next Commit: secure direct A -> B messaging

1. Add Ed25519 + X25519 identity material in `mesh-crypto`.
2. Add X25519/HKDF/ChaCha20-Poly1305 recipient encryption.
3. Add signed immutable bundle encoding.
4. Add a DirectMessage payload structure.
5. Add `MeshCore::send_text()`.
6. Add recipient delivery/decryption path.
7. Add deterministic crypto test vectors.
8. Only after those tests pass, add HELLO/session frames and Nearby Connections adapters.
