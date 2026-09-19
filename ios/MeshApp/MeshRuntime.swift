import Foundation
import SwiftUI

final class MeshRuntime: ObservableObject {
    @Published var fingerprint = ""
    @Published var logs: [String] = []
    @Published var messages: [MeshMessage] = []
    @Published var peers: [NetworkMeshTransport.Peer] = []
    @Published var draft = "Are you safe?"
    @Published var errorText: String?

    private let engine: MeshEngine
    private let transport = NetworkMeshTransport()
    private let work = DispatchQueue(label: "mesh.runtime")

    init() throws {
        let secrets: MeshIdentitySecrets
        if let seeds = IdentityKeychain.load() {
            secrets = MeshIdentitySecrets(
                userId: Data(),
                signingPublic: Data(),
                encryptionPublic: Data(),
                fingerprint: "",
                signingSeed: seeds.signingSeed,
                encryptionSeed: seeds.encryptionSeed,
                localDataKey: seeds.localDataKey
            )
        } else {
            secrets = generateIdentity()
            try IdentityKeychain.save(secrets)
        }
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first!
        try FileManager.default.createDirectory(at: support, withIntermediateDirectories: true)
        let dbPath = support.appendingPathComponent("mesh.sqlite").path
        engine = try MeshEngine.open(
            dbPath: dbPath,
            signingSeed: secrets.signingSeed,
            encryptionSeed: secrets.encryptionSeed,
            localDataKey: secrets.localDataKey
        )
        let identity = try engine.getPublicIdentity()
        let status = try engine.getMeshStatus()
        fingerprint = identity.fingerprint
        append("local \(identity.fingerprint.prefix(16))…")

        transport.onLog = { [weak self] line in
            Task { @MainActor in self?.append(line) }
        }
        transport.onPeer = { [weak self] peer in
            Task { @MainActor in self?.upsert(peer) }
        }
        transport.onEvent = { [weak self] event in
            self?.work.async {
                self?.handle(event: event)
            }
        }
        transport.start(discoveryId: status.discoveryId, identity: identity)
        refreshMessages()
    }

    func sendDraft() {
        let text = draft
        let recipient = peers.compactMap(\.userId).first ?? contacts.first?.userId
        guard let recipient else {
            errorText = "No contact yet. Wait for a peer advertisement."
            return
        }
        work.async { [weak self] in
            guard let self else { return }
            do {
                let actions = try self.engine.sendText(recipient: recipient, text: text)
                self.apply(actions)
                Task { @MainActor in self.refreshMessages() }
            } catch {
                Task { @MainActor in self.errorText = error.localizedDescription }
            }
        }
    }

    private var contacts: [MeshContact] {
        (try? engine.getContacts()) ?? []
    }

    private func upsert(_ peer: NetworkMeshTransport.Peer) {
        if let index = peers.firstIndex(where: { $0.token == peer.token }) {
            peers[index] = peer
        } else {
            peers.append(peer)
        }
        if let signing = peer.signingPublic, let encryption = peer.encryptionPublic {
            try? engine.addContact(
                signingPublic: signing,
                encryptionPublic: encryption,
                displayName: peer.displayName
            )
        }
    }

    private func handle(event: MeshEvent) {
        do {
            apply(try engine.processEvent(event: event))
            Task { @MainActor in self.refreshMessages() }
        } catch {
            Task { @MainActor in self.errorText = error.localizedDescription }
        }
    }

    private func apply(_ actions: [MeshAction]) {
        for action in actions {
            switch action {
            case let .connect(peerToken):
                transport.connect(peerToken: peerToken)
            case let .sendBytes(linkId, data):
                transport.send(linkId: linkId, data: data)
            case let .closeLink(linkId):
                transport.disconnect(linkId: linkId)
            case let .log(code):
                Task { @MainActor in self.append(code) }
            case .messageReceived, .messageUpdated:
                Task { @MainActor in self.refreshMessages() }
            case let .bundleStored(bundleId):
                Task { @MainActor in self.append("stored \(bundleId.hexString.prefix(8))…") }
            default:
                break
            }
        }
    }

    private func refreshMessages() {
        messages = (try? engine.getMessages(conversationId: nil)) ?? []
    }

    private func append(_ line: String) {
        logs.append(line)
        if logs.count > 50 {
            logs.removeFirst(logs.count - 50)
        }
    }
}

