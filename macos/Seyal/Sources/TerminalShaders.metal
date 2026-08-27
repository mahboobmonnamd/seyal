#include <metal_stdlib>
using namespace metal;

struct TerminalInstance {
    float2 origin;
    float2 size;
    float4 uv_rect;
    uint foreground;
    uint background;
    uint flags;
    uint atlas_slice;
};

struct TerminalVertexOut {
    float4 position [[position]];
    float2 local;
    float2 uv;
    uint foreground [[flat]];
    uint background [[flat]];
    uint flags [[flat]];
    uint atlas_slice [[flat]];
};

static float4 unpack_rgba8(uint packed) {
    return float4(
        float(packed & 0xffu),
        float((packed >> 8) & 0xffu),
        float((packed >> 16) & 0xffu),
        float((packed >> 24) & 0xffu)
    ) / 255.0;
}

vertex TerminalVertexOut seyal_terminal_vertex(
    uint vertex_id [[vertex_id]],
    uint instance_id [[instance_id]],
    const device TerminalInstance *instances [[buffer(0)]],
    constant float2 &viewport [[buffer(1)]])
{
    constexpr float2 corners[6] = {
        float2(0.0, 0.0),
        float2(1.0, 0.0),
        float2(0.0, 1.0),
        float2(0.0, 1.0),
        float2(1.0, 0.0),
        float2(1.0, 1.0),
    };

    const TerminalInstance instance = instances[instance_id];
    const float2 local = corners[vertex_id];
    const float2 pixel = instance.origin + local * instance.size;
    const float2 ndc = float2(
        pixel.x / max(viewport.x, 1.0) * 2.0 - 1.0,
        1.0 - pixel.y / max(viewport.y, 1.0) * 2.0
    );

    TerminalVertexOut out;
    out.position = float4(ndc, 0.0, 1.0);
    out.local = local;
    out.uv = mix(instance.uv_rect.xy, instance.uv_rect.zw, local);
    out.foreground = instance.foreground;
    out.background = instance.background;
    out.flags = instance.flags;
    out.atlas_slice = instance.atlas_slice;
    return out;
}

fragment float4 seyal_terminal_fragment(
    TerminalVertexOut in [[stage_in]],
    texture2d_array<float> glyph_atlas [[texture(0)]],
    sampler glyph_sampler [[sampler(0)]])
{
    constexpr uint glyph_flag = 1u << 0;
    constexpr uint underline_flag = 1u << 1;
    constexpr uint cursor_flag = 1u << 2;

    float4 foreground = unpack_rgba8(in.foreground);
    float4 background = unpack_rgba8(in.background);
    if ((in.flags & cursor_flag) != 0u) {
        const float4 temporary = foreground;
        foreground = background;
        background = temporary;
    }

    float4 color = background;
    if ((in.flags & glyph_flag) != 0u) {
        const float coverage = glyph_atlas.sample(glyph_sampler, in.uv, in.atlas_slice).r;
        color = mix(color, foreground, coverage);
    }
    if ((in.flags & underline_flag) != 0u && in.local.y >= 0.88) {
        color = foreground;
    }
    return color;
}
