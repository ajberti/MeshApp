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
        let tokens = blob.split { $0.isWhitespace }.map(String.init)
        guard tokens.contains(where: { $0 == Self.versionToken || $0.hasPrefix("\(Self.versionToken):") }) else {
            return nil
        }
        var signing: Data?
        var encryption: Data?
        var name: String?
        for token in tokens {
            if token.hasPrefix("s:") {
                signing = Data(hex: String(token.dropFirst(2)))
            } else if token.hasPrefix("e:") {
                encryption = Data(hex: String(token.dropFirst(2)))
            } else if token.hasPrefix("n:") {
                let value = String(token.dropFirst(2))
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
