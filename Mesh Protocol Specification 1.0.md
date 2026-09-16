# Mesh Protocol Specification 1.0

**Protocol name:** Mesh Protocol  
**Abbreviation:** MP  
**Version:** 1.0  
**Status:** Draft  
**Primary use case:** Infrastructure-independent mobile messaging  
**Supported topology:** Peer-to-peer, store-carry-forward, delay-tolerant mesh

---

# 1. Purpose

Mesh Protocol defines how devices discover one another, establish temporary connections, exchange encrypted messages, relay messages for other users, acknowledge delivery, suppress duplicates and expire obsolete data without relying on Internet infrastructure.

The protocol is designed for environments where:

- Internet connectivity is unavailable;
- cellular networks are unavailable;
- DNS is unavailable;
- central servers are unavailable;
- network topology changes continuously;
- devices may only encounter one another briefly;
- no end-to-end route currently exists between sender and recipient.

The protocol shall support communication of the form:

```text
Alice → Bob → Charlie → David
```

where:

- Alice creates a message for David;
- Bob and Charlie act only as relays;
- Bob and Charlie cannot decrypt the message;
- David may receive the message hours after Alice created it;
- Alice and David never need to be simultaneously connected.

---

# 2. Protocol Objectives

Mesh Protocol 1.0 shall provide:

1. Offline peer discovery.
2. Device-to-device connections.
3. Transport-independent operation.
4. End-to-end encrypted messages.
5. Store-and-forward relaying.
6. Duplicate suppression.
7. Message expiry.
8. Delivery acknowledgements.
9. Interrupted-transfer recovery.
10. Protocol version negotiation.
11. Resource controls.
12. Basic abuse resistance.

Mesh Protocol 1.0 does not attempt to provide:

- global real-time routing;
- anonymous communication;
- realtime voice;
- realtime video;
- group messaging;
- guaranteed delivery;
- guaranteed sender location privacy;
- protection against global radio traffic analysis.

---

# 3. Architectural Layers

Mesh Protocol shall be divided into five layers.

```text
+--------------------------------------+
| Application Layer                    |
| Messages / conversations             |
+--------------------------------------+
| End-to-End Message Layer             |
| Encryption / signatures              |
+--------------------------------------+
| Bundle Layer                         |
| Store / relay / TTL / priority       |
+--------------------------------------+
| Mesh Session Layer                   |
| Discovery / sync / transfer          |
+--------------------------------------+
| Transport Layer                      |
| BLE / Wi-Fi / Nearby / Wi-Fi Aware   |
+--------------------------------------+
```

No layer above the Transport Layer shall depend on a specific radio technology.

---

# 4. Fundamental Unit: Bundle

The fundamental transferable unit is called a **Bundle**.

A Bundle is:

> A self-contained encrypted object which may be stored and transported by devices other than its sender and recipient.

A Bundle may represent:

- a user message;
- a delivery acknowledgement;
- a future attachment;
- protocol control data.

Relay devices treat bundles primarily as opaque objects.

---

# 5. Identifiers

## 5.1 Node ID

Every installation generates a random 128-bit Node ID.

```text
NodeID = 16 random bytes
```

Example textual representation:

```text
47F8C2A1-445D-48BA-9421-B645286531A9
```

Node IDs must not be derived from:

- IMEI;
- MAC address;
- telephone number;
- Apple ID;
- Google account;
- hardware serial number.

Node IDs may change if the application is completely reset.

---

# 6. User Identity

A user identity is cryptographic rather than server-issued.

At first launch, the device generates:

```text
Identity Signing Key
Identity Encryption Key
```

Recommended algorithms:

```text
Signing:
Ed25519

Encryption:
X25519
```

The resulting public identity contains:

```text
IdentityPublicKey
EncryptionPublicKey
DisplayName
ProtocolCapabilities
```

The private keys never leave the device except through an explicit backup mechanism.

---

# 7. User ID

A stable User ID shall be derived from the identity public key.

Recommended:

```text
UserID =
SHA-256(IdentityPublicKey)[0..15]
```

This produces a 128-bit identifier.

Example display form:

```text
8D7F-293A-61BC-44F2-983A-2668-72D1-6119
```

Applications may show shorter fingerprints for usability, but protocol operations shall use the full value.

---

# 8. Contact Verification

Users establish trusted contacts by exchanging public identities.

Recommended methods:

1. QR code.
2. NFC.
3. Nearby exchange.
4. Manual fingerprint comparison.

The identity record exchanged shall contain:

```text
protocolVersion
userID
displayName
identityPublicKey
encryptionPublicKey
createdAt
signature
```

The signature covers every preceding field.

---

# 9. Contact Fingerprint

A human-verifiable fingerprint shall be calculated from:

```text
SHA-256(
    IdentityPublicKey ||
    EncryptionPublicKey
)
```

The application may display this as groups.

Example:

```text
7214 8821 9043 6612
```

or as words.

---

# 10. Discovery Privacy

Permanent User IDs shall not normally be broadcast during peer discovery.

Each node periodically generates an ephemeral Discovery ID.

```text
DiscoveryID = 16 random bytes
```

Recommended lifetime:

```text
15 minutes
```

Discovery advertisements should contain only:

```text
DiscoveryID
ProtocolMajorVersion
CapabilityFlags
```

Example:

```text
DiscoveryID: C7A9D11492BC61FD...
Protocol: 1
Capabilities: 0x13
```

This reduces passive tracking of permanent application identities.

---

# 11. Transport Abstraction

Mesh Protocol shall operate over any bidirectional reliable or semi-reliable byte transport.

Examples:

```text
Nearby Connections
Wi-Fi Aware
Bluetooth
BLE
Wi-Fi Direct
Local Wi-Fi
Future radio transports
```

The Mesh Core requires the following conceptual interface:

```text
discover()

advertise()

connect(peer)

send(bytes)

receive(bytes)

disconnect()
```

Transport-specific discovery identifiers shall never become permanent Mesh identity identifiers.

---

# 12. Link Establishment

Once two devices establish physical connectivity, they create a **Mesh Session**.

Example:

```text
Node A                  Node B

HELLO       -------->

            <--------   HELLO

KEY_INIT    -------->

            <--------   KEY_REPLY

SESSION_OK  -------->

            <--------   SESSION_OK
```

The resulting session is encrypted.

---

# 13. HELLO Frame

The first Mesh frame shall contain:

```text
protocolMajor
protocolMinor
discoveryID
sessionNonce
capabilities
maxFrameSize
```

Example:

```text
protocolMajor: 1
protocolMinor: 0

sessionNonce:
32 random bytes

maxFrameSize:
65536
```

---

# 14. Protocol Negotiation

Nodes shall negotiate the highest mutually supported protocol version.

Example:

```text
Node A:
1.0–1.3

Node B:
1.0–1.1

Selected:
1.1
```

If no compatible version exists:

```text
ERROR_UNSUPPORTED_VERSION
```

The connection shall terminate.

---

# 15. Link Encryption

Every peer-to-peer encounter shall establish temporary encrypted link keys.

Recommended primitives:

```text
Key agreement:
X25519

Key derivation:
HKDF-SHA256

Authenticated encryption:
ChaCha20-Poly1305
```

Each encounter generates fresh ephemeral X25519 keys.

---

# 16. Link Key Exchange

Node A generates:

```text
A_ephemeral_private
A_ephemeral_public
```

Node B generates:

```text
B_ephemeral_private
B_ephemeral_public
```

They exchange public keys.

Both calculate:

```text
SharedSecret =
X25519(
    local_private,
    remote_public
)
```

Session keys are generated using:

```text
HKDF-SHA256
```

with the complete handshake transcript included as context.

Separate keys shall be derived for:

```text
A → B encryption
B → A encryption
```

---

# 17. Link Authentication

Mesh Protocol 1.0 does not require relay devices to reveal their permanent user identity during every encounter.

The temporary link therefore primarily provides:

- confidentiality against passive listeners;
- integrity;
- protection against accidental corruption.

End-to-end bundle signatures provide sender authentication.

A future protocol version may add authenticated relay identities.

---

# 18. Frame Format

All Mesh Session frames use the following binary header.

```text
Offset   Size    Field
------   ----    -------------------
0        2       Magic
2        1       Major Version
3        1       Minor Version
4        1       Frame Type
5        1       Flags
6        4       Payload Length
10       N       Payload
```

Multi-byte integer fields use network byte order.

Magic value:

```text
0x4D50
```

ASCII:

```text
MP
```

---

# 19. Frame Types

Initial frame types:

```text
0x01 HELLO
0x02 KEY_INIT
0x03 KEY_REPLY
0x04 SESSION_OK

0x10 INVENTORY_SUMMARY
0x11 INVENTORY_REQUEST

0x20 BUNDLE_OFFER
0x21 BUNDLE_REQUEST
0x22 BUNDLE_DATA
0x23 BUNDLE_COMPLETE

0x30 RECEIPT_SUMMARY

0x40 PING
0x41 PONG

0x7F ERROR
```

Frame types `0x80–0xFF` are reserved for future extensions.

---

# 20. Payload Encoding

Structured Mesh Protocol payloads shall use:

```text
Canonical CBOR
```

Reasons include:

- compact representation;
- deterministic serialisation;
- binary support;
- extensibility;
- broad platform support.

Canonical encoding is required where data is cryptographically signed.

---

# 21. Bundle Structure

A Bundle consists of:

```text
BundleHeader
EncryptedPayload
SenderSignature
```

Logical structure:

```text
{
    version,
    bundleID,
    bundleType,
    destinationID,
    createdAt,
    ttl,
    hopCount,
    hopLimit,
    priority,
    payloadLength,
    encryptedPayload,
    senderSignature
}
```

---

# 22. Bundle ID

Every Bundle shall have a randomly generated 128-bit Bundle ID.

```text
BundleID = CSPRNG(128 bits)
```

The ID must not depend on:

- timestamp;
- sender identity;
- content;
- recipient identity.

Collision probability is considered negligible.

---

# 23. Bundle Types

Initial types:

```text
0x01 DIRECT_MESSAGE
0x02 DELIVERY_ACK
0x03 READ_ACK
0x04 CONTROL

0x10 RESERVED_ATTACHMENT
0x11 RESERVED_GROUP
0x12 RESERVED_BROADCAST
```

Mesh Protocol 1.0 requires support only for:

```text
DIRECT_MESSAGE
DELIVERY_ACK
```

---

# 24. Destination Identifier

Direct messages include:

```text
destinationID
```

which is the recipient User ID.

Relays may therefore determine that a Bundle is not intended for themselves without decrypting its contents.

This exposes limited routing metadata.

Mesh Protocol 1.0 accepts this privacy trade-off for implementation simplicity.

Future versions may introduce rotating destination tokens.

---

# 25. Message Payload

After decryption, a DIRECT_MESSAGE payload contains:

```text
{
    messageID,
    conversationID,
    senderUserID,
    recipientUserID,
    sequence,
    sentAt,
    contentType,
    content
}
```

For text:

```text
contentType = "text/plain"
```

Example:

```text
content =
"Meet us at the community centre."
```

---

# 26. Message ID

Each logical user message receives its own random 128-bit Message ID.

Normally:

```text
MessageID != BundleID
```

This allows future versions to carry one logical message across several bundles.

---

# 27. End-to-End Encryption

Relay devices must never receive plaintext DIRECT_MESSAGE contents.

Mesh Protocol 1.0 shall use recipient public-key encryption.

Recommended construction:

```text
X25519
HKDF-SHA256
ChaCha20-Poly1305
```

Each message uses a fresh ephemeral X25519 key pair.

---

# 28. Message Encryption

Sender generates:

```text
EphemeralPrivate
EphemeralPublic
```

The sender calculates:

```text
SharedSecret =
X25519(
    EphemeralPrivate,
    RecipientEncryptionPublicKey
)
```

A message encryption key is derived:

```text
MessageKey =
HKDF-SHA256(
    SharedSecret,
    salt,
    context
)
```

The payload is then encrypted using:

```text
ChaCha20-Poly1305
```

The encrypted structure contains:

```text
ephemeralPublicKey
nonce
ciphertext
authenticationTag
```

---

# 29. Sender Authentication

The sender signs the canonical Bundle representation using Ed25519.

The signature covers:

```text
BundleHeader
EncryptedPayload
```

The recipient verifies the signature against the sender's known identity key.

A relay may also verify signatures if the sender public identity is known, but relay verification is not required.

---

# 30. Cryptographic Limitation of Version 1

The Version 1 stateless encryption design prioritises:

- offline operation;
- out-of-order delivery;
- simplicity;
- relay compatibility.

It does not provide full post-compromise security equivalent to protocols using a continuous Double Ratchet.

A future protocol version may add:

```text
prekeys
session ratchets
forward secrecy
post-compromise recovery
```

without changing the Bundle routing architecture.

---

# 31. Bundle Priority

Four priority values are defined:

```text
0 = Bulk
1 = Normal
2 = High
3 = Emergency
```

Recommended default:

```text
Normal
```

Priority influences:

- transmission order;
- storage eviction;
- replication policy.

Priority must not bypass abuse controls.

---

# 32. Bundle TTL

Every Bundle contains:

```text
ttlSeconds
```

Recommended defaults:

```text
Normal message:
259200 seconds
= 72 hours

Emergency:
86400 seconds
= 24 hours

Delivery receipt:
604800 seconds
= 7 days
```

Applications may expose configurable expiry periods.

---

# 33. Clock Handling

Mobile clocks cannot be assumed accurate.

Each device therefore stores:

```text
senderCreatedAt
localFirstSeenAt
```

Expiry decisions should use local elapsed time whenever possible.

A relay must not discard a message solely because the sender's wall-clock timestamp differs moderately from its own.

Implementations should tolerate clock errors of at least:

```text
±24 hours
```

---

# 34. Hop Count

Every Bundle contains:

```text
hopCount
hopLimit
```

The sender initially sets:

```text
hopCount = 0
```

Each relay increments:

```text
hopCount += 1
```

If:

```text
hopCount >= hopLimit
```

the Bundle shall not be forwarded further.

Recommended initial value:

```text
hopLimit = 20
```

---

# 35. Important Bundle Integrity Rule

Mutable relay information such as hop count must not invalidate the sender's end-to-end signature.

Therefore the Bundle contains two header regions:

```text
ImmutableHeader
RelayHeader
```

The signature covers only:

```text
ImmutableHeader
EncryptedPayload
```

The RelayHeader may contain:

```text
hopCount
lastRelayTime
```

---

# 36. Immutable Header

Recommended structure:

```text
protocolVersion
bundleID
bundleType
senderID
destinationID
createdAt
ttlSeconds
hopLimit
priority
payloadLength
```

---

# 37. Relay Header

Recommended structure:

```text
hopCount
receivedAt
```

Relay metadata must never be trusted as sender-authenticated content.

---

# 38. Local Bundle State

Each device maintains:

```text
BundleID
State
FirstSeen
LastForwarded
ForwardCount
PeersForwardedTo
Expiry
```

Possible states:

```text
QUEUED
RELAYING
DELIVERED_LOCAL
ACKNOWLEDGED
EXPIRED
EVICTED
```

---

# 39. Peer Synchronisation

After session establishment:

```text
A                      B

INVENTORY  -------->

           <--------   INVENTORY

OFFER      -------->

           <--------   REQUEST

DATA       -------->

           <--------   COMPLETE
```

Each node determines which Bundles may be useful to the other.

---

# 40. Inventory Exchange

For very small queues, implementations may send explicit Bundle IDs.

Example:

```text
[
  BundleA,
  BundleB,
  BundleC
]
```

For larger stores, the protocol supports compact inventory summaries.

Recommended:

```text
Bloom filter
```

An inventory summary may include:

```text
filterAlgorithm
filterSize
hashCount
itemCount
filterData
```

---

# 41. False Positives

Bloom filters may incorrectly indicate that a peer already owns a Bundle.

This can delay delivery.

Therefore implementations should periodically perform a more precise inventory reconciliation for:

```text
High priority
Emergency
Destination-local bundles
Long-undelivered bundles
```

---

# 42. Bundle Offer

A node shall not automatically transmit every stored Bundle.

It first sends a BUNDLE_OFFER.

Example:

```text
{
    bundleID,
    bundleType,
    destinationID,
    size,
    priority,
    ttlRemaining,
    hopCount
}
```

The receiving node decides whether to request it.

---

# 43. Bundle Acceptance

A device may reject a Bundle because:

```text
Already owned
Expired
Hop limit reached
Storage full
Bundle too large
Origin rate limited
Policy restriction
Unsupported type
```

---

# 44. Bundle Transfer

Requested Bundles are transferred using:

```text
BUNDLE_DATA
```

Large Bundles may be segmented.

Each segment contains:

```text
bundleID
offset
length
data
```

---

# 45. Interrupted Transfers

The receiving node tracks contiguous bytes already received.

If the encounter is interrupted, a later encounter may request:

```text
bundleID
offset
```

and resume transmission.

Partial Bundle data shall not become eligible for forwarding until the complete Bundle has been verified.

---

# 46. Bundle Verification

Upon completing a transfer, the receiver checks:

1. Length.
2. CBOR validity.
3. Bundle ID format.
4. TTL.
5. Hop limit.
6. Cryptographic structure.
7. Signature if possible.
8. Duplicate state.

Only then may the Bundle enter the relay queue.

---

# 47. Direct Delivery

When:

```text
bundle.destinationID == localUserID
```

the device attempts end-to-end decryption.

If successful:

1. Verify sender signature.
2. Verify embedded sender ID.
3. Check replay state.
4. Store the message.
5. Display the message.
6. Create DELIVERY_ACK.

---

# 48. Delivery Acknowledgement

A DELIVERY_ACK is itself a Bundle.

Its encrypted payload contains:

```text
originalBundleID
originalMessageID
recipientID
deliveredAt
status
```

Possible status:

```text
DELIVERED
```

The recipient cryptographically signs the acknowledgement.

---

# 49. ACK Routing

The ACK destination is the original sender.

It propagates through the mesh exactly like another direct Bundle.

Example:

```text
Alice → Bob → Charlie → David

David ACK
   ↓
Charlie → Bob → Alice
```

The return route does not need to match the forward route.

---

# 50. ACK Processing

When the sender receives a valid DELIVERY_ACK:

```text
Message State = DELIVERED
```

The UI may display:

```text
Delivered
```

The acknowledgement can additionally act as a **tombstone** informing relays that continued replication of the original Bundle is unnecessary.

---

# 51. Tombstone Records

A relay receiving evidence that Bundle X has reached its destination may create:

```text
Tombstone(X)
```

The tombstone prevents future acceptance of Bundle X.

Recommended tombstone retention:

```text
7 days
```

Tombstones should be compact.

---

# 52. Deduplication

Every node shall maintain a recent Bundle ID index.

Upon receiving:

```text
BundleID = X
```

if X already exists:

```text
discard duplicate
```

No user-visible duplicate message shall appear.

---

# 53. Replay Protection

Delivered logical Message IDs shall also be stored.

If a malicious node creates a new Bundle containing an old valid encrypted message, the destination must reject it as a replay.

Therefore deduplication occurs at:

```text
BundleID level
MessageID level
```

---

# 54. Initial Routing Algorithm

Mesh Protocol 1.0 uses:

**Controlled Epidemic Routing**

When nodes encounter one another they exchange useful Bundles that the other node does not possess.

This favours reliability in sparse or unpredictable networks.

---

# 55. Basic Forwarding Rule

Node A should offer Bundle X to Node B when:

```text
X is valid

AND

X is not expired

AND

X hopCount < hopLimit

AND

B probably does not have X

AND

A has not exceeded replication policy
```

---

# 56. Destination Priority

If Node B is the destination of Bundle X:

```text
X must receive highest transfer priority.
```

Subject only to basic security and integrity checks.

---

# 57. Forwarding Order

Recommended queue order:

```text
1. Bundles destined for current peer
2. Delivery acknowledgements
3. Emergency bundles
4. High-priority bundles
5. Normal bundles
6. Bulk bundles
```

Within a category:

```text
oldest useful Bundle first
```

is recommended initially.

---

# 58. Replication Control

Unrestricted epidemic routing can overwhelm the network.

Each node therefore tracks:

```text
forwardCount
```

Recommended initial replication targets:

```text
Bulk       2
Normal     6
High       12
Emergency  20
```

These values are implementation defaults rather than protocol constants.

---

# 59. Encounter History

A node may locally record anonymous encounter statistics.

Example:

```text
PeerDiscoveryID
FirstSeen
LastSeen
EncounterCount
AverageConnectionDuration
```

This information may support smarter routing later.

It shall not be required for Protocol 1.0 forwarding.

---

# 60. Storage Quota

Every application maintains a Relay Store quota.

Recommended default:

```text
250 MB
```

The user's own messages shall use a separate logical storage allocation.

Relay data must never consume all available device storage.

---

# 61. Eviction Policy

When relay storage is full, the recommended removal order is:

```text
1. Expired Bundles
2. Acknowledged Bundles
3. Bulk Bundles
4. Highly replicated Bundles
5. Old Normal Bundles
6. High-priority Bundles
7. Emergency Bundles
```

Bundles destined for the local user must not be evicted merely because relay storage is full.

---

# 62. Maximum Bundle Size

Version 1 recommended limits:

```text
Text Bundle:
64 KB maximum

Generic Bundle:
1 MB maximum
```

Larger content shall eventually use an attachment/chunk protocol.

A node may advertise a lower supported maximum.

---

# 63. Rate Limiting

Each node shall enforce local rate limits.

Suggested initial limits per apparent origin:

```text
Normal:
60 Bundles/hour

High:
30 Bundles/hour

Emergency:
10 Bundles/hour
```

Exact values are implementation policy.

No peer is obliged to relay arbitrary volumes of traffic.

---

# 64. Emergency Abuse Protection

The Emergency flag shall not grant unlimited resources.

Nodes may track:

```text
Emergency bundles per sender
Emergency bundles per encounter
Emergency storage consumption
```

A device may demote excessive emergency traffic.

---

# 65. Unknown Senders

Mesh relay must work even when the relay does not know the sender.

Therefore:

```text
Unknown sender != reject
```

However receiving applications may distinguish:

```text
Verified contact
Known identity
Unknown identity
Blocked identity
```

Only the destination needs to decide whether to display a message from an unknown sender.

---

# 66. Blocked Users

A local user may block another User ID.

The application should:

- refuse to display future messages from that ID;
- optionally refuse to relay Bundles originating from that identity where identifiable.

Blocking is local policy and is not broadcast to the network.

---

# 67. Session Keepalive

Long-running connections may exchange:

```text
PING
PONG
```

Recommended idle ping interval:

```text
15 seconds
```

Transports providing their own reliable liveness mechanism may disable Mesh PING/PONG.

---

# 68. Connection Scheduling

Mesh synchronisation should assume encounters can end unexpectedly.

Therefore the highest-value transfers occur first.

Recommended sequence:

```text
1. Session establishment
2. Destination-local traffic
3. ACK traffic
4. Emergency traffic
5. Inventory exchange
6. Remaining relay traffic
```

This allows even a 2–3 second connection to be useful.

---

# 69. Capability Flags

HELLO may advertise:

```text
TEXT
ATTACHMENTS
WIFI_AWARE
BLE
BACKGROUND_RELAY
ACK_TOMBSTONES
BLOOM_INVENTORY
RESUME_TRANSFER
```

Unknown capability bits shall be ignored.

---

# 70. Error Codes

Initial errors:

```text
0x0001 UNSUPPORTED_VERSION
0x0002 MALFORMED_FRAME
0x0003 INVALID_BUNDLE
0x0004 BUNDLE_TOO_LARGE
0x0005 STORAGE_FULL
0x0006 RATE_LIMITED
0x0007 UNSUPPORTED_TYPE
0x0008 SESSION_ERROR
0x0009 CRYPTO_ERROR
```

Errors should generally terminate only the affected operation.

Severe framing or cryptographic session errors should terminate the session.

---

# 71. Security Requirements

Implementations must use a cryptographically secure random number generator for:

```text
Node IDs
Bundle IDs
Message IDs
nonces
ephemeral keys
session nonces
```

Application-level pseudorandom generators are insufficient.

---

# 72. Private Key Storage

Long-term private keys shall use platform-secure facilities where available.

Examples include:

```text
Android Keystore
Apple Keychain / Secure Enclave-backed facilities
```

Private identity keys shall never be included in:

```text
mesh advertisements
Bundles
logs
crash reports
analytics
```

---

# 73. Logging

Production logging shall not contain:

```text
plaintext messages
private keys
message encryption keys
contact encryption secrets
full decrypted payloads
```

Diagnostic logging should use abbreviated identifiers.

Example:

```text
Bundle 7A21…D992 relayed
```

---

# 74. Metadata Exposure

Protocol 1.0 does not hide all metadata.

A relay may potentially observe:

```text
Bundle ID
destination ID
approximate Bundle size
priority
TTL
timing
hop count
```

It cannot read the encrypted user message.

Stronger metadata privacy may be introduced in later protocol versions.

---

# 75. Threat Model

Protocol 1.0 assumes attackers may:

- passively monitor radio traffic;
- run modified Mesh clients;
- create arbitrary identities;
- replay Bundles;
- duplicate Bundles;
- modify Bundles;
- flood peers;
- deliberately refuse forwarding;
- selectively forward messages;
- lie about inventory;
- attempt resource exhaustion.

The protocol does not assume relay nodes are trustworthy.

---

# 76. Malicious Relays

No routing protocol can force a malicious relay to forward messages.

Therefore availability comes primarily from:

```text
redundant paths
multiple relays
replication
```

rather than trust in any individual intermediary.

---

# 77. Sybil Attacks

Because Mesh Protocol requires no central identity authority, an attacker may create many identities.

Version 1 limits the effect primarily through:

```text
rate limits
storage quotas
priority limits
per-peer connection limits
```

Strong Sybil resistance is outside the initial protocol scope.

---

# 78. Protocol Extensibility

CBOR maps should use numeric field identifiers.

Unknown optional fields:

```text
MUST be ignored
```

Unknown mandatory fields:

```text
MUST cause operation rejection
```

Future features must not require old nodes to reinterpret existing field meanings.

---

# 79. Suggested Numeric Bundle Fields

Example canonical mapping:

```text
1  version
2  bundleID
3  bundleType
4  senderID
5  destinationID
6  createdAt
7  ttlSeconds
8  hopLimit
9  priority
10 payloadLength
11 encryptedPayload
12 signature
```

Relay envelope:

```text
20 hopCount
21 receivedAt
```

---

# 80. Suggested DIRECT_MESSAGE Payload Fields

After decryption:

```text
1 version
2 messageID
3 conversationID
4 senderID
5 recipientID
6 sequence
7 sentAt
8 contentType
9 content
```

---

# 81. Conversation ID

For a two-party conversation, a deterministic Conversation ID may be generated from both User IDs.

For example:

```text
ConversationID =
SHA-256(
    MIN(UserA, UserB) ||
    MAX(UserA, UserB)
)[0..15]
```

This allows both devices to independently calculate the same identifier.

---

# 82. Message Sequence

A sender may maintain a monotonically increasing sequence number for each conversation.

Sequence numbers assist with:

- ordering;
- replay detection;
- missing-message detection.

Sequence numbers must not be treated as proof that every prior message was successfully delivered.

---

# 83. Offline Bootstrap

A new installation requires no server interaction.

Startup procedure:

```text
Generate identity keys

Generate encryption keys

Generate User ID

Generate Node ID

Create local encrypted database

Begin discovery
```

The application is immediately capable of communicating.

---

# 84. No Global Directory

Protocol 1.0 has no concept of:

```text
username search
telephone number lookup
central contact directory
global presence service
```

Users must exchange identities through an independent trust mechanism.

---

# 85. Recommended MVP Test

Use three physical phones:

```text
A
B
C
```

A knows C's public identity.

B does not need to know either user personally.

Step 1:

```text
A creates:
"Hello C"
```

C is absent.

Step 2:

```text
A encounters B.
```

A transfers the encrypted Bundle to B.

Step 3:

A is switched off.

Step 4:

```text
B encounters C.
```

B transfers the Bundle.

Step 5:

C decrypts:

```text
Hello C
```

Step 6:

C creates a DELIVERY_ACK.

Step 7:

At some later point the ACK propagates back to A.

Success requires:

```text
B never possesses plaintext.
```

---

# 86. Five-Node Acceptance Test

A more complete acceptance test:

```text
A → B → C → D → E
```

Conditions:

- Internet disabled.
- Cellular data disabled.
- No common Wi-Fi infrastructure.
- Nodes introduced sequentially.
- No A↔E direct contact.
- Intermediate nodes cannot decrypt content.

A sends:

```text
TEST MESSAGE 001
```

Expected result:

```text
E receives exactly one copy.
```

Later:

```text
A receives DELIVERY_ACK.
```

---

# 87. Reference Routing Pseudocode

Conceptual forwarding logic:

```text
for bundle in localRelayStore:

    if bundle.expired:
        delete(bundle)
        continue

    if bundle.hopCount >= bundle.hopLimit:
        continue

    if peer.has(bundle.bundleID):
        continue

    if peer.userID == bundle.destinationID:
        priority = IMMEDIATE

    else:
        priority = calculatePriority(bundle)

    if replicationPolicyAllows(bundle, peer):
        offer(bundle, peer)
```

---

# 88. Reference Receive Pseudocode

```text
onBundleReceived(bundle):

    if malformed(bundle):
        reject()

    if alreadyHave(bundle.bundleID):
        rejectDuplicate()

    if expired(bundle):
        reject()

    if bundle.hopCount >= bundle.hopLimit:
        reject()

    verifyStructure(bundle)

    store(bundle)

    if bundle.destinationID == localUserID:

        plaintext = decrypt(bundle)

        verifySenderSignature(bundle)

        if alreadyHaveMessage(plaintext.messageID):
            discardReplay()

        else:
            storeMessage(plaintext)
            createDeliveryAck(bundle)

    else:

        bundle.hopCount += 1

        queueForRelay(bundle)
```

---

# 89. Transport Independence Example

The same Bundle may travel:

```text
A
 |
 | Bluetooth
 v
B
 |
 | Wi-Fi Aware
 v
C
 |
 | Nearby Connection
 v
D
 |
 | Local Wi-Fi
 v
E
```

None of the routing or cryptographic layers needs to know which transport was used at a previous hop.

---

# 90. Future Routing Improvements

Protocol 1.x may introduce routing hints without changing Bundle semantics.

Candidates include:

```text
Spray and Wait
PRoPHET-style encounter prediction
Community routing
Geographic hints
Relay reputation
Infrastructure relay preference
```

Nodes may advertise supported routing extensions.

---

# 91. Future Message Types

Potential later Bundle types:

```text
GROUP_MESSAGE
VOICE_NOTE
ATTACHMENT_MANIFEST
ATTACHMENT_CHUNK
LOCATION
BROADCAST
EMERGENCY_ALERT
CONTACT_CARD
ROUTE_HINT
```

---

# 92. Future Gateway Extension

A node with external connectivity may eventually act as a gateway.

Example:

```text
Local Mesh
    |
Gateway A
    |
Internet / Satellite
    |
Gateway B
    |
Remote Mesh
```

Gateway operation must remain optional.

A Mesh network must never require gateway availability.

---

# 93. Future Fixed Relay Support

The protocol should also be usable by:

```text
Raspberry Pi
embedded Linux device
community relay
solar relay station
vehicle relay
satellite gateway
```

Such devices participate using the same:

```text
HELLO
session
inventory
Bundle
ACK
```

mechanisms as mobile phones.

---

# 94. Protocol Philosophy

Mesh Protocol does not attempt to make every device maintain a continuous connection to every other device.

Instead:

> The message persists even when the network does not.

A telephone is simultaneously:

```text
user device
temporary router
Bundle store
physical carrier
```

Human movement becomes part of the transport network.

---

# 95. Mesh Protocol 1.0 Success Definition

Protocol 1.0 shall be considered technically proven when:

> A cryptographically authenticated text message can originate on one offline mobile device, travel across at least three independent relay devices using store-carry-forward routing, reach a destination device that was never simultaneously connected to the sender, and return a delivery acknowledgement through an independently formed relay path.

During this process:

- no Internet connection is required;
- no server is contacted;
- no relay can decrypt the message;
- duplicate messages are suppressed;
- expired traffic disappears automatically;
- interrupted encounters do not corrupt the network.

That constitutes the minimum viable **Mesh Protocol 1.0**.