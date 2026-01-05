//
//  App.swift
//  ZippyRemote
//
//  Main app entry point
//

import SwiftUI

@main
struct ZippyRemoteApp: App {
    @StateObject private var appState = AppState()
    
    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(appState)
                .preferredColorScheme(.dark) // Support dark mode
        }
    }
}

/// Global app state
class AppState: ObservableObject {
    @Published var zrcCore: ZrcCore?
    @Published var currentSession: UInt64?
    @Published var devices: [Device] = []
    
    init() {
        // Initialize ZRC core with default config
        let config = """
        {
            "rendezvous_urls": [],
            "relay_urls": [],
            "transport_preference": "auto"
        }
        """
        
        do {
            zrcCore = try ZrcCore(configJson: config)
        } catch {
            print("Failed to initialize ZRC core: \(error)")
        }
    }
}
