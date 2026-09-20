import Foundation

extension String {
    /// First 12 hex chars in groups of four, for display and verification.
    var meshShortFingerprint: String {
        let hex = filter(\.isHexDigit)
        let slice = hex.prefix(12)
        var parts: [String] = []
        var index = slice.startIndex
        while index < slice.endIndex {
            let next = slice.index(index, offsetBy: 4, limitedBy: slice.endIndex) ?? slice.endIndex
            parts.append(String(slice[index..<next]))
            index = next
        }
        return parts.joined(separator: " ")
    }
}

extension MeshContact {
    var title: String {
        let generated = String(signingPublic.hexString.prefix(12))
        if displayName.isEmpty || displayName == generated {
            return fingerprint.meshShortFingerprint
        }
        return displayName
    }
}

extension MeshMessageState {
    var statusLabel: String {
        switch self {
        case .queued: return "Waiting for mesh"
        case .relayed: return "Relayed"
        case .delivered: return "Delivered"
        case .failed: return "Failed"
        }
    }
}
