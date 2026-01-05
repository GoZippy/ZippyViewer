//
//  TouchInputHandler.swift
//  ZippyRemote
//
//  Touch input handling and gesture recognition
//

import SwiftUI
import UIKit

class TouchInputHandler: ObservableObject {
    private var inputSender: InputSender?
    private var viewBounds: (() -> CGRect)?
    private var remoteSize: (() -> CGSize)?
    
    private var lastTouchLocation: CGPoint?
    
    func setup(inputSender: InputSender, viewBounds: @escaping () -> CGRect, remoteSize: @escaping () -> CGSize) {
        self.inputSender = inputSender
        self.viewBounds = viewBounds
        self.remoteSize = remoteSize
    }
    
    func handleTap(at location: CGPoint) {
        guard let inputSender = inputSender else { return }
        let remotePoint = mapToRemote(location)
        inputSender.sendClick(x: Int32(remotePoint.x), y: Int32(remotePoint.y), button: .left)
        
        // Haptic feedback
        let generator = UIImpactFeedbackGenerator(style: .light)
        generator.impactOccurred()
    }
    
    func handleLongPress(at location: CGPoint) {
        guard let inputSender = inputSender else { return }
        let remotePoint = mapToRemote(location)
        inputSender.sendClick(x: Int32(remotePoint.x), y: Int32(remotePoint.y), button: .right)
        
        let generator = UIImpactFeedbackGenerator(style: .medium)
        generator.impactOccurred()
    }
    
    func handlePan(translation: CGPoint, location: CGPoint) {
        guard let inputSender = inputSender else { return }
        let remotePoint = mapToRemote(location)
        inputSender.sendMouseMove(x: Int32(remotePoint.x), y: Int32(remotePoint.y))
    }
    
    func handleTwoFingerScroll(translation: CGPoint) {
        guard let inputSender = inputSender else { return }
        inputSender.sendScroll(deltaX: Int32(translation.x), deltaY: Int32(translation.y))
    }
    
    private func mapToRemote(_ local: CGPoint) -> CGPoint {
        guard let viewBounds = viewBounds,
              let remoteSize = remoteSize else {
            return local
        }
        
        let bounds = viewBounds()
        let remote = remoteSize()
        
        let scaleX = remote.width / bounds.width
        let scaleY = remote.height / bounds.height
        
        return CGPoint(
            x: local.x * scaleX,
            y: local.y * scaleY
        )
    }
}

// Input sender wrapper
class InputSender {
    private let zrcCore: ZrcCore
    private let sessionId: UInt64
    
    init(zrcCore: ZrcCore, sessionId: UInt64) {
        self.zrcCore = zrcCore
        self.sessionId = sessionId
    }
    
    func sendClick(x: Int32, y: Int32, button: MouseButton) {
        Task {
            let event = InputEvent.mouseClick(x: x, y: y, button: button)
            try? await zrcCore.sendInput(sessionId: sessionId, event: event)
        }
    }
    
    func sendMouseMove(x: Int32, y: Int32) {
        Task {
            let event = InputEvent.mouseMove(x: x, y: y)
            try? await zrcCore.sendInput(sessionId: sessionId, event: event)
        }
    }
    
    func sendScroll(deltaX: Int32, deltaY: Int32) {
        Task {
            let event = InputEvent.scroll(deltaX: deltaX, deltaY: deltaY)
            try? await zrcCore.sendInput(sessionId: sessionId, event: event)
        }
    }
}
