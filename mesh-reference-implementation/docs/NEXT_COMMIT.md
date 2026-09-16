# Next Commit: UniFFI, then Nearby on Android

The in-process Mesh Session path is done. Native code still cannot call `MeshCore`, and there is no Android or iOS project. Radios stay in native code; protocol stays in Rust. Nearby Connections is a replaceable `MeshTransport` and must not leak into routing or crypto.

1. Keep the in-process Mesh Session tests green.
2. Give `mesh-bindings` a real UniFFI API: `open`, `process_event`, `send_text`, `add_contact`, and read conversations/messages. IDs stay bytes. Do not expose SQLite or crypto internals.
3. Add a thin Nearby adapter on Android first. Map radio callbacks to `CoreEvent` (`PeerDiscovered`, `LinkOpened`, `BytesReceived`, `LinkClosed`) and execute `CoreAction` (`Connect`, `SendBytes`, `CloseLink`).
4. Prove a device-to-device Bundle transfer over Nearby (same path as the in-process session test). Then add iOS.
5. Do not start Jetpack Compose or SwiftUI messaging until two devices complete `HELLO` → session keys → inventory → `send_text` delivery.
