//
//  DarkModeSupport.swift
//  ZippyRemote
//
//  Dark Mode support and color scheme management
//

import SwiftUI

// Color scheme extension
extension ColorScheme {
    var isDark: Bool {
        self == .dark
    }
}

// Custom colors that adapt to dark mode
struct AdaptiveColors {
    static var background: Color {
        Color(uiColor: UIColor.systemBackground)
    }
    
    static var secondaryBackground: Color {
        Color(uiColor: UIColor.secondarySystemBackground)
    }
    
    static var text: Color {
        Color(uiColor: UIColor.label)
    }
    
    static var secondaryText: Color {
        Color(uiColor: UIColor.secondaryLabel)
    }
}

// View modifier for dark mode support
struct DarkModeModifier: ViewModifier {
    @Environment(\.colorScheme) var colorScheme
    
    func body(content: Content) -> some View {
        content
            .preferredColorScheme(.dark) // Force dark mode for viewer
            .background(AdaptiveColors.background)
    }
}

extension View {
    func darkModeSupport() -> some View {
        modifier(DarkModeModifier())
    }
}
