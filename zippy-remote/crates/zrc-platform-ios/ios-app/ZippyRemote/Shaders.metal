//
//  Shaders.metal
//  ZippyRemote
//
//  Metal shaders for frame rendering
//

#include <metal_stdlib>
using namespace metal;

struct VertexIn {
    float2 position [[attribute(0)]];
    float2 texCoord [[attribute(1)]];
};

struct VertexOut {
    float4 position [[position]];
    float2 texCoord;
};

struct Uniforms {
    float4x4 transform;
};

// Vertex shader
vertex VertexOut vertexShader(VertexIn in [[stage_in]],
                              constant Uniforms &uniforms [[buffer(1)]]) {
    VertexOut out;
    float4 position = float4(in.position, 0.0, 1.0);
    out.position = uniforms.transform * position;
    out.texCoord = in.texCoord;
    return out;
}

// Fragment shader - simple texture sampling
fragment float4 fragmentShader(VertexOut in [[stage_in]],
                                texture2d<float> texture [[texture(0)]]) {
    constexpr sampler textureSampler(mag_filter::linear, min_filter::linear);
    return texture.sample(textureSampler, in.texCoord);
}
