import Foundation
import Security

enum IdentityKeychain {
    private static let service = "com.meshproject.mesh.dev"
    private static let account = "identity-seeds-v1"

    struct Seeds {
        var signingSeed: Data
        var encryptionSeed: Data
        var localDataKey: Data
    }

    static func load() -> Seeds? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var item: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &item)
        guard status == errSecSuccess, let data = item as? Data, data.count == 96 else {
            return nil
        }
        return Seeds(
            signingSeed: data.prefix(32),
            encryptionSeed: data.subdata(in: 32..<64),
            localDataKey: data.suffix(32)
        )
    }

    static func save(_ secrets: MeshIdentitySecrets) throws {
        var blob = Data()
        blob.append(secrets.signingSeed)
        blob.append(secrets.encryptionSeed)
        blob.append(secrets.localDataKey)
        guard blob.count == 96 else {
            throw KeychainError.invalidLength
        }
        let base: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]
        SecItemDelete(base as CFDictionary)
        var add = base
        add[kSecValueData as String] = blob
        add[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        let status = SecItemAdd(add as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw KeychainError.unhandled(status)
        }
    }

    enum KeychainError: Error {
        case invalidLength
        case unhandled(OSStatus)
    }
}
