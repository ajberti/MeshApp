# iOS app

Loads `MeshEngine` over UniFFI and uses Apple's Network framework (Bonjour `_mesh._tcp` + peer-to-peer TCP) as the first radio. This is a replaceable `MeshTransport`, not part of Mesh Protocol. Google Nearby Connections is deferred until Android exists.

The main UI is a conversation list. Add a contact with a `mesh:1` QR or paste (both people must add each other). Bonjour peers, engine logs and the raw contact card are under **Advanced**.

## Open

```bash
open ios/MeshApp.xcodeproj
```

In Xcode, select a Development Team, then run on two simulators or two iPhones. Allow Local Network when prompted. Use **+** to show your QR / copy a card, add the other person, then send in the thread.

The Run Script phase builds `libmesh_bindings.a` and regenerates Swift in `MeshApp/Generated`. Xcode's script PATH does not include Cargo, so `ios/scripts/build-rust.sh` sources `~/.cargo/env`.

Needs: Rust stable, `rustup` targets `aarch64-apple-ios` and `aarch64-apple-ios-sim`.

## Manual generate

```bash
ios/scripts/build-rust.sh
```
