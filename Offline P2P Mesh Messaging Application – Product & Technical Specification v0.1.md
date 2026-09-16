# Offline P2P Mesh Messaging Application

**Version:** 0.1  
**Status:** Initial specification  
**Primary platforms:** iOS and Android

## 1. Purpose

The application provides communication between mobile phones when normal Internet and cellular data services are unavailable, unreliable, deliberately disabled, censored or compromised.

The system shall not require:

- an Internet connection;
- a mobile data connection;
- a central messaging server;
- DNS;
- cloud authentication;
- functioning telecommunications infrastructure.

Participating phones shall discover nearby participating phones and exchange encrypted data directly.

Phones shall also act as relays for other users, allowing messages to propagate across a population of devices.

The initial application will provide text messaging. The underlying network shall be designed to support images, files, voice messages and potentially live voice communication in future versions.

---

# 2. Fundamental Concept

The application is not simply a Bluetooth chat application.

Every participating telephone becomes a **node in a decentralised delay-tolerant network**.

For example:

**Phone A → Phone B → Phone C → Phone D**

If A wishes to send a message to D but D is not nearby, A may give the encrypted message to B.

Later B encounters C and transfers it.

Later C encounters D and delivers it.

Neither B nor C must be capable of reading the message.

This behaviour is called **store, carry and forward**.

A message can therefore travel considerable geographical distances without conventional telecommunications infrastructure, provided there are sufficient participating devices moving between locations.

The network should also relay messages immediately where several nodes are simultaneously connected.

---

# 3. Core Design Principles

The system shall be:

### Infrastructure independent

Core communications must continue to operate with:

- no Internet;
- no DNS;
- no cellular network;
- no Wi-Fi access point;
- no central server.

### Decentralised

No device shall be inherently more important than another.

Every capable device may:

- discover peers;
- accept connections;
- send messages;
- receive messages;
- relay messages.

### Store-and-forward

Messages must survive temporary disconnection.

### End-to-end encrypted

Relay devices must not be able to read private messages.

### Multi-transport

The message-routing layer must not depend on one radio technology.

Transport mechanisms may include:

- Bluetooth LE;
- Bluetooth;
- peer-to-peer Wi-Fi;
- Wi-Fi Aware;
- Nearby Connections;
- local Wi-Fi networks;
- future radio technologies.

### Opportunistic

Connections may last only seconds.

The protocol must make productive use of short encounters between devices.

### Battery aware

The application must balance network availability against battery consumption.

### Server optional

Internet connectivity may improve the application when available, but Internet infrastructure must never be required for its core messaging functions.

---

# 4. Terminology

**Node**  
A device running the application.

**Peer**  
Another node currently reachable over one of the supported transports.

**Bundle**  
An encrypted unit of information being transported through the mesh.

**Relay**  
A device temporarily storing a bundle intended for another user.

**Encounter**  
A period during which two nodes are able to communicate.

**Neighbour**  
A currently reachable node.

**TTL**  
Time-to-live controlling how long a bundle remains eligible for forwarding.

**Hop**  
One device-to-device transfer.

---

# 5. User Identity

The application must not require Internet registration.

On first launch, the application shall generate a cryptographic identity locally.

A user identity shall contain at minimum:

- unique user identifier;
- public identity key;
- private identity key;
- display name chosen by the user;
- optional profile information.

Private keys shall remain on the device and shall be stored using the operating system's secure credential storage.

The public identifier should be derived from or cryptographically associated with the public key rather than being allocated by a server.

Example:

`8D7F-293A-61BC-44F2`

Users may optionally select a human-readable name such as:

`Andy`

Human-readable names are not guaranteed to be globally unique.

---

# 6. Adding Contacts

Users must be able to establish trusted contacts without Internet connectivity.

The initial application shall support contact exchange using:

### QR codes

User A displays an identity QR code.

User B scans it.

The QR code contains sufficient information to establish cryptographic trust.

### Nearby exchange

Two users physically near each other may exchange identities through the P2P connection.

Both devices shall display a verification code or fingerprint.

Example:

`438 921`

The users confirm that both phones display the same code.

### Manual identity fingerprint

Advanced users may compare public-key fingerprints manually.

Future versions may also support NFC contact exchange.

---

# 7. Messaging

Version 1 shall support:

- one-to-one text messages;
- message timestamps;
- delivery states;
- conversation history;
- offline sending;
- automatic relay;
- message expiry.

A user may compose a message regardless of whether the recipient is currently reachable.

Example:

Andy sends:

> Are you safe? We are at the community centre.

The application immediately stores the message locally.

The UI might show:

**Waiting for mesh**

If a suitable peer is encountered:

**Relayed**

When confirmation eventually returns from the recipient:

**Delivered**

A later protocol version may support:

**Read**

---

# 8. Message Architecture

Every message shall receive a globally unique message identifier.

Conceptually a message bundle contains:

```text
protocolVersion
messageID
messageType
senderID
recipientID
createdAt
expiresAt
hopCount
hopLimit
priority
encryptedPayload
signature
```

Relay nodes should have access only to information required to route and manage the bundle.

The actual message body must remain encrypted.

Example encrypted payload:

```text
{
    conversationID,
    sequence,
    text,
    attachments,
    replyTo
}
```

---

# 9. End-to-End Encryption

Private communications shall use end-to-end encryption.

If:

**A → B → C → D**

and D is the recipient, only A and D must be capable of reading the message.

B and C store and forward ciphertext.

Each bundle must also be cryptographically authenticated so that alterations can be detected.

The cryptographic design should use established cryptographic libraries and protocols rather than custom encryption algorithms.

A suitable architecture would use:

- modern asymmetric identity keys;
- ephemeral session keys where possible;
- authenticated encryption;
- digital signatures;
- secure random number generation.

Forward secrecy should be a target for the production cryptographic protocol.

---

# 10. Peer Discovery

Each node periodically searches for participating devices.

Discovery must occur without Internet access.

When two compatible nodes discover each other they establish an authenticated session.

The discovery process should reveal as little permanent identifying information as practical to passive observers.

The application should periodically rotate advertised discovery identifiers.

A discovery identifier therefore should not simply contain the permanent user ID.

---

# 11. Peer Synchronisation

Once two nodes connect, they perform a short synchronisation handshake.

Conceptually:

```text
A discovers B

A <-> B establish secure link

A -> B protocol capabilities
B -> A protocol capabilities

A -> B message inventory
B -> A message inventory

A determines messages useful to B
B determines messages useful to A

Messages transferred

Delivery acknowledgements exchanged

Connection terminates
```

The protocol must avoid transferring bundles that the receiving node already possesses.

For small networks this may initially use lists of message IDs.

For larger networks, compact structures such as Bloom filters or synchronisation summaries should be investigated.

---

# 12. Mesh Routing

Version 1 should favour reliability over sophisticated routing optimisation.

The initial routing algorithm should therefore use **controlled epidemic forwarding**.

When Node A encounters Node B:

1. They compare their message inventories.
2. A offers messages B does not possess.
3. B offers messages A does not possess.
4. Each node accepts eligible messages.
5. Duplicate messages are discarded.

Several controls prevent uncontrolled replication.

Every bundle shall contain:

- expiry time;
- maximum hops;
- priority;
- replication rules.

A message might therefore have:

```text
TTL: 72 hours
Maximum hops: 20
Priority: Normal
```

Once either limit is reached, the relay removes the bundle.

Later versions should investigate routing algorithms such as:

- Spray and Wait;
- encounter-frequency routing;
- destination prediction;
- geographic/community routing;
- adaptive replication.

---

# 13. Delivery Receipts

The recipient generates a signed delivery acknowledgement when a message arrives.

The acknowledgement itself becomes a small mesh bundle.

It travels back through the mesh until it reaches the sender.

Once sufficient delivery confirmation exists, nodes may delete the original message earlier than its normal expiry period.

Acknowledgements must also be deduplicated.

---

# 14. Local Message Database

Every device shall contain an encrypted local database.

It shall maintain at least:

```text
Users
Contacts
Conversations
Messages
Bundles
Peers
EncounterHistory
DeliveryReceipts
RoutingMetadata
Settings
```

The queue of relay messages must be separate logically from the user's personal message history.

A user deleting a conversation should therefore not accidentally corrupt the routing network.

---

# 15. Transport Layer

Routing logic must be independent of physical transport.

Define a transport interface conceptually similar to:

```text
MeshTransport

startDiscovery()
stopDiscovery()

advertise()
stopAdvertising()

connect(peer)

send(peer, data)

disconnect(peer)
```

Transport implementations can then include:

```text
NearbyTransport
BluetoothTransport
WifiAwareTransport
LocalNetworkTransport
ApplePeerTransport
```

The routing engine sees a peer connection rather than needing to understand which radio technology produced it.

---

# 16. Recommended Initial Transport

For the first prototype, implement **Google Nearby Connections** for both Android and iOS.

Nearby Connections can discover nearby devices and transfer arbitrary payloads without Internet connectivity and uses technologies including Bluetooth, BLE and Wi-Fi underneath. It is available on both Android and iOS.

The system architecture must nevertheless treat Nearby Connections as a replaceable transport rather than making it part of the mesh protocol itself.

A later resilience-focused version should add direct platform transports so the application is not permanently dependent upon a single vendor framework.

---

# 17. Platform Architecture

Because this application depends heavily on low-level networking and background radio behaviour, the initial production applications should use substantial native code.

Recommended structure:

```text
                 +-----------------------+
                 |        UI Layer       |
                 +-----------+-----------+
                             |
                 +-----------v-----------+
                 |   Messaging Service   |
                 +-----------+-----------+
                             |
                 +-----------v-----------+
                 |     Mesh Core         |
                 |                       |
                 | Routing               |
                 | Bundle management     |
                 | Deduplication         |
                 | Cryptography          |
                 | Peer state            |
                 +-----------+-----------+
                             |
                 +-----------v-----------+
                 | Transport Abstraction |
                 +-----------+-----------+
                             |
           +-----------------+-----------------+
           |                 |                 |
     +-----v------+    +-----v------+    +-----v------+
     | Nearby     |    | Wi-Fi      |    | Bluetooth  |
     | Connections|    | Aware      |    | / BLE      |
     +------------+    +------------+    +------------+
```

A strong implementation option would be:

**Shared core:** Rust

containing:

- protocol implementation;
- routing;
- bundle database logic;
- cryptography;
- message serialisation.

**Android:** Kotlin

**iOS:** Swift

The shared Rust library could be exposed to both platforms through platform bindings.

This avoids implementing two subtly different mesh protocols.

---

# 18. Android Networking

Android transport support should eventually include:

### Nearby Connections

Primary MVP transport.

### Wi-Fi Aware

Used when supported by the hardware.

Wi-Fi Aware allows compatible Android devices to discover and establish direct connections without an access point.

### Bluetooth LE

Useful for discovery and small amounts of data.

### Wi-Fi P2P

Potential additional high-bandwidth transport.

The application must gracefully detect which facilities exist on each device.

---

# 19. iOS Networking

The iOS networking strategy should target modern Apple networking APIs.

For modern supported devices, Wi-Fi Aware should be investigated as a primary high-speed P2P transport.

Apple also provides peer-to-peer networking through its Network framework.

Apple's proprietary peer-to-peer Wi-Fi cannot be used as the sole cross-platform solution because its over-the-air protocol is Apple-specific.

Bluetooth should therefore remain an important interoperability/fallback mechanism.

Background execution behaviour must be specifically tested across supported iOS versions.

The application must never assume that it can run continuously in the background.

---

# 20. Background Operation

This is one of the most important technical challenges.

The application should attempt to:

- advertise availability;
- discover peers;
- exchange queued messages;
- terminate connections quickly;
- return to low-power operation.

The protocol should be designed around intermittent execution rather than requiring permanently running processes.

The application should synchronise useful information as rapidly as possible whenever the operating system provides an execution opportunity.

If the user actively enters an **Emergency Mesh Mode**, the application may use a more aggressive discovery policy where permitted.

---

# 21. Battery Modes

Users shall be able to select:

### Low Power

Infrequent discovery.

Suitable for prolonged emergencies.

### Balanced

Default operating mode.

### Maximum Mesh

Frequent discovery and aggressive relaying.

Suitable when communication availability is more important than battery life.

The network engine should also automatically respond to battery level.

For example:

```text
Battery > 50%       Normal relay behaviour
Battery 20-50%      Reduced discovery
Battery < 20%       Reduced relay behaviour
Battery < 10%       Messages for owner prioritised
```

Exact thresholds should be determined through testing.

---

# 22. Message Priorities

Bundles should support priority classes.

Suggested initial priorities:

```text
Emergency
High
Normal
Bulk
```

Emergency messages receive preferential:

- storage;
- transmission;
- synchronisation;
- replication.

A malicious user must not be able to gain unlimited network resources simply by marking every message Emergency.

Rate limiting and abuse controls are therefore required.

---

# 23. Network Abuse Protection

A decentralised system is vulnerable to deliberate flooding.

A malicious device could generate millions of messages and consume every participant's storage and battery.

Protection mechanisms shall include:

- maximum bundle size;
- per-origin message quotas;
- rate limiting;
- maximum relay storage;
- TTL enforcement;
- hop limits;
- duplicate detection;
- cryptographic sender identity;
- ability to block identities;
- prioritisation of trusted contacts.

Future versions may implement reputation or proof-of-work mechanisms.

---

# 24. Relay Storage

Users should be able to control how much storage their phone contributes to the network.

Example:

```text
Relay storage:

Low       50 MB
Normal    250 MB
High      1 GB
Unlimited User-defined
```

When storage becomes full, bundles should be removed according to an eviction policy considering:

- expiry;
- priority;
- replication count;
- age;
- probable usefulness.

Private messages belonging to the device owner should not be removed merely to retain third-party relay bundles.

---

# 25. Broadcast Messages

The network may eventually support broadcast channels.

Examples:

```text
Local emergency information
Community announcements
Missing person notices
Medical assistance requests
Official emergency broadcasts
```

Broadcast traffic presents substantially greater abuse and scaling problems than direct messaging.

It should therefore **not** be included in the initial MVP.

When implemented, broadcasts must have:

- strict TTL;
- geographical or network scope where practical;
- rate limits;
- cryptographic origin identity.

An official-authority verification system could eventually allow trusted organisations to sign emergency announcements.

---

# 26. User Interface

The main interface should feel similar to a conventional messaging application.

Primary screens:

### Conversations

Displays existing conversations.

### Conversation

Normal chat interface.

### Contacts

Trusted contacts and identity verification.

### Mesh

Shows basic network condition.

Possible information:

```text
Nearby nodes: 7
Connected nodes: 3
Messages relayed today: 142
Your waiting messages: 2
Mesh status: GOOD
```

The application should not overwhelm ordinary users with routing details.

An advanced diagnostics screen can expose technical information.

---

# 27. Network Visualisation

A later feature may display nearby network activity.

For privacy reasons it should not display permanent identities of unrelated relay nodes.

An example visualisation might show:

```text
You
 |
 +-- Node
 |
 +-- Node
      |
      +-- Node
```

This should represent connectivity rather than identifiable individuals.

---

# 28. Emergency Mode

A prominent **Emergency Mesh Mode** should be considered.

When enabled, subject to OS restrictions, the application should:

- increase peer discovery;
- increase relay participation;
- prioritise emergency messages;
- reduce nonessential activity;
- display network status prominently;
- preserve battery where appropriate.

The screen should clearly state:

**Internet connection is not required.**

---

# 29. Protocol Versioning

The network protocol must support future versions.

Every handshake and bundle shall identify the protocol version.

Example:

```text
Mesh Protocol 1.0
```

Nodes should advertise supported protocol versions and capabilities.

Example:

```text
Protocol: 1.2
Text: YES
Images: YES
Voice Messages: YES
Realtime Voice: NO
WiFiAware: YES
```

Older nodes should continue communicating with newer nodes wherever protocol compatibility permits.

---

# 30. Serialisation

A compact binary protocol should be used for mesh traffic.

Candidates include:

- Protocol Buffers;
- CBOR;
- MessagePack.

JSON may be useful during initial debugging but should not be the final over-the-air representation because efficiency matters when thousands of bundles are being exchanged.

---

# 31. MVP Scope

Version 0.1 should deliberately remain small.

It needs to prove five things:

1. Two phones can discover each other with no Internet connection.
2. They can establish a secure session.
3. Phone A can send a text message to Phone B.
4. A third phone can relay a message between two phones that never directly meet.
5. The system functions across Android and iOS.

The crucial demonstration is:

```text
Alice
  |
  | sends "Hello Charlie"
  v
Bob

Alice disappears.

Bob later encounters Charlie.

Bob
  |
  | forwards encrypted bundle
  v
Charlie

Charlie reads:

"Hello Charlie"
```

Bob must never be able to read the message.

---

# 32. MVP Development Phases

## Phase 1 — Direct P2P

Implement:

- Android application;
- iOS application;
- peer discovery;
- connection establishment;
- text transfer.

No routing.

Goal:

```text
A <----> B
```

---

## Phase 2 — Mesh Relay

Implement:

- bundle storage;
- unique message IDs;
- TTL;
- hop counter;
- duplicate detection;
- forwarding.

Goal:

```text
A ----> B ----> C
```

A and C never directly encounter one another.

---

## Phase 3 — Cryptographic Identity

Implement:

- permanent user keys;
- contact exchange;
- QR verification;
- end-to-end encryption;
- signatures.

---

## Phase 4 — Delivery System

Implement:

- acknowledgements;
- delivery receipts;
- expiry;
- queue management;
- conversation state.

---

## Phase 5 — Resilience

Implement:

- background operation;
- battery modes;
- multiple simultaneous peers;
- connection interruption recovery;
- storage limits;
- routing limits.

---

## Phase 6 — Large Mesh Testing

Test populations of:

```text
10 nodes
50 nodes
100 nodes
500+ simulated nodes
```

Physical testing can be combined with network simulation.

---

# 33. Required Test Scenarios

The system must be tested under genuine infrastructure failure conditions.

Test devices should have:

```text
Mobile data OFF
Wi-Fi access points unavailable
Internet unavailable
Airplane mode with permitted radios manually re-enabled
```

Important tests include:

### Direct delivery

A and B remain together.

### Delayed delivery

A sends while B is absent.

They later meet.

### Single relay

A → B → C.

### Multiple relay

A → B → C → D → E.

### Network partition

Two groups operate independently and later encounter one another.

### Duplicate paths

A message reaches the destination through several relay paths.

Only one logical message appears.

### Node disappearance

A relay phone is switched off during transmission.

### Malicious duplication

Thousands of copies of the same bundle are introduced.

### Clock errors

Phones have incorrect system times.

### Low battery

Nodes restrict relay activity.

### Storage exhaustion

Relay storage reaches its configured limit.

---

# 34. Security Threat Model

Assume an attacker may:

- listen to radio traffic;
- operate participating nodes;
- capture relay messages;
- alter bundles;
- replay old bundles;
- create many identities;
- deliberately flood the network;
- impersonate users;
- attempt traffic analysis;
- physically obtain a phone.

The system should therefore provide:

- confidentiality;
- message integrity;
- sender authentication;
- replay protection;
- duplicate suppression;
- key verification;
- local database protection;
- traffic minimisation where possible.

The system cannot guarantee protection against all traffic-analysis attacks because radio transmissions themselves reveal that communication is occurring.

---

# 35. Important Limitations

The application must clearly communicate its physical limitations.

This system cannot magically provide worldwide communication merely because the application is installed.

Communication requires participating devices to form a physical chain.

For example:

```text
Town A  <---- people/devices moving ----> Town B
```

If no participating device ever crosses a network gap, a message cannot cross that gap either.

Likewise, radio conditions, operating-system restrictions and battery state affect network performance.

The design should therefore favour eventual delivery rather than guaranteeing immediate delivery.

---

# 36. Future Features

Once the core mesh protocol is proven, the architecture should be capable of supporting:

- group messaging;
- photographs;
- document transfer;
- voice notes;
- emergency broadcasts;
- location sharing;
- maps;
- missing-person notices;
- community message boards;
- realtime audio;
- video;
- gateways to surviving Internet connections;
- dedicated solar-powered mesh relay nodes;
- Raspberry Pi/community relay stations;
- satellite gateway nodes;
- LoRa or other long-range radio gateways.

These should use the same underlying bundle-routing architecture wherever appropriate.

---

# 37. Infrastructure Gateways

An important future concept is a **gateway node**.

Suppose 500 phones have no Internet access but one device occasionally obtains:

- Starlink;
- satellite data;
- functioning broadband;
- functioning cellular data.

That device could optionally bridge compatible mesh traffic to another distant mesh.

Conceptually:

```text
LOCAL MESH
   |
   |
Gateway
   |
Satellite / Internet
   |
Gateway
   |
REMOTE MESH
```

The mesh itself still works without that gateway.

The gateway simply extends its reach.

---

# 38. Dedicated Relay Nodes

A future companion device could provide permanent mesh coverage.

For example:

```text
Solar panel
    |
Battery
    |
Raspberry Pi / embedded computer
    |
Wi-Fi + Bluetooth
```

Placed at:

- community centres;
- hospitals;
- shelters;
- hilltops;
- transport hubs.

These devices could store substantially larger message queues than phones and operate continuously.

The mobile protocol should therefore be documented and open enough for non-phone implementations.

---

# 39. Protocol Openness

The mesh protocol should ultimately be publicly documented.

The design should avoid requiring a proprietary server.

An open protocol would allow third parties to create:

- desktop nodes;
- hardware relays;
- emergency-service gateways;
- embedded devices;
- alternative mobile clients.

Interoperability becomes especially valuable if the system is intended for emergency communications.

---

# 40. Primary Success Criterion

The defining test for the project is simple:

> Five phones are placed in different locations. There is no Internet, no functioning cellular network and no shared Wi-Fi infrastructure. A message originating on Phone A eventually reaches Phone E by travelling through participating phones B, C and D, and none of those relay phones can decrypt the message.

If that works reliably, the fundamental architecture has succeeded.