//
//  BroadcastSampleHandler.swift
//  BroadcastExtension
//
//  ReplayKit Broadcast Extension for screen sharing
//

import ReplayKit
import CoreMedia

class BroadcastSampleHandler: RPBroadcastSampleHandler {
    
    private var zrcCore: ZrcCore?
    private var sessionId: UInt64?
    
    override func broadcastStarted(withSetupInfo setupInfo: [String: NSObject]?) {
        // Initialize ZRC core
        let config = """
        {
            "rendezvous_urls": [],
            "relay_urls": [],
            "transport_preference": "auto"
        }
        """
        
        do {
            zrcCore = try ZrcCore(configJson: config)
            
            // Connect to waiting controller
            if let deviceId = setupInfo?["deviceId"] as? Data {
                Task {
                    do {
                        sessionId = try await zrcCore?.startSession(deviceId: deviceId)
                    } catch {
                        finishBroadcastWithError(error)
                    }
                }
            }
        } catch {
            finishBroadcastWithError(error)
        }
    }
    
    override func broadcastFinished() {
        if let sessionId = sessionId {
            Task {
                try? await zrcCore?.endSession(sessionId: sessionId)
            }
        }
        zrcCore = nil
    }
    
    override func processSampleBuffer(_ sampleBuffer: CMSampleBuffer, with sampleBufferType: RPSampleBufferType) {
        guard sampleBufferType == .video,
              let sessionId = sessionId,
              let imageBuffer = CMSampleBufferGetImageBuffer(sampleBuffer)
        else { return }
        
        CVPixelBufferLockBaseAddress(imageBuffer, .readOnly)
        defer { CVPixelBufferUnlockBaseAddress(imageBuffer, .readOnly) }
        
        let width = CVPixelBufferGetWidth(imageBuffer)
        let height = CVPixelBufferGetHeight(imageBuffer)
        let bytesPerRow = CVPixelBufferGetBytesPerRow(imageBuffer)
        
        guard let baseAddress = CVPixelBufferGetBaseAddress(imageBuffer) else { return }
        
        // Copy pixel data
        let data = Data(bytes: baseAddress, count: bytesPerRow * height)
        
        let timestamp = UInt64(CMSampleBufferGetPresentationTimeStamp(sampleBuffer).seconds * 1000)
        
        let frameData = FrameData(
            data: data,
            width: UInt32(width),
            height: UInt32(height),
            timestamp: timestamp
        )
        
        // Send frame to connected controller
        Task {
            // Send frame via ZRC protocol
            // This would typically:
            // 1. Encode frame data in ZRC frame packet format
            // 2. Send via QUIC media stream
            // 3. Handle memory pressure and frame dropping if needed
            
            guard let zrcCore = zrcCore, let sessionId = sessionId else { return }
            
            // TODO: Full implementation requires:
            // - Frame encoding/compression
            // - QUIC stream writing
            // - Memory management (stay under 50MB limit)
            
            // For now, this is a structural placeholder
            // The actual frame sending would use zrc-core's media transport API
        }
    }
}
