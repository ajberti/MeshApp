# Mesh Reference Implementation

Initial Rust workspace for Mesh Protocol 1.0.

## Current milestone

This skeleton implements:

- strongly typed 128-bit identifiers;
- protocol bundle metadata and priorities;
- CBOR bundle encoding/decoding;
- SQLite-backed bundle/tombstone storage;
- controlled-epidemic routing decisions;
- an event/action `MeshCore` shell;
- initial unit tests;
- a thin UniFFI-ready bindings crate.

Cryptographic message/session implementation and mobile transports intentionally come next.

## Build

Install stable Rust, then run:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Architecture

Native Android/iOS code owns radios, permissions, secure OS storage and UI. Rust owns protocol state, routing, bundle persistence and later cryptography.

```text
Native UI / transport
       |
       v
   mesh-bindings
       |
       v
    mesh-core
   /    |     \
wire  routing  store
  \      |      /
      mesh-types
```
