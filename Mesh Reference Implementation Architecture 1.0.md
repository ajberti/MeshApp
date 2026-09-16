# Mesh Reference Implementation Architecture 1.0

**Project:** Offline P2P Mesh Messaging  
**Protocol:** Mesh Protocol 1.0  
**Document status:** Draft implementation architecture  
**Target platforms:** Android and iOS  
**Shared core:** Rust

---

# 1. Objective

This document defines the software architecture for the first working implementation of Mesh Protocol 1.0.

The reference implementation must demonstrate:

```text
Phone A → Phone B → Phone C
```

where:

- all phones have Internet access disabled;
- A creates a message addressed to C;
- A and C never communicate directly;
- B receives and stores the encrypted Bundle;
- B cannot decrypt the message;
- B later encounters C;
- C receives and decrypts the message;
- C generates a delivery acknowledgement;
- the acknowledgement can later propagate back toward A.

The implementation shall be structured so this can later expand to:

```text
A → B → C → D → E → ... → Destination
```

without redesigning the application.

---

# 2. High-Level Architecture

The application is divided into three major components.

```text
+------------------------------------------------+
|                Native Application              |
|                                                |
| Android: Kotlin                                |
| iOS: Swift                                     |
|                                                |
| UI                                             |
| Permissions                                    |
| Notifications                                  |
| Secure OS storage                              |
| Radio transports                               |
+-----------------------+------------------------+
                        |
                        | UniFFI
                        |
+-----------------------v------------------------+
|                  Rust Mesh Core                |
|                                                |
| Identity                                       |
| Cryptography                                   |
| Bundle creation                                |
| Bundle parsing                                 |
| Session protocol                               |
| Synchronisation                                |
| Routing                                        |
| Deduplication                                  |
| Storage                                        |
| Delivery ACK processing                        |
| Protocol state machines                        |
+-----------------------+------------------------+
                        |
                        |
+-----------------------v------------------------+
|                     SQLite                     |
|                                                |
| Contacts                                       |
| Conversations                                  |
| Messages                                       |
| Bundles                                        |
| Tombstones                                     |
| Relay state                                    |
| Transfer state                                 |
+------------------------------------------------+
```

The critical rule is:

> Native code owns the radios. Rust owns the network protocol.

---

# 3. Technology Stack

Recommended implementation:

```text
Shared protocol core:
Rust

Rust/native bindings:
UniFFI

Android:
Kotlin
Jetpack Compose

iOS:
Swift
SwiftUI

Database:
SQLite owned by Rust

MVP transport:
Nearby Connections

Future transports:
Wi-Fi Aware
BLE
Wi-Fi Direct
Local LAN
```

Rust is used because the protocol must behave identically across platforms and because cryptographic and binary protocol code benefits from its memory-safety model.

---

# 4. Repository Structure

Recommended monorepo:

```text
mesh/
│
├── Cargo.toml
│
├── README.md
│
├── docs/
│   ├── mesh-protocol-1.0.md
│   ├── architecture-1.0.md
│   └── threat-model.md
│
├── crates/
│   │
│   ├── mesh-types/
│   ├── mesh-crypto/
│   ├── mesh-wire/
│   ├── mesh-store/
│   ├── mesh-routing/
│   ├── mesh-core/
│   ├── mesh-bindings/
│   └── mesh-sim/
│
├── android/
│   └── app/
│
├── ios/
│   └── MeshApp/
│
├── tools/
│   └── mesh-cli/
│
└── test-vectors/
```

---

# 5. Rust Workspace

Use a Cargo workspace.

Conceptually:

```toml
[workspace]
members = [
    "crates/mesh-types",
    "crates/mesh-crypto",
    "crates/mesh-wire",
    "crates/mesh-store",
    "crates/mesh-routing",
    "crates/mesh-core",
    "crates/mesh-bindings",
    "crates/mesh-sim",
]
```

The crates deliberately separate security-critical functionality from routing and platform integration.

---

# 6. mesh-types

Contains protocol-independent types.

Examples:

```rust
pub struct UserId([u8; 16]);
pub struct NodeId([u8; 16]);
pub struct BundleId([u8; 16]);
pub struct MessageId([u8; 16]);
pub struct ConversationId([u8; 16]);
pub struct DiscoveryId([u8; 16]);
pub struct LinkId(u64);
```

Do not pass raw `Vec<u8>` identifiers throughout the program.

Using distinct Rust types prevents errors such as accidentally comparing a Message ID with a Bundle ID.

---

# 7. mesh-crypto

Responsible exclusively for cryptographic operations.

Modules:

```text
mesh-crypto/
├── identity.rs
├── signing.rs
├── key_exchange.rs
├── session_crypto.rs
├── message_crypto.rs
├── key_derivation.rs
├── fingerprint.rs
└── random.rs
```

Recommended primitives:

```text
Ed25519
X25519
HKDF-SHA256
ChaCha20-Poly1305
XChaCha20-Poly1305
SHA-256
```

Likely Rust libraries include:

```text
ed25519-dalek
x25519-dalek
chacha20poly1305
hkdf
sha2
rand_core
zeroize
```

No cryptographic primitive shall be implemented manually.

---

# 8. Identity Structure

Conceptual Rust type:

```rust
pub struct Identity {
    pub user_id: UserId,

    signing_private: SecretSigningKey,
    signing_public: SigningPublicKey,

    encryption_private: SecretEncryptionKey,
    encryption_public: EncryptionPublicKey,
}
```

Private fields shall not implement:

```text
Debug
Display
Serialize
```

unless serialization is explicitly required for secure storage.

---

# 9. Secret Memory

Sensitive temporary values should use types that clear their backing memory on destruction where practical.

Examples:

```text
private keys
derived encryption keys
shared secrets
plaintext key material
```

Secret values must never appear in application logs.

---

# 10. mesh-wire

This crate implements Mesh Protocol wire encoding.

Responsibilities:

```text
CBOR encoding
CBOR decoding
Frame parsing
Frame creation
Bundle serialisation
Bundle validation
Version negotiation
Protocol limits
```

Modules:

```text
mesh-wire/
├── frame.rs
├── bundle.rs
├── hello.rs
├── inventory.rs
├── transfer.rs
├── ack.rs
└── error.rs
```

---

# 11. Wire Bundle Representation

Recommended Rust model:

```rust
pub struct ImmutableBundleHeader {
    pub protocol_version: u16,
    pub bundle_id: BundleId,
    pub bundle_type: BundleType,
    pub sender_id: UserId,
    pub destination_id: UserId,
    pub created_at: i64,
    pub ttl_seconds: u32,
    pub hop_limit: u16,
    pub priority: Priority,
    pub payload_length: u32,
}

pub struct RelayHeader {
    pub hop_count: u16,
}

pub struct WireBundle {
    pub immutable: ImmutableBundleHeader,
    pub relay: RelayHeader,
    pub encrypted_payload: Vec<u8>,
    pub sender_signature: Vec<u8>,
}
```

The signature covers:

```text
ImmutableBundleHeader
+
EncryptedPayload
```

but not:

```text
RelayHeader
```

because relays increment the hop count.

---

# 12. mesh-store

The Rust core owns the SQLite database.

Native Android and iOS code must not independently modify Mesh protocol tables.

Benefits:

```text
one database schema
one migration system
one routing implementation
one transaction model
identical behaviour on Android and iOS
```

A library such as `rusqlite` is suitable.

---

# 13. Database Location

Native code supplies the Rust core with an application-private path.

Example conceptually:

```text
Android:

/data/data/<application>/files/mesh.db


iOS:

<Application Support>/mesh.db
```

Rust opens and owns the database.

Recommended SQLite settings:

```sql
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
```

---

# 14. Database Schema

Initial schema:

```sql
CREATE TABLE contacts (
    user_id             BLOB PRIMARY KEY,
    display_name        TEXT NOT NULL,
    signing_public_key  BLOB NOT NULL,
    encryption_public_key BLOB NOT NULL,
    trust_state         INTEGER NOT NULL,
    fingerprint         TEXT NOT NULL,
    created_at          INTEGER NOT NULL
);
```

---

# 15. Conversations

```sql
CREATE TABLE conversations (
    conversation_id BLOB PRIMARY KEY,
    remote_user_id  BLOB NOT NULL,
    created_at      INTEGER NOT NULL,
    last_message_at INTEGER
);
```

---

# 16. User Messages

```sql
CREATE TABLE messages (
    message_id          BLOB PRIMARY KEY,
    conversation_id     BLOB NOT NULL,
    sender_id           BLOB NOT NULL,
    recipient_id        BLOB NOT NULL,

    direction           INTEGER NOT NULL,

    sent_at             INTEGER,
    received_at         INTEGER,

    state               INTEGER NOT NULL,

    content_type        TEXT NOT NULL,

    content_nonce       BLOB NOT NULL,
    content_ciphertext  BLOB NOT NULL,

    original_bundle_id  BLOB,

    FOREIGN KEY(conversation_id)
        REFERENCES conversations(conversation_id)
);
```

Possible states:

```text
QUEUED
MESH_RELAYED
DELIVERED
FAILED
```

---

# 17. Local Message Encryption

The plaintext copy shown to the user must also be encrypted at rest.

Generate:

```text
LocalDataKey = random 256-bit key
```

and store the key using native secure storage.

Message contents are encrypted before writing to SQLite using:

```text
XChaCha20-Poly1305
```

SQLite therefore contains:

```text
nonce
ciphertext
```

rather than plaintext message bodies.

---

# 18. Bundle Store

```sql
CREATE TABLE bundles (
    bundle_id           BLOB PRIMARY KEY,

    bundle_type         INTEGER NOT NULL,

    sender_id           BLOB NOT NULL,
    destination_id      BLOB NOT NULL,

    created_at          INTEGER NOT NULL,
    first_seen_at       INTEGER NOT NULL,

    ttl_seconds         INTEGER NOT NULL,

    hop_limit           INTEGER NOT NULL,
    hop_count           INTEGER NOT NULL,

    priority            INTEGER NOT NULL,

    encrypted_payload   BLOB NOT NULL,
    sender_signature    BLOB NOT NULL,

    forward_count       INTEGER NOT NULL DEFAULT 0,

    state               INTEGER NOT NULL,

    size_bytes          INTEGER NOT NULL
);
```

---

# 19. Tombstones

```sql
CREATE TABLE tombstones (
    bundle_id       BLOB PRIMARY KEY,
    reason          INTEGER NOT NULL,
    created_at      INTEGER NOT NULL,
    expires_at      INTEGER NOT NULL
);
```

Tombstones prevent already-delivered messages from circulating indefinitely.

---

# 20. Partial Transfers

```sql
CREATE TABLE partial_transfers (
    bundle_id       BLOB PRIMARY KEY,
    expected_size   INTEGER NOT NULL,
    received_size   INTEGER NOT NULL,
    data            BLOB NOT NULL,
    updated_at      INTEGER NOT NULL
);
```

Version 1 may initially omit resumption and add this once direct transfer is proven.

---

# 21. Forwarding History

```sql
CREATE TABLE bundle_forward_history (
    bundle_id       BLOB NOT NULL,
    peer_hint       BLOB,
    forwarded_at    INTEGER NOT NULL,

    PRIMARY KEY(bundle_id, peer_hint)
);
```

Peer hints are not considered permanent identities.

They exist only to reduce needless retransmission.

---

# 22. mesh-routing

Responsible for determining:

```text
What should I send?
To whom?
In what order?
Should I store this Bundle?
Should I delete this Bundle?
```

No radio code belongs here.

---

# 23. Routing Interface

Example:

```rust
pub trait RoutingPolicy {
    fn evaluate(
        &self,
        bundle: &BundleMetadata,
        peer: &PeerContext,
    ) -> RoutingDecision;
}
```

Possible results:

```rust
pub enum RoutingDecision {
    SendImmediately,
    Offer,
    Skip,
    Drop,
}
```

---

# 24. Version 1 Routing Policy

Version 1 uses Controlled Epidemic Routing.

Priority:

```text
1 Destination is current peer
2 Delivery ACK
3 Emergency
4 High
5 Normal
6 Bulk
```

Eligibility:

```rust
if expired(bundle) {
    Drop
}
else if hop_count >= hop_limit {
    Skip
}
else if peer_has_bundle(bundle.id) {
    Skip
}
else if peer_is_destination(bundle) {
    SendImmediately
}
else if replication_limit_reached(bundle) {
    Skip
}
else {
    Offer
}
```

---

# 25. mesh-core

`mesh-core` is the principal protocol engine.

Conceptual object:

```rust
pub struct MeshCore {
    identity: Identity,
    store: MeshStore,
    router: Router,
    sessions: HashMap<LinkId, MeshSession>,
    config: MeshConfig,
}
```

There should normally be one `MeshCore` instance per application process.

---

# 26. Actor Model

The core should behave as a single-threaded state machine.

Native components deliver events to it.

The core returns actions.

Conceptually:

```text
Native Event
     |
     v
+------------+
| Mesh Core  |
+------------+
     |
     v
Core Actions
```

This dramatically reduces race conditions in:

```text
routing
bundle state
session state
database updates
ACK processing
```

---

# 27. Core Event Interface

Example:

```rust
pub enum CoreEvent {

    PeerDiscovered {
        peer_token: Vec<u8>,
        discovery_id: DiscoveryId,
    },

    PeerLost {
        peer_token: Vec<u8>,
    },

    LinkOpened {
        link_id: LinkId,
        peer_token: Vec<u8>,
    },

    BytesReceived {
        link_id: LinkId,
        data: Vec<u8>,
    },

    LinkClosed {
        link_id: LinkId,
    },

    SendText {
        recipient: UserId,
        text: String,
    },

    Tick {
        now_ms: i64,
    },

    AppForegrounded,

    AppBackgrounded,
}
```

---

# 28. Core Actions

The Rust core responds with zero or more actions.

```rust
pub enum CoreAction {

    Connect {
        peer_token: Vec<u8>,
    },

    SendBytes {
        link_id: LinkId,
        data: Vec<u8>,
    },

    CloseLink {
        link_id: LinkId,
    },

    MessageUpdated {
        message_id: MessageId,
    },

    MessageReceived {
        message_id: MessageId,
    },

    MeshStatusChanged,

    Log {
        level: LogLevel,
        code: String,
    },
}
```

Native code executes these actions.

---

# 29. Why Event/Action Instead of Callbacks

This model allows the identical Mesh Core to run inside:

```text
Android
iOS
command-line tools
unit tests
network simulator
```

The simulator merely provides fake CoreEvents and records CoreActions.

No actual Bluetooth or Wi-Fi hardware is required.

---

# 30. UniFFI Boundary

`mesh-bindings` exposes `MeshCore` to:

```text
Kotlin
Swift
```

Recommended public interface:

```rust
pub struct MeshEngine {
    ...
}
```

Methods conceptually:

```text
open()
process_event()
send_text()
add_contact()
get_conversations()
get_messages()
get_mesh_status()
close()
```

Avoid exposing internal database objects through FFI.

---

# 31. FFI Principle

The FFI API should be coarse-grained.

Good:

```text
send_text(user_id, message)
```

Avoid APIs like:

```text
create_bundle()
encrypt_payload()
insert_bundle_row()
update_forward_count()
```

Those operations belong inside Rust.

---

# 32. Transport Architecture

Native applications implement:

```text
TransportManager
```

which owns one or more:

```text
MeshTransport
```

implementations.

Conceptually:

```text
              TransportManager
                     |
       +-------------+-------------+
       |             |             |
     Nearby       WiFiAware       BLE
    Transport      Transport    Transport
```

For MVP:

```text
TransportManager
      |
Nearby Connections
```

only.

---

# 33. Transport Interface

Conceptual native interface:

```text
startAdvertising()
stopAdvertising()

startDiscovery()
stopDiscovery()

connect(peerToken)

send(linkID, data)

disconnect(linkID)
```

Transport callbacks produce:

```text
peerFound
peerLost
connected
bytesReceived
disconnected
```

These become `CoreEvent` objects.

---

# 34. Discovery Advertisement

The native transport advertises a small Mesh context.

Example:

```text
protocol major
discovery ID
capability flags
```

The context must not contain:

```text
real name
telephone number
permanent User ID
email address
```

---

# 35. Discovery ID Rotation

Rust generates:

```text
DiscoveryID
```

approximately every:

```text
15 minutes
```

Native transports receive the new advertisement and restart advertising where required.

---

# 36. Simultaneous Connection Prevention

Every device both advertises and discovers.

This means:

```text
A discovers B
B discovers A
```

at approximately the same time.

Without a rule they may create two connections.

Use the Discovery IDs as a deterministic tiebreaker.

Example:

```text
if LocalDiscoveryID < RemoteDiscoveryID:
    initiate connection
else:
    wait for incoming connection
```

This prevents most duplicate connection races.

---

# 37. Mesh Session State Machine

Each active link has a Rust `MeshSession`.

States:

```text
CONNECTED
    |
    v
HELLO
    |
    v
KEY_EXCHANGE
    |
    v
SECURE
    |
    v
INVENTORY
    |
    v
TRANSFER
    |
    v
IDLE
    |
    v
CLOSED
```

Invalid state transitions terminate the session.

---

# 38. Session Object

Conceptually:

```rust
pub struct MeshSession {
    link_id: LinkId,

    state: SessionState,

    remote_discovery_id: Option<DiscoveryId>,

    local_ephemeral_key: Option<SecretKey>,
    remote_ephemeral_key: Option<PublicKey>,

    tx_key: Option<SessionKey>,
    rx_key: Option<SessionKey>,

    tx_counter: u64,
    rx_counter: u64,

    peer_inventory: PeerInventory,
}
```

---

# 39. Session Handshake

Once native reports:

```text
LinkOpened
```

Rust begins:

```text
A                         B

HELLO ------------------->

      <------------------- HELLO

KEY_INIT ---------------->

      <------------------- KEY_REPLY

SESSION_OK -------------->

      <------------------- SESSION_OK
```

All following frames are protected by the derived Mesh Session encryption keys.

---

# 40. Inventory Strategy for MVP

Do not implement Bloom filters initially.

For the first prototype send an explicit list of Bundle IDs.

Example:

```text
INVENTORY_SUMMARY

[
   A113...
   B771...
   C992...
]
```

For a few hundred Bundles this is acceptable.

Once realistic networks contain thousands of Bundles, replace this with:

```text
Bloom filters
+
priority reconciliation
```

without changing the surrounding architecture.

---

# 41. Sending a New Message

Suppose Alice sends:

```text
"Are you safe?"
```

to Charlie.

Call:

```text
MeshCore.send_text(
    CharlieUserID,
    "Are you safe?"
)
```

---

# 42. Message Creation Path

Rust performs:

```text
1 Look up Charlie's public encryption key

2 Generate MessageID

3 Calculate ConversationID

4 Create plaintext message payload

5 CBOR encode payload

6 Generate ephemeral X25519 key pair

7 Calculate recipient shared secret

8 Derive message encryption key

9 Encrypt payload

10 Generate BundleID

11 Construct ImmutableBundleHeader

12 Sign immutable header + encrypted payload

13 Store Bundle

14 Store locally encrypted conversation message

15 Mark message QUEUED
```

No transport is required during this process.

Alice may be completely alone.

---

# 43. Alice Encounters Bob

Native transport observes Bob.

```text
Phone A                      Phone B

Discovery  <--------------> Discovery
```

Native sends:

```text
PeerDiscovered(B)
```

to the Rust core.

Using the deterministic discovery rule, one side outputs:

```text
CoreAction::Connect(B)
```

---

# 44. Transport Connection

Native establishes the physical connection.

Example:

```text
Nearby Connections
```

Native then reports:

```text
CoreEvent::LinkOpened
```

to Rust.

Rust begins Mesh Protocol session establishment.

---

# 45. Alice and Bob Synchronise

After encryption is established:

```text
A                        B

INVENTORY ------------->

          <------------- INVENTORY
```

Suppose:

```text
A contains Bundle X
B does not
```

where Bundle X is intended for Charlie.

Alice evaluates:

```text
not expired
hop_count < hop_limit
replication allowed
Bob doesn't have X
```

Result:

```text
OFFER X
```

---

# 46. Bob Requests Bundle

```text
A                         B

BUNDLE_OFFER X --------->

           <------------- BUNDLE_REQUEST X

BUNDLE_DATA X ---------->

           <------------- BUNDLE_COMPLETE X
```

Bob verifies:

```text
format
Bundle ID
size
TTL
hop limit
signature structure
```

Bob cannot decrypt the user payload.

---

# 47. Bob Stores Bundle

Bob stores:

```text
Bundle X
Destination = Charlie
hopCount = 1
```

Alice increments:

```text
forwardCount(X)
```

Alice's UI may now show:

```text
Relayed
```

rather than:

```text
Delivered
```

These have different meanings.

---

# 48. Alice Leaves

Alice can now:

```text
walk away
lose battery
turn phone off
leave the country
```

and Bundle X still exists.

Bob physically carries it.

This is the defining property of the architecture.

---

# 49. Bob Encounters Charlie

Several hours later:

```text
Bob                Charlie
```

discover one another.

The same handshake occurs.

Charlie sends an inventory that does not contain Bundle X.

Bob's router detects:

```text
Bundle X.destination == Charlie.UserID
```

It immediately receives the highest transfer priority.

---

# 50. Bob Transfers Bundle X

```text
Bob                       Charlie

BUNDLE_OFFER X ---------->

             <------------ BUNDLE_REQUEST X

BUNDLE_DATA X ----------->

             <------------ BUNDLE_COMPLETE X
```

Charlie sees:

```text
destinationID == localUserID
```

and invokes local delivery.

---

# 51. Charlie Decrypts

Rust performs:

```text
1 Parse encrypted payload

2 Extract sender ephemeral public key

3 Perform X25519 using Charlie's private key

4 Derive MessageKey

5 Authenticate/decrypt ChaCha20-Poly1305 payload

6 Decode CBOR

7 Obtain Alice User ID

8 Load Alice public signing key

9 Verify Bundle signature

10 Check MessageID replay table

11 Store local message

12 Notify UI
```

Charlie's UI displays:

```text
Alice

Are you safe?
```

Bob has never possessed this plaintext.

---

# 52. Delivery ACK Creation

Charlie now generates:

```text
DELIVERY_ACK
```

containing:

```text
original BundleID
original MessageID
Charlie UserID
delivery status
timestamp
```

The ACK is addressed to Alice.

The ACK itself becomes another encrypted Bundle.

---

# 53. ACK Propagation

Charlie does not need to find Alice directly.

For example:

```text
Charlie
   |
   v
David
   |
   v
Emma
   |
   v
Alice
```

When Alice eventually receives the ACK:

```text
Message.state = DELIVERED
```

The UI changes to:

```text
Delivered
```

---

# 54. Tombstone Propagation

The ACK identifies the delivered original Bundle.

Relay nodes seeing the valid ACK may create:

```text
Tombstone(Bundle X)
```

and remove Bundle X from their forwarding queue.

This gradually cleans delivered traffic from the network.

---

# 55. Android Application Structure

Recommended:

```text
android/app/
│
├── ui/
│   ├── conversations/
│   ├── conversation/
│   ├── contacts/
│   ├── meshstatus/
│   └── settings/
│
├── mesh/
│   ├── MeshRuntime.kt
│   ├── TransportManager.kt
│   └── NearbyTransport.kt
│
├── security/
│   └── SecureStore.kt
│
└── notifications/
```

---

# 56. Android MeshRuntime

`MeshRuntime` connects native Android components to Rust.

Conceptually:

```text
Nearby callback
      |
      v
MeshRuntime
      |
      v
Rust MeshCore
      |
      v
CoreActions
      |
      v
TransportManager
```

`MeshRuntime` executes all protocol interactions through a serialized coroutine or actor.

---

# 57. Android UI

The UI should use ordinary application state rather than directly accessing Mesh protocol objects.

Example:

```text
ConversationViewModel
       |
       v
MeshRepository
       |
       v
MeshEngine
```

Views should not know:

```text
Bundle IDs
hop counts
session keys
CBOR
routing algorithms
```

unless displaying an explicit diagnostics screen.

---

# 58. iOS Application Structure

Recommended:

```text
MeshApp/
│
├── UI/
│   ├── Conversations/
│   ├── Conversation/
│   ├── Contacts/
│   ├── MeshStatus/
│   └── Settings/
│
├── Mesh/
│   ├── MeshRuntime.swift
│   ├── TransportManager.swift
│   └── NearbyTransport.swift
│
├── Security/
│   └── SecureStore.swift
│
└── Notifications/
```

The structure should deliberately mirror Android.

---

# 59. Native Secure Storage

The following must not be stored in ordinary SQLite rows:

```text
Ed25519 private identity key
X25519 private encryption key
LocalDataKey
```

On Android they are protected through Android secure key-storage facilities.

On iOS they are protected using Keychain services.

Rust receives the secret material only when required to operate.

---

# 60. First-Launch Provisioning

On first launch:

```text
Native starts app

        ↓

SecureStore empty?

        ↓ yes

Rust generates:
Identity signing key
Encryption key
LocalDataKey
NodeID

        ↓

Native persists secret material securely

        ↓

Rust creates database

        ↓

Rust creates UserID

        ↓

Application ready
```

No server call occurs.

---

# 61. Subsequent Startup

```text
Native application starts

        ↓

Load protected secrets

        ↓

Open MeshCore

        ↓

Open SQLite

        ↓

Run schema migrations

        ↓

Generate new DiscoveryID

        ↓

Start advertising/discovery
```

---

# 62. Rust Core Threading Model

One logical executor owns:

```text
MeshCore
SQLite connection
Session state
Router
```

This means no two threads can simultaneously:

```text
accept the same Bundle
increment the same hop counter
process the same ACK
change the same session state
```

Transport events may arrive on arbitrary native threads, but they must be serialized before reaching Rust.

---

# 63. Timers

Rust receives regular:

```text
Tick
```

events.

Suggested active interval:

```text
1 second
```

The Tick handles:

```text
TTL expiry
session timeout
discovery ID rotation
partial transfer cleanup
tombstone expiry
relay-store maintenance
```

It should not continuously poll the database unnecessarily.

---

# 64. Application Lifecycle

MVP behaviour:

```text
Foreground:
Full mesh operation

Background:
Best effort

Terminated:
No guaranteed mesh operation
```

The first prototype should concentrate on reliable foreground operation.

Do not attempt to solve every mobile OS background restriction before proving the routing system.

---

# 65. Mesh Status Model

Rust exposes a simple object:

```rust
pub struct MeshStatus {
    pub discovered_peers: u32,
    pub active_links: u32,
    pub queued_user_messages: u32,
    pub relay_bundle_count: u32,
    pub bytes_relayed_today: u64,
}
```

The UI converts this into user-friendly information.

Example:

```text
Mesh active

4 nearby nodes
2 connected

1 message waiting
38 messages relayed
```

---

# 66. UI Message States

Recommended user-visible states:

```text
Queued
Relayed
Delivered
Failed
```

Definitions:

**Queued**

No successful relay yet.

**Relayed**

At least one other node has accepted the Bundle.

**Delivered**

Signed delivery acknowledgement received.

**Failed**

Message expired before acknowledgement.

Do not display conventional messaging ticks if their meaning could be confused with Internet messaging behaviour.

---

# 67. Transport Manager

The transport manager allows several transports to coexist later.

Example:

```text
                  MeshCore
                     |
                     v
              TransportManager
               /     |      \
              /      |       \
        Nearby   WiFiAware    BLE
```

A Bundle has no knowledge of which transport carries it.

---

# 68. Transport Preference

Future transport selection might prefer:

```text
1 Existing connection
2 Wi-Fi Aware
3 Nearby high-bandwidth link
4 Wi-Fi Direct
5 Bluetooth
6 BLE
```

But Version 1 should not implement complex transport selection.

Use one transport first.

---

# 69. Nearby Connections MVP

For the initial Android/iOS proof of concept:

```text
Transport = Nearby Connections
Topology = cluster
Payload = bytes
```

Native Nearby endpoint IDs remain entirely outside Mesh Protocol.

They are temporary transport identifiers.

---

# 70. Protocol Service Identifier

Use a fixed application service identifier.

For example:

```text
com.meshproject.mesh
```

Development and production environments may use different identifiers:

```text
com.meshproject.mesh.dev

com.meshproject.mesh
```

This prevents development devices interfering with production networks.

---

# 71. Maximum Frame Size

Set conservative limits.

Example:

```text
Control frame:
64 KB maximum

Version 1 text Bundle:
64 KB maximum
```

Frame parser code must reject claimed sizes exceeding protocol limits before allocating large buffers.

---

# 72. Parser Security

Assume every byte received from another device is malicious.

The decoder shall validate:

```text
Magic
Protocol version
Frame type
Length
CBOR nesting
Array size
String size
Bundle size
Key length
Signature length
```

before processing the frame.

Never trust remote length fields when allocating memory.

---

# 73. Session Replay Protection

Encrypted session frames use monotonically increasing counters.

Example:

```text
TX counter:
1
2
3
4
...
```

The counter becomes part of AEAD nonce construction or authenticated framing.

Frames with duplicate or invalid counters shall be rejected.

---

# 74. Message Replay Protection

Destination devices maintain delivered Message IDs.

If:

```text
MessageID already exists
```

the message is not displayed again.

An ACK may still be generated to help remove redundant copies from the mesh.

---

# 75. Bundle Deduplication

Before accepting a Bundle:

```text
SELECT bundle_id
FROM bundles
WHERE bundle_id = ?
```

and:

```text
SELECT bundle_id
FROM tombstones
WHERE bundle_id = ?
```

If either exists, the Bundle does not enter relay storage again.

---

# 76. Transaction Boundaries

Important state transitions should be transactional.

For example, receiving a complete Bundle should atomically:

```text
insert Bundle
update inventory state
record encounter state
```

Local delivery should atomically:

```text
insert Message
mark Bundle delivered locally
create ACK Bundle
```

This prevents crashes from leaving contradictory state.

---

# 77. Relay Store Maintenance

Periodic maintenance:

```text
Delete expired Bundles

Delete expired tombstones

Delete abandoned partial transfers

Enforce storage quota

Compact forwarding history
```

Should run:

```text
at startup
periodically while active
when storage pressure occurs
```

---

# 78. Configuration

Rust owns a `MeshConfig`.

Example:

```rust
pub struct MeshConfig {
    pub protocol_major: u8,
    pub protocol_minor: u8,

    pub default_ttl_secs: u32,
    pub default_hop_limit: u16,

    pub relay_quota_bytes: u64,

    pub normal_replication_limit: u16,
    pub high_replication_limit: u16,
    pub emergency_replication_limit: u16,

    pub max_bundle_bytes: u32,
}
```

Configuration should be versioned.

---

# 79. Sensible Initial Defaults

For development:

```text
Text TTL:
72 hours

Hop limit:
20

Relay storage:
250 MB

Normal replication:
6

High replication:
12

Emergency replication:
20

Maximum text Bundle:
64 KB
```

These are starting points for testing, not permanent protocol constants.

---

# 80. Logging Architecture

Rust outputs structured diagnostics.

Example:

```text
SESSION_STARTED
SESSION_SECURE
INVENTORY_RECEIVED
BUNDLE_OFFERED
BUNDLE_ACCEPTED
BUNDLE_RELAYED
BUNDLE_DELIVERED
ACK_RECEIVED
BUNDLE_EXPIRED
```

Production logs must never include message plaintext or secret key material.

---

# 81. Developer Diagnostics Screen

During development provide an advanced screen showing:

```text
Node ID
Current Discovery ID
User ID fingerprint
Nearby peers
Active links
Bundle queue
Bundle IDs
Hop counts
TTL
Forward counts
Session state
Transport
Bytes sent
Bytes received
```

This will be extremely valuable when testing physical devices.

It can be hidden from normal users later.

---

# 82. Rust CLI Test Client

Create:

```text
tools/mesh-cli
```

which uses `mesh-core` without a phone.

Example:

```bash
mesh-cli identity
mesh-cli bundles
mesh-cli contacts
mesh-cli send <userid> "hello"
```

Later this can become the basis of:

```text
Raspberry Pi relays
community nodes
server gateways
```

---

# 83. Simulator

`mesh-sim` is strategically important.

It runs hundreds or thousands of virtual MeshCore instances.

Each virtual node has:

```text
MeshCore
virtual database
virtual transport
virtual clock
```

The simulator decides when nodes encounter each other.

---

# 84. Simulation Example

Create:

```text
500 virtual phones
```

distributed across a simulated city.

Each simulated minute:

```text
some nodes move
some nodes encounter others
some create messages
some disappear
some run out of battery
```

Measure:

```text
delivery percentage
median delivery time
Bundle replication
bytes transmitted
storage consumption
hop distribution
```

---

# 85. Deterministic Simulation

Inject:

```text
Clock
RandomSource
```

into MeshCore instead of hard-coding them.

Production uses:

```text
SystemClock
SecureRandom
```

Tests use:

```text
FakeClock
SeededRandom
```

This allows failed simulations to be reproduced exactly.

---

# 86. Unit Tests

Required Rust tests include:

```text
Bundle encode/decode

Signature verification

Signature tampering

Encryption/decryption

Wrong recipient failure

Expired Bundle rejection

Hop limit rejection

Duplicate Bundle rejection

Duplicate Message rejection

ACK processing

Tombstone processing

TTL calculation

Routing priority

Replication limit
```

---

# 87. Protocol Test Vectors

Maintain:

```text
test-vectors/
```

containing known inputs and expected outputs.

Examples:

```text
identity vectors
Bundle CBOR
signatures
X25519 shared-secret tests
message encryption vectors
HELLO frames
ACK frames
```

Android, iOS and Rust tooling can verify compatibility against these.

---

# 88. Fuzz Testing

The most important fuzz target is the wire parser.

Feed arbitrary bytes into:

```text
FrameDecoder
BundleDecoder
CBOR payload decoder
```

Expected result:

```text
valid object
or
controlled error
```

Never:

```text
panic
crash
unbounded allocation
undefined behaviour
```

---

# 89. Integration Test: Two Nodes

First integration test:

```text
A <----> B
```

No Internet.

A sends:

```text
HELLO B
```

B receives and displays it.

Success proves:

```text
transport
session
crypto
Bundle format
local delivery
```

---

# 90. Integration Test: Three Nodes

Next:

```text
A → B → C
```

Procedure:

```text
1 Disable Internet on all devices.

2 A has C as a trusted contact.

3 Keep C out of radio range.

4 A sends a message to C.

5 Bring B near A.

6 Confirm B accepts encrypted Bundle.

7 Move B away from A.

8 Turn A off.

9 Bring B near C.

10 Confirm C receives plaintext.

11 Inspect B database.

12 Confirm no plaintext exists.

13 Confirm C creates ACK.
```

This is the project's first major milestone.

---

# 91. Four-Node ACK Test

Test:

```text
MESSAGE:

A → B → C


ACK:

C → D → A
```

This proves delivery receipts do not rely on the original forwarding path.

---

# 92. Network Partition Test

Create two groups:

```text
A B C

D E F
```

No contact between groups.

Messages remain queued.

Later move C near D.

Expected:

```text
Bundles cross partition
```

and spread through the other group.

---

# 93. Duplicate Path Test

Topology:

```text
      B
     / \
A --    -- D
     \ /
      C
```

D receives the message through both:

```text
A → B → D

A → C → D
```

Expected user result:

```text
one message
```

not two.

---

# 94. Failure Injection

Test:

```text
disconnect halfway through transfer

kill app halfway through transfer

corrupt frame

replay old frame

alter signature

wrong destination

huge claimed payload

invalid CBOR

duplicate ACK

clock 12 hours wrong

clock 24 hours wrong
```

Each failure must produce controlled behaviour.

---

# 95. Development Phase 0

## Core skeleton

Build:

```text
Rust workspace

UniFFI bindings

Android shell

iOS shell

MeshCore event/action architecture
```

Success:

```text
Kotlin calls Rust.

Swift calls Rust.
```

---

# 96. Development Phase 1

## P2P transport

Implement Nearby Connections.

Success:

```text
Android ↔ Android

iPhone ↔ iPhone

Android ↔ iPhone
```

can exchange raw test bytes without Internet.

---

# 97. Development Phase 2

## Mesh sessions

Implement:

```text
HELLO
version negotiation
X25519 link handshake
encrypted frames
```

Success:

```text
two devices establish Mesh Session
```

and exchange encrypted protocol frames.

---

# 98. Development Phase 3

## Identity and direct messaging

Implement:

```text
identity generation
contact import
QR contact exchange
Bundle creation
end-to-end encryption
signature verification
```

Success:

```text
A → B
```

encrypted messaging.

---

# 99. Development Phase 4

## Relay

Implement:

```text
Bundle database
inventory exchange
Bundle offers
Bundle requests
routing
hop counts
TTL
deduplication
```

Success:

```text
A → B → C
```

with A switched off before B meets C.

---

# 100. Development Phase 5

## Delivery acknowledgement

Implement:

```text
DELIVERY_ACK
ACK routing
message status
tombstones
```

Success:

```text
A → B → C

C → D → A ACK
```

---

# 101. Development Phase 6

## Resilience

Implement:

```text
interrupted transfers
storage quotas
rate limits
expiry
battery policies
background behaviour
multiple simultaneous peers
```

---

# 102. Development Phase 7

## Simulator

Implement large-scale testing.

Start:

```text
10 nodes
```

then:

```text
100
500
1,000+
```

---

# 103. Items Deliberately Deferred

Do not include in the first working implementation:

```text
group messaging

photos

voice notes

video

live voice

broadcast channels

satellite gateways

LoRa

geographic routing

reputation systems

Double Ratchet

anonymous routing

complex Bloom-filter synchronisation
```

These distract from proving the fundamental network.

---

# 104. MVP Definition

The MVP does not require a polished WhatsApp-like application.

It requires:

```text
Contact A
Contact C

Compose message

Send

Mesh status

Receive message

Delivery state
```

plus an extensive developer diagnostics screen.

---

# 105. Most Important Engineering Principle

Transport code must never implement routing decisions.

For example:

Bad:

```text
NearbyTransport decides
which Bundles Bob needs.
```

Correct:

```text
NearbyTransport reports Bob exists.

MeshCore decides whether Bob
should receive each Bundle.
```

---

# 106. Second Most Important Engineering Principle

The UI must never own message delivery state.

For example:

Bad:

```text
UI marks message delivered
after a socket write succeeds.
```

Correct:

```text
Rust marks message delivered
only after a valid destination ACK
returns through the mesh.
```

---

# 107. Third Most Important Engineering Principle

The application must function correctly even when every connection disappears unexpectedly.

The architecture should assume:

```text
connections are temporary

Bundles are persistent
```

rather than:

```text
connections are persistent

messages are temporary
```

---

# 108. Reference Data Flow

Complete logical flow:

```text
User types message
        |
        v
Native UI
        |
        v
MeshCore
        |
        +--> encrypt message
        |
        +--> sign Bundle
        |
        +--> SQLite
        |
        v
Relay queue


Nearby peer appears
        |
        v
TransportManager
        |
        v
MeshCore
        |
        +--> establish Mesh Session
        |
        +--> compare inventories
        |
        +--> routing decision
        |
        v
TransportManager
        |
        v
Nearby radio
        |
        v
Remote phone
        |
        v
Remote MeshCore
        |
        +--> store Bundle
        |
        +--> relay later
        |
        v
Destination
        |
        +--> decrypt
        |
        +--> verify
        |
        +--> display
        |
        +--> generate ACK
        |
        v
Mesh
        |
        v
Original sender
```

---

# 109. First Real Prototype

The first real development target should be exactly:

```text
3 physical phones
```

preferably:

```text
Android A
Android B
iPhone C
```

A knows C.

B knows nobody.

A creates:

```text
"Mesh test 001"
```

B receives ciphertext.

A is turned completely off.

B approaches C.

C displays:

```text
Mesh test 001
```

B's database and logs contain no plaintext.

That single experiment proves the central concept.

---

# 110. Reference Implementation Success Criterion

The Reference Implementation 1.0 is successful when:

> A text message created on an offline mobile phone can be securely stored, physically carried by an unrelated second phone, delivered to a third phone without the first phone being present, decrypted only by the intended recipient, and acknowledged through the same delay-tolerant Mesh Protocol.

At that point the project has progressed from an application idea to a functioning infrastructure-independent communications network.