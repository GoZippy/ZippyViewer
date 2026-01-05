//
//  ContentView.swift
//  ZippyRemote
//
//  Main content view with navigation
//

import SwiftUI

struct ContentView: View {
    @EnvironmentObject var appState: AppState
    
    var body: some View {
        NavigationView {
            DeviceListView()
                .environmentObject(appState)
        }
        .navigationViewStyle(.stack)
    }
}
