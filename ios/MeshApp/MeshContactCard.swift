import Foundation

/// Explicit contact payload for paste and QR. Not advertised on Bonjour.
struct MeshContactCard: Equatable {
    static let versionToken = "mesh:1"

    var signingPublic: Data
    var encryptionPublic: Data
    var displayName: String?

    init(signingPublic: Data, encryptionPublic: Data, displayName: String? = nil) {
        self.signingPublic = signingPublic
        self.encryptionPublic = encryptionPublic
        self.displayName = displayName
    }

    init(identity: MeshPublicIdentity, displayName: String? = nil) {
        self.init(
            signingPublic: identity.signingPublic,
            encryptionPublic: identity.encryptionPublic,
            displayName: displayName
        )
    }

    var encoded: String {
        var lines = [
            Self.versionToken,
            "s:\(signingPublic.hexString)",
            "e:\(encryptionPublic.hexString)",
        ]
        if let displayName, !displayName.isEmpty {
            lines.append("n:\(displayName)")
        }
        return lines.joined(separator: "\n")
    }

    init?(blob: String) {
        let lines = blob
            .replacingOccurrences(of: "\r\n", with: "\n")
            .split(separator: "\n", omittingEmptySubsequences: true)
            .map { $0.trimmingCharacters(in: .whitespaces) }
        guard lines.contains(where: { $0 == Self.versionToken || $0.hasPrefix("\(Self.versionToken)") }) else {
            return nil
        }
        var signing: Data?
        var encryption: Data?
        var name: String?
        for line in lines {
            if line.hasPrefix("s:") {
                signing = Data(hex: String(line.dropFirst(2)))
            } else if line.hasPrefix("e:") {
                encryption = Data(hex: String(line.dropFirst(2)))
            } else if line.hasPrefix("n:") {
                let value = String(line.dropFirst(2)).trimmingCharacters(in: .whitespaces)
                if !value.isEmpty {
                    name = value
                }
            }
        }
        guard let signing, let encryption, signing.count == 32, encryption.count == 32 else {
            return nil
        }
        self.signingPublic = signing
        self.encryptionPublic = encryption
        self.displayName = name
    }
}
