// ─── Vertex shader ──────────────────────────────────────────────────────────

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
};

// Quad plein écran généré sans vertex buffer (4 sommets TriangleStrip)
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

@group(0) @binding(0) var y_tex:  texture_2d<f32>;
@group(0) @binding(1) var u_tex:  texture_2d<f32>;
@group(0) @binding(2) var v_tex:  texture_2d<f32>;
@group(0) @binding(3) var samp:   sampler;

struct ColorTransform {
    matrix: mat4x4<f32>,
    offset: vec4<f32>,
};
@group(1) @binding(0) var<uniform> color: ColorTransform;

// ─── Fragment shader ────────────────────────────────────────────────────────

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let y = textureSample(y_tex, samp, in.tex_coord).r;
    let u = textureSample(u_tex, samp, in.tex_coord).r - 0.5;
    let v = textureSample(v_tex, samp, in.tex_coord).r - 0.5;

    // Appliquer la matrice couleur (BT.601 / BT.709 / BT.2020 configurable)
    let yuv = vec4<f32>(y, u, v, 1.0);
    var rgb = (color.matrix * yuv).rgb + color.offset.rgb;

    // Clamp [0, 1] — la tonemap HDR est dans hdr_tonemap.wgsl pour les passes HDR
    rgb = clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0));

    return vec4<f32>(rgb, 1.0);
}
