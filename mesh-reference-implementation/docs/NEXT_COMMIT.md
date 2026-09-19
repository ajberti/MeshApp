# Next Commit: prove iOS Bundle transfer

The iOS debug shell (`ios/MeshApp`) loads UniFFI `MeshEngine`, stores identity seeds in Keychain, and uses Network.framework Bonjour `_mesh._tcp` plus length-prefixed TCP as a replaceable `MeshTransport`. There is no messenger UI. Google Nearby Connections waits until Android exists.

1. Keep the in-process Mesh Session and `mesh-bindings` tests green.
2. Open `ios/MeshApp.xcodeproj`, set a Development Team, run two simulators or two iPhones.
3. Confirm a `send_text` Bundle transfers over the Network.framework adapter.
4. Android/Nearby and Compose/SwiftUI messaging wait until that works.
