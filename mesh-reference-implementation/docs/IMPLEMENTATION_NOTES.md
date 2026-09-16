# Implementation Notes

## Milestone 1

The first working milestone deliberately stops before cryptography and physical radio transports. It establishes the state-machine and persistence boundaries that those layers will plug into.

## Critical invariants

1. Bundle IDs and message IDs are distinct types.
2. A transport never decides which bundle to relay.
3. A successful radio write is not a delivery receipt.
4. Relay state is persisted before a bundle becomes eligible for forwarding.
5. Tombstoned bundles are never reinserted into the relay queue.
6. All foreign/native events enter the Rust core through one serialized event stream.
