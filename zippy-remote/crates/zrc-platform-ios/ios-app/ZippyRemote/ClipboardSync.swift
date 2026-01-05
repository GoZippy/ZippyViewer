//
//  ClipboardSync.swift
//  ZippyRemote
//
//  Clipboard synchronization between local and remote devices
//

import UIKit

class ClipboardSync: ObservableObject {
    @Published var isEnabled: Bool = true
    
    private let zrcCore: ZrcCore?
    private let sessionId: UInt64?
    
    init(zrcCore: ZrcCore?, sessionId: UInt64?) {
        self.zrcCore = zrcCore
        self.sessionId = sessionId
    }
    
    /// Read local clipboard
    func readLocalClipboard() -> String? {
        return UIPasteboard.general.string
    }
    
    /// Write to local clipboard
    func writeLocalClipboard(_ text: String) {
        UIPasteboard.general.string = text
    }
    
    /// Read image from local clipboard
    func readLocalClipboardImage() -> Data? {
        return UIPasteboard.general.image?.pngData()
    }
    
    /// Write image to local clipboard
    func writeLocalClipboardImage(_ data: Data) {
        if let image = UIImage(data: data) {
            UIPasteboard.general.image = image
        }
    }
    
    /// Send clipboard to remote device
    func sendToRemote(_ text: String) {
        guard isEnabled, let zrcCore = zrcCore, let sessionId = sessionId else { return }
        
        // TODO: Implement clipboard send via ZRC protocol
        // This would typically:
        // 1. Serialize clipboard data (text or image)
        // 2. Send via ZRC clipboard sync protocol message
        // 3. Handle errors and retries
        
        // Placeholder: The actual protocol implementation requires
        // integration with zrc-core's clipboard sync mechanism
        Task {
            // In production, this would use zrc-core's clipboard sync API
            // For now, this is a structural placeholder
        }
    }
    
    /// Receive clipboard from remote device
    func receiveFromRemote(_ text: String) {
        guard isEnabled else { return }
        writeLocalClipboard(text)
    }
}
