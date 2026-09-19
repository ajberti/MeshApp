# Next Commit: Nearby on Android

UniFFI `MeshEngine` is in place. Native code still has no Android or iOS project. Radios stay in native code; protocol stays in Rust. Nearby Connections is a replaceable `MeshTransport` and must not leak into routing or crypto.

1. Keep the in-process Mesh Session and `mesh-bindings` tests green.
2. Add a thin Nearby adapter on Android first. Map radio callbacks to `MeshEvent` (`PeerDiscovered`, `LinkOpened`, `BytesReceived`, `LinkClosed`) and execute `MeshAction` (`Connect`, `SendBytes`, `CloseLink`).
3. Persist `generate_identity()` seeds in Android Keystore and pass them to `MeshEngine::open`.
4. Prove a device-to-device Bundle transfer over Nearby (same path as the in-process session test). Then add iOS.
5. Do not start Jetpack Compose or SwiftUI messaging until two devices complete `HELLO` → session keys → inventory → `send_text` delivery.
