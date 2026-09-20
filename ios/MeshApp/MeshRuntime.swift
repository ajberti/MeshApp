import Foundation
import SwiftUI
import UIKit

final class MeshRuntime: ObservableObject {
    @Published var fingerprint = ""
    @Published var discoveryIdHex = ""
    @Published var contactCard = ""
    @Published var qrImage: UIImage?
    @Published var pasteText = ""
    @Published var logs: [String] = []
    @Published var messages: [MeshMessage] = []
    @Published var contacts: [MeshContact] = []
    @Published var conversations: [MeshConversation] = []
    @Published var peers: [NetworkMeshTransport.Peer] = []
    @Published var errorText: String?
    @Published var showScanner = false
    @Published var showAddContact = false

    private let engine: MeshEngine
    private let transport = NetworkMeshTransport()
    private let work = DispatchQueue(label: "mesh.runtime")
    private var localSigningPublic = Data()

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
        discoveryIdHex = status.discoveryId.hexString
        localSigningPublic = identity.signingPublic
        contactCard = MeshContactCard(identity: identity).encoded
        qrImage = ContactQRCode.image(from: contactCard)
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
        transport.start(discoveryId: status.discoveryId)
        refreshInbox()
    }

    func copyContactCard() {
        UIPasteboard.general.string = contactCard
        append("copied contact card")
    }

    func importPastedContact() {
        importContactCard(pasteText)
    }

    func importContactCard(_ blob: String) {
        errorText = nil
        guard let card = MeshContactCard(blob: blob) else {
            errorText = "Not a mesh:1 contact card."
            return
        }
        if card.signingPublic == localSigningPublic {
            errorText = "That is this device's identity."
            return
        }
        let name = card.displayName ?? String(card.signingPublic.hexString.prefix(12))
        work.async { [weak self] in
            guard let self else { return }
            do {
                try self.engine.addContact(
                    signingPublic: card.signingPublic,
                    encryptionPublic: card.encryptionPublic,
                    displayName: name
                )
                Task { @MainActor in
                    self.pasteText = ""
                    self.showScanner = false
                    self.refreshInbox()
                    self.append("added contact \(name)")
                }
            } catch {
                Task { @MainActor in self.errorText = error.localizedDescription }
            }
        }
    }

    func send(text: String, to contact: MeshContact) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        errorText = nil
        work.async { [weak self] in
            guard let self else { return }
            do {
                let actions = try self.engine.sendText(recipient: contact.userId, text: trimmed)
                self.apply(actions)
                Task { @MainActor in self.refreshInbox() }
            } catch {
                Task { @MainActor in self.errorText = error.localizedDescription }
            }
        }
    }

    func messages(for contact: MeshContact) -> [MeshMessage] {
        messages.filter { message in
            message.senderId == contact.userId || message.recipientId == contact.userId
        }
    }

    func preview(for contact: MeshContact) -> String {
        messages(for: contact).last?.text ?? "No messages yet"
    }

    var inboxContacts: [MeshContact] {
        contacts.sorted { lhs, rhs in
            let left = conversations.first(where: { $0.remoteUserId == lhs.userId })?.lastMessageAtMs ?? 0
            let right = conversations.first(where: { $0.remoteUserId == rhs.userId })?.lastMessageAtMs ?? 0
            if left != right {
                return left > right
            }
            return lhs.title.localizedCaseInsensitiveCompare(rhs.title) == .orderedAscending
        }
    }

    func clearError() {
        errorText = nil
    }

    private func upsert(_ peer: NetworkMeshTransport.Peer) {
        if let index = peers.firstIndex(where: { $0.token == peer.token }) {
            peers[index] = peer
        } else {
            peers.append(peer)
        }
    }

    private func handle(event: MeshEvent) {
        do {
            apply(try engine.processEvent(event: event))
            Task { @MainActor in self.refreshInbox() }
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
                Task { @MainActor in self.refreshInbox() }
            case let .bundleStored(bundleId):
                Task { @MainActor in self.append("stored \(bundleId.hexString.prefix(8))…") }
            default:
                break
            }
        }
    }

    private func refreshInbox() {
        contacts = (try? engine.getContacts()) ?? []
        conversations = (try? engine.getConversations()) ?? []
        messages = (try? engine.getMessages(conversationId: nil)) ?? []
    }

    private func append(_ line: String) {
        logs.append(line)
        if logs.count > 80 {
            logs.removeFirst(logs.count - 80)
        }
    }
}
