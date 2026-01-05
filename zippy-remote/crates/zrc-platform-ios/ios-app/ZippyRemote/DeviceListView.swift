//
//  DeviceListView.swift
//  ZippyRemote
//
//  Device list view with pairing support
//

import SwiftUI

struct Device: Identifiable {
    let id: String
    let name: String
    var isOnline: Bool
    var lastSeen: Date?
}

struct DeviceListView: View {
    @EnvironmentObject var appState: AppState
    @State private var showingPairingSheet = false
    
    var body: some View {
        List {
            ForEach(appState.devices) { device in
                DeviceRow(device: device)
                    .onTapGesture {
                        connectToDevice(device)
                    }
            }
            .onDelete(perform: removeDevices)
        }
        .navigationTitle("Devices")
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button(action: { showingPairingSheet = true }) {
                    Image(systemName: "plus")
                }
            }
        }
        .sheet(isPresented: $showingPairingSheet) {
            PairingView()
                .environmentObject(appState)
        }
    }
    
    private func connectToDevice(_ device: Device) {
        // TODO: Implement device connection
        guard let zrcCore = appState.zrcCore else { return }
        
        Task {
            do {
                let deviceId = Data(hex: device.id) ?? Data()
                let sessionId = try await zrcCore.startSession(deviceId: deviceId)
                await MainActor.run {
                    appState.currentSession = sessionId
                }
            } catch {
                print("Failed to start session: \(error)")
            }
        }
    }
    
    private func removeDevices(at offsets: IndexSet) {
        appState.devices.remove(atOffsets: offsets)
    }
}

struct DeviceRow: View {
    let device: Device
    
    var body: some View {
        HStack {
            Circle()
                .fill(device.isOnline ? Color.green : Color.gray)
                .frame(width: 10, height: 10)
            
            VStack(alignment: .leading) {
                Text(device.name)
                    .font(.headline)
                Text(String(device.id.prefix(8)))
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            
            Spacer()
            
            if let lastSeen = device.lastSeen {
                Text(lastSeen, style: .relative)
                    .font(.caption2)
                    .foregroundColor(.secondary)
            }
        }
        .padding(.vertical, 4)
    }
}

// Helper extension for hex string to Data
extension Data {
    init?(hex: String) {
        let len = hex.count / 2
        var data = Data(capacity: len)
        var i = hex.startIndex
        for _ in 0..<len {
            let j = hex.index(i, offsetBy: 2)
            let bytes = hex[i..<j]
            if var num = UInt8(bytes, radix: 16) {
                data.append(&num, count: 1)
            } else {
                return nil
            }
            i = j
        }
        self = data
    }
}
