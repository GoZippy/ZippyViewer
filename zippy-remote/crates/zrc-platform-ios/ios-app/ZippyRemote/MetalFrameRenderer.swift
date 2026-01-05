//
//  MetalFrameRenderer.swift
//  ZippyRemote
//
//  Metal-based frame renderer for efficient GPU rendering
//

import MetalKit
import Metal

class MetalFrameRenderer: NSObject, ObservableObject, MTKViewDelegate {
    private let device: MTLDevice
    private let commandQueue: MTLCommandQueue
    private var pipelineState: MTLRenderPipelineState?
    private var currentTexture: MTLTexture?
    
    private var zoom: Float = 1.0
    private var panOffset: CGPoint = .zero
    
    override init() {
        guard let device = MTLCreateSystemDefaultDevice() else {
            fatalError("Metal is not supported on this device")
        }
        
        self.device = device
        
        guard let commandQueue = device.makeCommandQueue() else {
            fatalError("Failed to create command queue")
        }
        
        self.commandQueue = commandQueue
        super.init()
    }
    
    func setup(metalView: MTKView) {
        metalView.device = device
        metalView.colorPixelFormat = .bgra8Unorm
        metalView.delegate = self
        
        // Create render pipeline
        guard let library = device.makeDefaultLibrary() else {
            fatalError("Failed to create Metal library")
        }
        
        let vertexFunction = library.makeFunction(name: "vertexShader")
        let fragmentFunction = library.makeFunction(name: "fragmentShader")
        
        let pipelineDescriptor = MTLRenderPipelineDescriptor()
        pipelineDescriptor.vertexFunction = vertexFunction
        pipelineDescriptor.fragmentFunction = fragmentFunction
        pipelineDescriptor.colorAttachments[0].pixelFormat = metalView.colorPixelFormat
        
        do {
            pipelineState = try device.makeRenderPipelineState(descriptor: pipelineDescriptor)
        } catch {
            fatalError("Failed to create pipeline state: \(error)")
        }
    }
    
    func updateFrame(_ frameData: FrameData) {
        let textureDescriptor = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .bgra8Unorm,
            width: Int(frameData.width),
            height: Int(frameData.height),
            mipmapped: false
        )
        textureDescriptor.usage = [.shaderRead]
        
        guard let texture = device.makeTexture(descriptor: textureDescriptor) else {
            return
        }
        
        frameData.data.withUnsafeBytes { ptr in
            texture.replace(
                region: MTLRegionMake2D(0, 0, Int(frameData.width), Int(frameData.height)),
                mipmapLevel: 0,
                withBytes: ptr.baseAddress!,
                bytesPerRow: Int(frameData.width) * 4
            )
        }
        
        currentTexture = texture
    }
    
    func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {
        // Handle size changes
    }
    
    func draw(in view: MTKView) {
        guard let texture = currentTexture,
              let commandBuffer = commandQueue.makeCommandBuffer(),
              let renderPassDescriptor = view.currentRenderPassDescriptor,
              let renderEncoder = commandBuffer.makeRenderCommandEncoder(descriptor: renderPassDescriptor),
              let pipelineState = pipelineState
        else { return }
        
        renderEncoder.setRenderPipelineState(pipelineState)
        renderEncoder.setFragmentTexture(texture, index: 0)
        
        // Create full-screen quad vertices
        let viewSize = view.drawableSize
        let textureSize = CGSize(width: CGFloat(texture.width), height: CGFloat(texture.height))
        
        // Calculate aspect ratio and scaling
        let viewAspect = Float(viewSize.width / viewSize.height)
        let textureAspect = Float(textureSize.width / textureSize.height)
        
        // Scale to fit while maintaining aspect ratio
        var scaleX: Float = 1.0
        var scaleY: Float = 1.0
        if textureAspect > viewAspect {
            scaleY = viewAspect / textureAspect
        } else {
            scaleX = textureAspect / viewAspect
        }
        
        // Apply zoom and pan transforms
        var transform = matrix_identity_float4x4
        transform = matrix_multiply(transform, matrix4x4_scale(zoom * scaleX, zoom * scaleY, 1.0))
        transform = matrix_multiply(transform, matrix4x4_translation(Float(panOffset.x), Float(panOffset.y), 0))
        renderEncoder.setVertexBytes(&transform, length: MemoryLayout<matrix_float4x4>.size, index: 1)
        
        // Full-screen quad vertices (normalized device coordinates)
        let vertices: [Float] = [
            -1.0, -1.0, 0.0, 1.0,  // Bottom-left
             1.0, -1.0, 1.0, 1.0,  // Bottom-right
            -1.0,  1.0, 0.0, 0.0,  // Top-left
             1.0,  1.0, 1.0, 0.0   // Top-right
        ]
        
        renderEncoder.setVertexBytes(vertices, length: vertices.count * MemoryLayout<Float>.size, index: 0)
        
        // Draw quad as triangle strip
        renderEncoder.drawPrimitives(type: .triangleStrip, vertexStart: 0, vertexCount: 4)
        renderEncoder.endEncoding()
        
        if let drawable = view.currentDrawable {
            commandBuffer.present(drawable)
        }
        commandBuffer.commit()
    }
}

// Helper functions for matrix operations
func matrix4x4_scale(_ x: Float, _ y: Float, _ z: Float) -> matrix_float4x4 {
    return matrix_float4x4(
        columns: (
            vector_float4(x, 0, 0, 0),
            vector_float4(0, y, 0, 0),
            vector_float4(0, 0, z, 0),
            vector_float4(0, 0, 0, 1)
        )
    )
}

func matrix4x4_translation(_ x: Float, _ y: Float, _ z: Float) -> matrix_float4x4 {
    return matrix_float4x4(
        columns: (
            vector_float4(1, 0, 0, 0),
            vector_float4(0, 1, 0, 0),
            vector_float4(0, 0, 1, 0),
            vector_float4(x, y, z, 1)
        )
    )
}

// Metal view wrapper
struct MetalView: UIViewRepresentable {
    let renderer: MetalFrameRenderer
    
    func makeUIView(context: Context) -> MTKView {
        let view = MTKView()
        renderer.setup(metalView: view)
        return view
    }
    
    func updateUIView(_ uiView: MTKView, context: Context) {
        // Updates handled by delegate
    }
}
