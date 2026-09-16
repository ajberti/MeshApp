# Next Commit: Mesh session HELLO and encrypted frames

1. Add the `0x4D50` binary frame header and frame types.
2. Implement HELLO, version negotiation, ephemeral X25519 link keys and SESSION_OK.
3. Protect subsequent frames with ChaCha20-Poly1305 session keys and counters.
4. Exchange an explicit Bundle ID inventory (no Bloom filters).
5. Transfer a `send_text` Bundle across two in-process `MeshCore` sessions.
6. Keep Nearby Connections and native apps until that session test passes.
