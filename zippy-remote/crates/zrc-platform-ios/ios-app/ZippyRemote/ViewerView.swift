//
//  ViewerView.swift
//  ZippyRemote
//
//  Main viewer view with Metal rendering and touch input
//

import SwiftUI
import MetalKit

struct ViewerView: View {
    @EnvironmentObject var appState: AppState
    @StateObject private var renderer = MetalFrameRenderer()
    @StateObject private var inputHandler = TouchInputHandler()
    
    var body: some View {
        ZStack {
            MetalView(renderer: renderer)
                .gesture(
                    DragGesture(minimumDistance: 0)
                        .onChanged { value in
                            inputHandler.handlePan(translation: value.translation, location: value.location)
                        }
                )
                .onTapGesture { location in
                    inputHandler.handleTap(at: location)
                }
                .onLongPressGesture {
                    // Right click
                } perform: { location in
                    inputHandler.handleLongPress(at: location)
                }
            
            VStack {
                ConnectionStatusView()
                Spacer()
                KeyboardToolbar()
            }
        }
        .task {
            await pollFrames()
        }
    }
    
    private func pollFrames() async {
        guard let zrcCore = appState.zrcCore,
              let sessionId = appState.currentSession else { return }
        
        while appState.currentSession != nil {
            if let frame = try? await zrcCore.pollFrame(sessionId: sessionId) {
                await MainActor.run {
                    renderer.updateFrame(frame)
                }
            }
            try? await Task.sleep(nanoseconds: 16_666_666) // ~60fps
        }
    }
}

struct ConnectionStatusView: View {
    @EnvironmentObject var appState: AppState
    
    var body: some View {
        HStack {
            Circle()
                .fill(appState.currentSession != nil ? Color.green : Color.red)
                .frame(width: 8, height: 8)
            Text(appState.currentSession != nil ? "Connected" : "Disconnected")
                .font(.caption)
        }
        .padding(8)
        .background(Color.black.opacity(0.5))
        .cornerRadius(8)
    }
}

struct KeyboardToolbar: View {
    @EnvironmentObject var appState: AppState
    @State private var ctrlPressed = false
    @State private var altPressed = false
    @State private var cmdPressed = false
    @State private var shiftPressed = false
    
    var body: some View {
        HStack {
            Button(action: { toggleModifier(key: .ctrl) }) {
                Text("Ctrl")
                    .foregroundColor(ctrlPressed ? .white : .primary)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                    .background(ctrlPressed ? Color.blue : Color(.systemGray5))
                    .cornerRadius(8)
            }
            
            Button(action: { toggleModifier(key: .alt) }) {
                Text("Alt")
                    .foregroundColor(altPressed ? .white : .primary)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                    .background(altPressed ? Color.blue : Color(.systemGray5))
                    .cornerRadius(8)
            }
            
            Button(action: { toggleModifier(key: .cmd) }) {
                Text("Cmd")
                    .foregroundColor(cmdPressed ? .white : .primary)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                    .background(cmdPressed ? Color.blue : Color(.systemGray5))
                    .cornerRadius(8)
            }
            
            Button(action: { toggleModifier(key: .shift) }) {
                Text("Shift")
                    .foregroundColor(shiftPressed ? .white : .primary)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                    .background(shiftPressed ? Color.blue : Color(.systemGray5))
                    .cornerRadius(8)
            }
            
            Spacer()
            
            Menu {
                Button("Ctrl+Alt+Del") {
                    sendCtrlAltDel()
                }
            } label: {
                Text("Ctrl+Alt+Del")
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                    .background(Color(.systemGray5))
                    .cornerRadius(8)
            }
        }
        .padding()
        .background(Color(.systemGray6))
    }
    
    private func toggleModifier(key: ModifierKey) {
        guard let zrcCore = appState.zrcCore,
              let sessionId = appState.currentSession else { return }
        
        Task {
            let keyCode: UInt32
            switch key {
            case .ctrl: keyCode = 0x3B // Left Control
            case .alt: keyCode = 0x3A // Left Option/Alt
            case .cmd: keyCode = 0x37 // Left Command
            case .shift: keyCode = 0x38 // Left Shift
            }
            
            let isPressed: Bool
            switch key {
            case .ctrl: isPressed = !ctrlPressed; ctrlPressed.toggle()
            case .alt: isPressed = !altPressed; altPressed.toggle()
            case .cmd: isPressed = !cmdPressed; cmdPressed.toggle()
            case .shift: isPressed = !shiftPressed; shiftPressed.toggle()
            }
            
            let event = InputEvent.keyPress(code: keyCode, down: isPressed)
            try? await zrcCore.sendInput(sessionId: sessionId, event: event)
        }
    }
    
    private func sendCtrlAltDel() {
        guard let zrcCore = appState.zrcCore,
              let sessionId = appState.currentSession else { return }
        
        Task {
            // Send Ctrl+Alt+Del sequence
            let ctrlEvent = InputEvent.keyPress(code: 0x3B, down: true) // Ctrl down
            try? await zrcCore.sendInput(sessionId: sessionId, event: ctrlEvent)
            
            let altEvent = InputEvent.keyPress(code: 0x3A, down: true) // Alt down
            try? await zrcCore.sendInput(sessionId: sessionId, event: altEvent)
            
            let delEvent = InputEvent.keyPress(code: 0x75, down: true) // Delete down
            try? await zrcCore.sendInput(sessionId: sessionId, event: delEvent)
            
            // Release in reverse order
            try? await Task.sleep(nanoseconds: 50_000_000) // 50ms delay
            
            let delEventUp = InputEvent.keyPress(code: 0x75, down: false) // Delete up
            try? await zrcCore.sendInput(sessionId: sessionId, event: delEventUp)
            
            let altEventUp = InputEvent.keyPress(code: 0x3A, down: false) // Alt up
            try? await zrcCore.sendInput(sessionId: sessionId, event: altEventUp)
            
            let ctrlEventUp = InputEvent.keyPress(code: 0x3B, down: false) // Ctrl up
            try? await zrcCore.sendInput(sessionId: sessionId, event: ctrlEventUp)
        }
    }
}

enum ModifierKey {
    case ctrl
    case alt
    case cmd
    case shift
}
