//
//  AccessibilitySupport.swift
//  ZippyRemote
//
//  Accessibility support: VoiceOver, Dynamic Type, etc.
//

import SwiftUI

// Dynamic Type support
extension View {
    func dynamicTypeSize(_ size: DynamicTypeSize) -> some View {
        self.environment(\.sizeCategory, size)
    }
}

// VoiceOver support
struct AccessibleButton: View {
    let title: String
    let action: () -> Void
    let accessibilityHint: String?
    
    init(_ title: String, hint: String? = nil, action: @escaping () -> Void) {
        self.title = title
        self.accessibilityHint = hint
        self.action = action
    }
    
    var body: some View {
        Button(action: action) {
            Text(title)
        }
        .accessibilityLabel(title)
        .accessibilityHint(accessibilityHint ?? "")
    }
}

// VoiceOver labels for viewer
struct ViewerAccessibilityLabels {
    static let viewer = "Remote Desktop Viewer"
    static let tapToClick = "Tap to left click"
    static let longPressToRightClick = "Long press to right click"
    static let panToMove = "Drag to move mouse"
    static let twoFingerScroll = "Two finger scroll"
    static let connected = "Connected to remote device"
    static let disconnected = "Disconnected from remote device"
}
