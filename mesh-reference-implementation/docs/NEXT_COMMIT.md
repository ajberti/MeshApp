# Next Commit: drop debug TXT, add explicit contacts

Two iOS simulators have transferred a `send_text` Bundle over Network.framework (`HELLO` → session keys → inventory → plaintext on the other device). The debug shell and Bonjour/TCP adapter stay; this is still not messenger UI.

Bonjour TXT currently advertises signing/encryption public keys so `add_contact` works without a QR flow. That is debug-only and must not remain the contact model.

1. Keep the in-process Mesh Session and `mesh-bindings` tests green.
2. Optional: confirm the same transfer on two physical iPhones on the same Wi-Fi.
3. Stop putting identity public keys in Bonjour TXT. Add an explicit contact exchange (QR or paste) that calls `add_contact`.
4. Android/Nearby still waits. A real conversation UI can start after contacts are not advertised in TXT.
