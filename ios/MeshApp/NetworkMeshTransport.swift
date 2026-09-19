import Foundation
import Network

/// Bonjour/TCP MeshTransport. Nearby Connections can replace this later.
final class NetworkMeshTransport {
    static let serviceType = "_mesh._tcp"

    struct Peer {
        var token: Data
        var endpoint: NWEndpoint
        var discoveryId: Data
        var signingPublic: Data?
        var encryptionPublic: Data?
        var userId: Data?
        var displayName: String
    }

    var onEvent: ((MeshEvent) -> Void)?
    var onPeer: ((Peer) -> Void)?
    var onLog: ((String) -> Void)?

    private let queue = DispatchQueue(label: "mesh.transport")
    private var listener: NWListener?
    private var browser: NWBrowser?
    private var connections: [UInt64: Connection] = [:]
    private var endpoints: [Data: NWEndpoint] = [:]
    private var advertised: Set<Data> = []
    private var nextLinkId: UInt64 = 1
    private let instanceName = UUID().uuidString

    func start(discoveryId: Data, identity: MeshPublicIdentity) {
        queue.async { [weak self] in
            self?.startLocked(discoveryId: discoveryId, identity: identity)
        }
    }

    func stop() {
        queue.async { [weak self] in
            guard let self else { return }
            self.browser?.cancel()
            self.listener?.cancel()
            self.connections.values.forEach { $0.connection.cancel() }
            self.connections.removeAll()
            self.endpoints.removeAll()
        }
    }

    func connect(peerToken: Data) {
        queue.async { [weak self] in
            guard let self else { return }
            guard let endpoint = self.endpoints[peerToken] else {
                self.onLog?("connect: unknown peer")
                return
            }
            self.open(endpoint: endpoint, peerToken: peerToken)
        }
    }

    func send(linkId: UInt64, data: Data) {
        queue.async { [weak self] in
            guard let connection = self?.connections[linkId] else { return }
            connection.send(payload: data)
        }
    }

    func disconnect(linkId: UInt64) {
        queue.async { [weak self] in
            self?.connections[linkId]?.connection.cancel()
            self?.connections.removeValue(forKey: linkId)
        }
    }

    private func startLocked(discoveryId: Data, identity: MeshPublicIdentity) {
        let txt = NWTXTRecord([
            "d": discoveryId.hexString,
            "s": identity.signingPublic.hexString,
            "e": identity.encryptionPublic.hexString,
            "u": identity.userId.hexString,
            "n": String(identity.fingerprint.prefix(12)),
        ])

        let parameters = Self.peerParameters()
        do {
            let listener = try NWListener(using: parameters)
            listener.service = NWListener.Service(
                name: instanceName,
                type: Self.serviceType,
                txtRecord: txt
            )
            listener.stateUpdateHandler = { [weak self] state in
                self?.onLog?("listener \(state)")
            }
            listener.newConnectionHandler = { [weak self] connection in
                self?.queue.async {
                    self?.attach(connection: connection, peerToken: Data(UUID().uuidString.utf8))
                }
            }
            listener.start(queue: queue)
            self.listener = listener
        } catch {
            onLog?("listener failed: \(error)")
        }

        let browser = NWBrowser(
            for: .bonjourWithTXTRecord(type: Self.serviceType, domain: nil),
            using: parameters
        )
        browser.browseResultsChangedHandler = { [weak self] results, _ in
            self?.queue.async {
                self?.handleBrowser(results: results)
            }
        }
        browser.start(queue: queue)
        self.browser = browser
    }

    private func handleBrowser(results: Set<NWBrowser.Result>) {
        var current: Set<Data> = []
        for result in results {
            guard case let .service(name: name, type: _, domain: _, interface: _) = result.endpoint else {
                continue
            }
            if name == instanceName {
                continue
            }
            let token = Data(name.utf8)
            current.insert(token)
            endpoints[token] = result.endpoint
            var discoveryId = Data()
            var signingPublic: Data?
            var encryptionPublic: Data?
            var userId: Data?
            var displayName = name
            if case let .bonjour(txt) = result.metadata {
                discoveryId = Data(hex: Self.txtString(txt, "d") ?? "") ?? Data()
                signingPublic = Data(hex: Self.txtString(txt, "s") ?? "")
                encryptionPublic = Data(hex: Self.txtString(txt, "e") ?? "")
                userId = Data(hex: Self.txtString(txt, "u") ?? "")
                if let nameHint = Self.txtString(txt, "n"), !nameHint.isEmpty {
                    displayName = nameHint
                }
            }
            onPeer?(
                Peer(
                    token: token,
                    endpoint: result.endpoint,
                    discoveryId: discoveryId,
                    signingPublic: signingPublic,
                    encryptionPublic: encryptionPublic,
                    userId: userId,
                    displayName: displayName
                )
            )
            if !discoveryId.isEmpty {
                onEvent?(.peerDiscovered(peerToken: token, discoveryId: discoveryId))
            }
        }
        for lost in advertised.subtracting(current) {
            endpoints.removeValue(forKey: lost)
            onEvent?(.peerLost(peerToken: lost))
        }
        advertised = current
    }

    private static func txtString(_ record: NWTXTRecord, _ key: String) -> String? {
        guard let value = record[key], !value.isEmpty else {
            return nil
        }
        return value
    }

    private func open(endpoint: NWEndpoint, peerToken: Data) {
        let connection = NWConnection(to: endpoint, using: Self.peerParameters())
        attach(connection: connection, peerToken: peerToken)
    }

    private func attach(connection: NWConnection, peerToken: Data) {
        let linkId = nextLinkId
        nextLinkId += 1
        let wrapped = Connection(linkId: linkId, connection: connection)
        wrapped.onPayload = { [weak self] data in
            self?.onEvent?(.bytesReceived(linkId: linkId, data: data))
        }
        wrapped.onReady = { [weak self] in
            self?.onEvent?(.linkOpened(linkId: linkId, peerToken: peerToken))
        }
        wrapped.onClosed = { [weak self] in
            self?.queue.async {
                self?.connections.removeValue(forKey: linkId)
            }
            self?.onEvent?(.linkClosed(linkId: linkId))
        }
        connections[linkId] = wrapped
        connection.stateUpdateHandler = { [weak wrapped] state in
            switch state {
            case .ready:
                wrapped?.onReady?()
                wrapped?.receiveLoop()
            case .failed, .cancelled:
                wrapped?.onClosed?()
            default:
                break
            }
        }
        connection.start(queue: queue)
    }

    private static func peerParameters() -> NWParameters {
        let parameters = NWParameters.tcp
        parameters.includePeerToPeer = true
        parameters.allowLocalEndpointReuse = true
        return parameters
    }
}

private final class Connection {
    let linkId: UInt64
    let connection: NWConnection
    var buffer = Data()
    var onPayload: ((Data) -> Void)?
    var onReady: (() -> Void)?
    var onClosed: (() -> Void)?

    init(linkId: UInt64, connection: NWConnection) {
        self.linkId = linkId
        self.connection = connection
    }

    func send(payload: Data) {
        var framed = Data()
        var length = UInt32(payload.count).bigEndian
        framed.append(Data(bytes: &length, count: 4))
        framed.append(payload)
        connection.send(content: framed, completion: .contentProcessed { _ in })
    }

    func receiveLoop() {
        connection.receive(minimumIncompleteLength: 1, maximumLength: 64 * 1024) { [weak self] data, _, isComplete, error in
            guard let self else { return }
            if let data, !data.isEmpty {
                self.buffer.append(data)
                self.drain()
            }
            if isComplete || error != nil {
                self.onClosed?()
                return
            }
            self.receiveLoop()
        }
    }

    private func drain() {
        while buffer.count >= 4 {
            let length = buffer.prefix(4).reduce(into: UInt32(0)) { partial, byte in
                partial = (partial << 8) | UInt32(byte)
            }
            if length > 256 * 1024 {
                onClosed?()
                return
            }
            let total = 4 + Int(length)
            guard buffer.count >= total else { return }
            let payload = buffer.subdata(in: 4..<total)
            buffer.removeSubrange(0..<total)
            onPayload?(payload)
        }
    }
}

extension Data {
    var hexString: String {
        map { String(format: "%02x", $0) }.joined()
    }

    init?(hex: String) {
        let cleaned = hex.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !cleaned.isEmpty, cleaned.count.isMultiple(of: 2) else {
            return nil
        }
        var data = Data()
        var index = cleaned.startIndex
        while index < cleaned.endIndex {
            let next = cleaned.index(index, offsetBy: 2)
            guard let byte = UInt8(cleaned[index..<next], radix: 16) else {
                return nil
            }
            data.append(byte)
            index = next
        }
        self = data
    }
}
