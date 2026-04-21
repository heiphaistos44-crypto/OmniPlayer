// ─── Vertex shader ──────────────────────────────────────────────────────────

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    let x = f32((vi & 1u) * 2u) - 1.0;
    let y = 1.0 - f32((vi >> 1u) * 2u);
    var out: VertexOutput;
    out.position  = vec4<f32>(x, y, 0.0, 1.0);
    out.tex_coord = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

// ─── Bindings ───────────────────────────────────────────────────────────────

@group(0) @binding(0) var y_tex: texture_2d<f32>;
@group(0) @binding(1) var u_tex: texture_2d<f32>;
@group(0) @binding(2) var v_tex: texture_2d<f32>;
@group(0) @binding(3) var samp:  sampler;

// Kept for API compatibility (future HDR / BT.2020 switch)
struct ColorTransform {
    matrix: mat4x4<f32>,
    offset: vec4<f32>,
};
@group(1) @binding(0) var<uniform> color: ColorTransform;

// ─── Fragment shader ────────────────────────────────────────────────────────

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let y_raw = textureSample(y_tex, samp, in.tex_coord).r;
    let u_raw = textureSample(u_tex, samp, in.tex_coord).r;
    let v_raw = textureSample(v_tex, samp, in.tex_coord).r;

    // BT.709 limited range (TV range) — standard for H.264 / H.265 / VP9
    // Y  ∈ [16/255, 235/255]  →  scale by 255/219
    // UV ∈ [16/255, 240/255], centre 128/255  →  scale by 255/224
    let y = (y_raw - 16.0 / 255.0) * (255.0 / 219.0);
    let u = (u_raw - 128.0 / 255.0) * (255.0 / 224.0);
    let v = (v_raw - 128.0 / 255.0) * (255.0 / 224.0);

    let r = clamp(y + 1.5748 * v,              0.0, 1.0);
    let g = clamp(y - 0.1873 * u - 0.4681 * v, 0.0, 1.0);
    let b = clamp(y + 1.8556 * u,              0.0, 1.0);

    return vec4<f32>(r, g, b, 1.0);
}
