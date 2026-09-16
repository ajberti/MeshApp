# Next Commit: MeshCore::send_text and local delivery

1. Add contacts storage (public signing and encryption keys, fingerprint, trust state).
2. Add `MeshCore` identity plus `add_contact()`.
3. Add `MeshCore::send_text(recipient, text)` using the signed/encrypted Bundle path.
4. On receive, if `destination_id` is local: decrypt, verify, store the message, emit `MessageReceived`.
5. Inject Clock and RandomSource traits for deterministic tests.
6. Keep HELLO/session frames and Nearby Connections until that in-process A → B test passes.
