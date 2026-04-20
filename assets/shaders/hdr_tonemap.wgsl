// ─── HDR Tone Mapping (PQ / HLG / Reinhard) ─────────────────────────────────
// Appliqué en post-process sur la texture linéaire HDR10.

@group(0) @binding(0) var hdr_tex: texture_2d<f32>;
@group(0) @binding(1) var samp:    sampler;

struct ToneMapParams {
    mode:          u32,   // 0=PQ Reinhard, 1=ACES, 2=Hable
    max_luminance: f32,   // nits max (ex: 1000.0, 4000.0, 10000.0)
    exposure:      f32,
    _pad:          f32,
};
@group(1) @binding(0) var<uniform> params: ToneMapParams;

// ACES filmic tone mapping (approximation de Krzysztof Narkowicz)
fn aces_filmic(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

// Hable/Uncharted 2
fn hable_partial(x: vec3<f32>) -> vec3<f32> {
    let A = 0.15; let B = 0.50; let C = 0.10;
    let D = 0.20; let E = 0.02; let F = 0.30;
    return ((x * (A * x + C * B) + D * E) / (x * (A * x + B) + D * F)) - vec3<f32>(E / F);
}
fn hable(v: vec3<f32>) -> vec3<f32> {
    let w      = vec3<f32>(11.2);
    let denom  = hable_partial(w);
    return hable_partial(v * 2.0) / denom;
}

// Reinhard simple
fn reinhard(x: vec3<f32>) -> vec3<f32> {
    return x / (x + vec3<f32>(1.0));
}

// PQ inverse EOTF (SMPTE ST.2084) — linéaire vers affichage SDR
fn pq_to_linear(N: vec3<f32>) -> vec3<f32> {
    let m1 = 0.1593017578125;
    let m2 = 78.84375;
    let c1 = 0.8359375;
    let c2 = 18.8515625;
    let c3 = 18.6875;
    let Np = pow(N, vec3<f32>(1.0 / m2));
    return pow(max(Np - c1, vec3<f32>(0.0)) / (c2 - c3 * Np), vec3<f32>(1.0 / m1));
}

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex fn vs_main(@builtin(vertex_index) vi: u32) -> VOut {
    let x = f32((vi & 1u) * 2u) - 1.0;
    let y = 1.0 - f32((vi >> 1u) * 2u);
    return VOut(vec4<f32>(x, y, 0.0, 1.0), vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5));
}

@fragment fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    var hdr = textureSample(hdr_tex, samp, in.uv).rgb;

    // Normalise la luminance par rapport au max display
    hdr = hdr * (params.exposure / params.max_luminance * 10000.0);

    var ldr: vec3<f32>;
    switch params.mode {
        case 1u:  { ldr = aces_filmic(hdr); }
        case 2u:  { ldr = hable(hdr); }
        default:  { ldr = reinhard(hdr); }
    }

    // Gamma sRGB (2.2 approximation)
    ldr = pow(clamp(ldr, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(1.0 / 2.2));
    return vec4<f32>(ldr, 1.0);
}
