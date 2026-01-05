//
//  KeychainStore.swift
//  ZippyRemote
//
//  iOS Keychain storage for cryptographic keys
//

import Foundation
import Security

enum KeychainError: Error {
    case storeFailed(OSStatus)
    case loadFailed(OSStatus)
    case deleteFailed(OSStatus)
    case keyGenerationFailed(Error)
}

class KeychainStore {
    private let service = "io.zippyremote.keys"
    
    /// Store a key in the Keychain
    func storeKey(name: String, data: Data) throws {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: name,
            kSecValueData as String: data,
            kSecAttrAccessible as String: kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
            kSecAttrSynchronizable as String: false  // Don't sync to iCloud
        ]
        
        // Delete existing item first
        SecItemDelete(query as CFDictionary)
        
        let status = SecItemAdd(query as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw KeychainError.storeFailed(status)
        }
    }
    
    /// Load a key from the Keychain
    func loadKey(name: String) throws -> Data? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: name,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne
        ]
        
        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        
        switch status {
        case errSecSuccess:
            return result as? Data
        case errSecItemNotFound:
            return nil
        default:
            throw KeychainError.loadFailed(status)
        }
    }
    
    /// Delete a key from the Keychain
    func deleteKey(name: String) throws {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: name
        ]
        
        let status = SecItemDelete(query as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw KeychainError.deleteFailed(status)
        }
    }
    
    /// Generate a Secure Enclave key for signing
    func generateSecureEnclaveKey(name: String) throws -> SecKey {
        var error: Unmanaged<CFError>?
        
        guard let access = SecAccessControlCreateWithFlags(
            kCFAllocatorDefault,
            kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
            [.privateKeyUsage],
            &error
        ) else {
            throw KeychainError.keyGenerationFailed(error!.takeRetainedValue())
        }
        
        let attributes: [String: Any] = [
            kSecAttrKeyType as String: kSecAttrKeyTypeECSECPrimeRandom,
            kSecAttrKeySizeInBits as String: 256,
            kSecAttrTokenID as String: kSecAttrTokenIDSecureEnclave,
            kSecPrivateKeyAttrs as String: [
                kSecAttrIsPermanent as String: true,
                kSecAttrApplicationTag as String: name.data(using: .utf8)!,
                kSecAttrAccessControl as String: access
            ]
        ]
        
        guard let privateKey = SecKeyCreateRandomKey(attributes as CFDictionary, &error) else {
            throw KeychainError.keyGenerationFailed(error!.takeRetainedValue())
        }
        
        return privateKey
    }
    
    /// Zeroize key material (overwrite with zeros before deletion)
    func zeroizeKey(name: String) throws {
        // Load key to get size
        if let data = try loadKey(name) {
            // Overwrite with zeros
            var zeroData = Data(count: data.count)
            zeroData.withUnsafeMutableBytes { ptr in
                memset(ptr.baseAddress, 0, data.count)
            }
            
            // Store zeros, then delete
            try storeKey(name: name, data: zeroData)
        }
        
        try deleteKey(name: name)
    }
}
