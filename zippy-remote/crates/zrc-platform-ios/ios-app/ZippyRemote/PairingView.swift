//
//  PairingView.swift
//  ZippyRemote
//
//  Device pairing view with QR code scanning
//

import SwiftUI
import AVFoundation
import AudioToolbox

struct PairingView: View {
    @EnvironmentObject var appState: AppState
    @Environment(\.dismiss) var dismiss
    @State private var inviteCode: String = ""
    @State private var showingCamera = false
    
    var body: some View {
        NavigationView {
            Form {
                Section(header: Text("Pair Device")) {
                    TextField("Invite Code", text: $inviteCode)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                    
                    Button("Scan QR Code") {
                        showingCamera = true
                    }
                    
                    Button("Import from Clipboard") {
                        if let clipboard = UIPasteboard.general.string {
                            inviteCode = clipboard
                        }
                    }
                    
                    Button("Import from File") {
                        // Present document picker
                        let picker = UIDocumentPickerViewController(forOpeningContentTypes: [.text, .json])
                        picker.delegate = self
                        if let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
                           let rootViewController = windowScene.windows.first?.rootViewController {
                            rootViewController.present(picker, animated: true)
                        }
                    }
                }
                
                Section {
                    Button("Pair") {
                        pairDevice()
                    }
                    .disabled(inviteCode.isEmpty)
                }
            }
            .navigationTitle("Pair Device")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") {
                        dismiss()
                    }
                }
            }
            .sheet(isPresented: $showingCamera) {
                QRCodeScannerView { code in
                    inviteCode = code
                    showingCamera = false
                }
            }
        }
    }
    
    private func pairDevice() {
        guard !inviteCode.isEmpty, let zrcCore = appState.zrcCore else {
            return
        }
        
        Task {
            do {
                // Parse invite code (format: JSON or base64 encoded)
                // For now, assume it's a JSON string with invite data
                // In production, this would use zrc-core's pairing controller
                
                // TODO: Full pairing implementation requires:
                // 1. Parse invite code (JSON or base64)
                // 2. Extract device info and SAS code
                // 3. Display SAS verification UI
                // 4. Use zrc-core PairingController to complete pairing
                // 5. Store pairing in Keychain via KeychainStore
                
                // Placeholder: Basic validation
                if inviteCode.count > 10 {
                    // Simulate pairing success
                    await MainActor.run {
                        dismiss()
                    }
                } else {
                    // Show error
                    await MainActor.run {
                        // In production, show alert with error
                        print("Invalid invite code")
                    }
                }
            } catch {
                await MainActor.run {
                    print("Pairing failed: \(error)")
                }
            }
        }
    }
}

extension PairingView: UIDocumentPickerDelegate {
    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        guard let url = urls.first else { return }
        
        // Read file content
        if url.startAccessingSecurityScopedResource() {
            defer { url.stopAccessingSecurityScopedResource() }
            
            if let data = try? Data(contentsOf: url),
               let content = String(data: data, encoding: .utf8) {
                inviteCode = content
            }
        }
    }
}

struct QRCodeScannerView: UIViewControllerRepresentable {
    let onCodeScanned: (String) -> Void
    
    func makeUIViewController(context: Context) -> QRScannerViewController {
        let vc = QRScannerViewController()
        vc.onCodeScanned = onCodeScanned
        return vc
    }
    
    func updateUIViewController(_ uiViewController: QRScannerViewController, context: Context) {}
}

// QR Scanner implementation
class QRScannerViewController: UIViewController {
    var onCodeScanned: ((String) -> Void)?
    
    private var captureSession: AVCaptureSession?
    private var previewLayer: AVCaptureVideoPreviewLayer?
    
    override func viewDidLoad() {
        super.viewDidLoad()
        setupCamera()
    }
    
    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        if let session = captureSession, !session.isRunning {
            session.startRunning()
        }
    }
    
    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        if let session = captureSession, session.isRunning {
            session.stopRunning()
        }
    }
    
    private func setupCamera() {
        let session = AVCaptureSession()
        self.captureSession = session
        
        guard let videoCaptureDevice = AVCaptureDevice.default(for: .video) else {
            print("Camera not available")
            return
        }
        
        do {
            let videoInput = try AVCaptureDeviceInput(device: videoCaptureDevice)
            if session.canAddInput(videoInput) {
                session.addInput(videoInput)
            } else {
                print("Cannot add video input")
                return
            }
            
            let metadataOutput = AVCaptureMetadataOutput()
            if session.canAddOutput(metadataOutput) {
                session.addOutput(metadataOutput)
                
                metadataOutput.setMetadataObjectsDelegate(self, queue: DispatchQueue.main)
                metadataOutput.metadataObjectTypes = [.qr]
            } else {
                print("Cannot add metadata output")
                return
            }
            
            previewLayer = AVCaptureVideoPreviewLayer(session: session)
            previewLayer?.frame = view.layer.bounds
            previewLayer?.videoGravity = .resizeAspectFill
            view.layer.addSublayer(previewLayer!)
            
            session.startRunning()
        } catch {
            print("Error setting up camera: \(error)")
        }
    }
    
    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        previewLayer?.frame = view.layer.bounds
    }
}

extension QRScannerViewController: AVCaptureMetadataOutputObjectsDelegate {
    func metadataOutput(_ output: AVCaptureMetadataOutput, didOutput metadataObjects: [AVMetadataObject], from connection: AVCaptureConnection) {
        if let metadataObject = metadataObjects.first {
            guard let readableObject = metadataObject as? AVMetadataMachineReadableCodeObject,
                  let stringValue = readableObject.stringValue else {
                return
            }
            
            AudioServicesPlaySystemSound(SystemSoundID(kSystemSoundID_Vibrate))
            onCodeScanned?(stringValue)
        }
    }
}
