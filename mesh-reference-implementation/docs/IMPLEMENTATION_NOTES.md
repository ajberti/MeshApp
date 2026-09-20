# Implementation Notes

## Milestone 1

The first working milestone established the state-machine and persistence boundaries that cryptography now plugs into.

## Milestone 2

Identity and recipient-box cryptography are implemented. Canonical CBOR uses numeric keys from Protocol §79. The Ed25519 signature covers the canonical map of fields 1–11 (immutable header plus encrypted payload) and does not cover hop count.

Direct-message key derivation:

```text
HKDF-SHA256
  IKM  = X25519(ephemeral_private, recipient_encryption_public)
  salt = "MeshProtocol-1.0"
  info = "direct-message-key-v1"
AEAD   = ChaCha20-Poly1305 (12-byte nonce, 16-byte tag appended to ciphertext)
```

Protocol version in a Bundle is the unsigned integer `(major << 8) | minor`, so 1.0 encodes as `0x0100`.

Local conversation copies are encrypted at rest with XChaCha20-Poly1305 and a 256-bit `LocalDataKey` held in memory. Native Keychain/Keystore persistence comes with the app shells.

## Milestone 3

`MeshCore::send_text()` creates a signed, encrypted Bundle with no radio. The destination core decrypts only after `add_contact()` has stored the sender's public keys. Relays store ciphertext and increment hop count.

Clock and RNG are injected (`MeshClock`, `MeshRng`) so tests can be deterministic.

## Milestone 4

A Mesh Session is established on `LinkOpened`. Both sides send `HELLO`; the node with the smaller Discovery ID sends `KEY_INIT`. Session keys are HKDF-SHA256 of the X25519 shared secret with the ordered HELLO bytes and ephemeral public keys as context. Subsequent frames set flag `0x01` and use ChaCha20-Poly1305 with nonce `0x00000000 || counter_be`. After `SESSION_OK` each side sends an explicit Bundle ID inventory (no Bloom filters), then `BUNDLE_OFFER` / `REQUEST` / `DATA` / `COMPLETE`.

```text
HKDF-SHA256
  IKM  = X25519(local_ephemeral, remote_ephemeral)
  salt = "MeshProtocol-1.0"
  info = "session-key-lo-hi-v1" || transcript
       | "session-key-hi-lo-v1" || transcript
AEAD   = ChaCha20-Poly1305
  nonce = 4 zero bytes || 8-byte big-endian counter
  AAD   = major || minor || frame_type || flags
```

## Milestone 5

`mesh-bindings` exposes a UniFFI `MeshEngine`. Native calls `generate_identity`, stores the seeds in Keystore/Keychain, then `open` / `process_event` / `send_text` / `add_contact` / `get_conversations` / `get_messages`. Identifiers cross FFI as byte arrays. SQLite, wire frames and crypto stay inside Rust.

## Milestone 6

iOS comes first (no Android hardware in this workflow). `ios/MeshApp` is a debug shell: Keychain identity, UniFFI `MeshEngine`, and a Network.framework Bonjour/TCP transport with length-prefixed frames. Two simulators have completed a `send_text` Bundle transfer on that path. Nearby Connections stays the later cross-platform radio. SwiftUI here is diagnostic only; Bonjour TXT still carries public keys for debug `add_contact`.

## Critical invariants

1. Bundle IDs and message IDs are distinct types.
2. A transport never decides which bundle to relay.
3. A successful radio write is not a delivery receipt.
4. Relay state is persisted before a bundle becomes eligible for forwarding.
5. Tombstoned bundles are never reinserted into the relay queue.
6. All foreign/native events enter the Rust core through one serialized event stream.
7. Relays must never obtain DIRECT_MESSAGE plaintext.
8. User IDs are SHA-256(Ed25519 public key)[0..15], never random in production.
