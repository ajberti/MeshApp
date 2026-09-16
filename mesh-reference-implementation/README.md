# Mesh Reference Implementation

Rust workspace for Mesh Protocol 1.0.

## Current milestone

This workspace implements:

- strongly typed 128-bit identifiers;
- canonical CBOR Bundles with numeric keys;
- Ed25519 identity, X25519 recipient encryption, HKDF-SHA256 and ChaCha20-Poly1305;
- signed immutable headers (relay hop count is unsigned);
- DirectMessage payload encoding;
- deterministic crypto test vectors;
- SQLite-backed bundle/tombstone storage;
- controlled-epidemic routing decisions;
- an event/action `MeshCore` shell with identity, contacts and `send_text()`;
- a thin UniFFI-ready bindings crate.

`MeshCore::send_text()` composes, encrypts, signs and stores a Bundle with no radio. The destination decrypts locally after a contact is added. Session HELLO frames and mobile transports come next.

## Build

Install stable Rust, then run:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Architecture

Native Android/iOS code owns radios, permissions, secure OS storage and UI. Rust owns protocol state, routing, bundle persistence and cryptography.

```text
Native UI / transport
       |
       v
   mesh-bindings
       |
       v
    mesh-core
   /   |    \   \
wire routing store crypto
  \    |     /    /
      mesh-types
```
