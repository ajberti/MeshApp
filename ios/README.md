# iOS debug shell

Loads `MeshEngine` over UniFFI and uses Apple's Network framework (Bonjour `_mesh._tcp` + peer-to-peer TCP) as the first radio. This is a replaceable `MeshTransport`, not part of Mesh Protocol. Google Nearby Connections is deferred until Android exists.

This is a diagnostic shell, not the messenger UI. The debug advertisement includes public keys in the Bonjour TXT record so two devices can `add_contact` without a QR flow.

## Open

```bash
open ios/MeshApp.xcodeproj
```

In Xcode, select a Development Team, then run on two simulators or two iPhones. Allow Local Network when prompted. Each instance should list the other under Peers; **Send to first contact** should show the plaintext on the other device.

The Run Script phase builds `libmesh_bindings.a` and regenerates Swift in `MeshApp/Generated`. Xcode's script PATH does not include Cargo, so `ios/scripts/build-rust.sh` sources `~/.cargo/env`.

Needs: Rust stable, `rustup` targets `aarch64-apple-ios` and `aarch64-apple-ios-sim`.

## Manual generate

```bash
ios/scripts/build-rust.sh
```
