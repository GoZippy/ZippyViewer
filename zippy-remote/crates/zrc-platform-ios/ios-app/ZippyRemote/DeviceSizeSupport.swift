//
//  DeviceSizeSupport.swift
//  ZippyRemote
//
//  Support for all iPhone and iPad sizes, Split View, Slide Over
//

import SwiftUI

// Device type detection
enum DeviceType {
    case iPhone
    case iPad
    case mac
}

struct DeviceInfo {
    static var current: DeviceType {
        #if os(iOS)
        if UIDevice.current.userInterfaceIdiom == .pad {
            return .iPad
        } else {
            return .iPhone
        }
        #else
        return .mac
        #endif
    }
    
    static var isIPad: Bool {
        current == .iPad
    }
    
    static var isIPhone: Bool {
        current == .iPhone
    }
}

// Adaptive layout for different device sizes
struct AdaptiveLayout<Content: View>: View {
    let content: Content
    
    init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }
    
    var body: some View {
        if DeviceInfo.isIPad {
            // iPad layout with more space
            HStack {
                Spacer()
                content
                    .frame(maxWidth: 1200)
                Spacer()
            }
        } else {
            // iPhone layout
            content
        }
    }
}

// Split View and Slide Over support (iPad only)
struct SplitViewSupport: ViewModifier {
    func body(content: Content) -> some View {
        if DeviceInfo.isIPad {
            content
                .navigationViewStyle(.automatic) // Supports Split View
        } else {
            content
                .navigationViewStyle(.stack)
        }
    }
}

extension View {
    func splitViewSupport() -> some View {
        modifier(SplitViewSupport())
    }
}
