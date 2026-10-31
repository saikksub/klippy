import Foundation
import LocalAuthentication
import Security

/// The key that encrypts the history database. Created at random on first
/// use and kept in the Keychain; handed out only after the user passes
/// Touch ID (or types their login password, e.g. with the lid closed).
/// No permission prompt is involved: macOS just shows its Touch ID sheet.
enum HistoryKey {
    private static let service = "com.kksaisub.klippy.history"
    private static let account = "history-encryption-key"
    private static let keyBytes = 32

    /// The key, or a message saying why the user couldn't be authenticated.
    static func unlock() async -> Result<[UInt8], UnlockFailure> {
        let context = LAContext()
        do {
            try await context.evaluatePolicy(
                .deviceOwnerAuthentication,
                localizedReason: "unlock your clipboard history"
            )
        } catch let error as LAError where error.code == .userCancel || error.code == .appCancel {
            return .failure(UnlockFailure("Unlock was cancelled."))
        } catch {
            return .failure(UnlockFailure(error.localizedDescription))
        }
        do {
            return .success(try storedKey() ?? createKey())
        } catch {
            return .failure(UnlockFailure("Couldn't read the key from the Keychain (\(error.localizedDescription))."))
        }
    }

    private static func storedKey() throws -> [UInt8]? {
        var result: CFTypeRef?
        let status = SecItemCopyMatching([
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: service,
            kSecAttrAccount: account,
            kSecReturnData: true,
        ] as CFDictionary, &result)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess, let data = result as? Data else { throw KeychainError(status) }
        return [UInt8](data)
    }

    private static func createKey() throws -> [UInt8] {
        var key = [UInt8](repeating: 0, count: keyBytes)
        guard SecRandomCopyBytes(kSecRandomDefault, keyBytes, &key) == errSecSuccess else {
            throw KeychainError(errSecAllocate)
        }
        let status = SecItemAdd([
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: service,
            kSecAttrAccount: account,
            kSecAttrLabel: "Klippy history encryption key",
            kSecAttrAccessible: kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
            kSecValueData: Data(key),
        ] as CFDictionary, nil)
        guard status == errSecSuccess else { throw KeychainError(status) }
        return key
    }
}

struct UnlockFailure: Error {
    let message: String
    init(_ message: String) { self.message = message }
}

private struct KeychainError: LocalizedError {
    let status: OSStatus
    init(_ status: OSStatus) { self.status = status }
    var errorDescription: String? {
        SecCopyErrorMessageString(status, nil) as String? ?? "error \(status)"
    }
}
