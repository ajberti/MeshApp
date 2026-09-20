# Next Commit: three-device iOS relay

Bounded stranger relay is in `MeshCore`: destination offers are requested first; foreign copies are capped per encounter and against the 250 MB relay quota. Senders also cap offers/bytes per session.

1. Keep Mesh Session, stranger-cap, and `mesh-bindings` tests green.
2. Prove A → C → B on three simulators (C has neither contact; B's app is quit while A meets C).
3. Android/Nearby and location-based routing still wait.
