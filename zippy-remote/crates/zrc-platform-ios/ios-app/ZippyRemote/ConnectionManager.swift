//
//  ConnectionManager.swift
//  ZippyRemote
//
//  Connection management with network change handling and background tasks
//

import Foundation
import Network
import UIKit

class ConnectionManager: ObservableObject {
    @Published var connectionStatus: ConnectionStatus = .disconnected
    @Published var networkType: NetworkType = .unknown
    
    private let pathMonitor = NWPathMonitor()
    private let monitorQueue = DispatchQueue(label: "NetworkMonitor")
    private var backgroundTask: UIBackgroundTaskIdentifier = .invalid
    
    init() {
        setupNetworkMonitoring()
        setupBackgroundTask()
    }
    
    private func setupNetworkMonitoring() {
        pathMonitor.pathUpdateHandler = { [weak self] path in
            DispatchQueue.main.async {
                self?.handleNetworkChange(path: path)
            }
        }
        pathMonitor.start(queue: monitorQueue)
    }
    
    private func handleNetworkChange(path: NWPath) {
        // Determine network type
        if path.usesInterfaceType(.wifi) {
            networkType = .wifi
        } else if path.usesInterfaceType(.cellular) {
            networkType = .cellular
        } else if path.usesInterfaceType(.wiredEthernet) {
            networkType = .ethernet
        } else {
            networkType = .unknown
        }
        
        // Update connection status
        if path.status == .satisfied {
            if connectionStatus == .disconnected {
                connectionStatus = .connected
                // Attempt reconnection
                attemptReconnection()
            }
        } else {
            connectionStatus = .disconnected
        }
    }
    
    private func attemptReconnection() {
        // Attempt to reconnect active sessions
        // This would typically:
        // 1. Check if there are active sessions
        // 2. Re-establish QUIC connections
        // 3. Resume frame polling
        // 4. Update connection status
        
        // For now, this is a placeholder that would integrate with ZrcCore
        // to reconnect sessions after network changes
        if connectionStatus == .connected {
            // Trigger reconnection in app state
            NotificationCenter.default.post(
                name: NSNotification.Name("NetworkReconnected"),
                object: nil
            )
        }
    }
    
    private func setupBackgroundTask() {
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(appDidEnterBackground),
            name: UIApplication.didEnterBackgroundNotification,
            object: nil
        )
        
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(appWillEnterForeground),
            name: UIApplication.willEnterForegroundNotification,
            object: nil
        )
    }
    
    @objc private func appDidEnterBackground() {
        // Start background task for graceful disconnect
        backgroundTask = UIApplication.shared.beginBackgroundTask { [weak self] in
            self?.endBackgroundTask()
        }
        
        // Perform graceful disconnect
        performGracefulDisconnect()
    }
    
    @objc private func appWillEnterForeground() {
        endBackgroundTask()
    }
    
    private func performGracefulDisconnect() {
        // Perform graceful disconnect
        // 1. Notify app to send disconnect message to remote device
        NotificationCenter.default.post(
            name: NSNotification.Name("AppEnteringBackground"),
            object: nil
        )
        
        // 2. Close QUIC connection cleanly (handled by ZrcCore)
        // 3. Clean up session state (handled by app state)
        
        // End background task after completion
        // Use a reasonable timeout (iOS allows up to 30 seconds)
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) { [weak self] in
            self?.endBackgroundTask()
        }
    }
    
    private func endBackgroundTask() {
        if backgroundTask != .invalid {
            UIApplication.shared.endBackgroundTask(backgroundTask)
            backgroundTask = .invalid
        }
    }
    
    deinit {
        pathMonitor.cancel()
        NotificationCenter.default.removeObserver(self)
    }
}

enum ConnectionStatus {
    case connected
    case connecting
    case disconnected
    case error(String)
}

enum NetworkType {
    case wifi
    case cellular
    case ethernet
    case unknown
}
