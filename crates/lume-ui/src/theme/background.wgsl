struct Uniforms {
    c0: vec4<f32>,
    c1: vec4<f32>,
    c2: vec4<f32>,
    c3: vec4<f32>,
    // x = time_s, y = scrim_alpha, z = acrylic_alpha (1.0 = opaque backdrop, < 1.0 = translucent tint)
    params: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> u: Uniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VertexOutput {
    var out: VertexOutput;
    let x = f32((idx << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(idx & 2u) * 2.0 - 1.0;
    out.position = vec4<f32>(x, -y, 0.0, 1.0);
    out.uv = vec2<f32>(f32((idx << 1u) & 2u) * 0.5, f32(idx & 2u) * 0.5);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = u.params.x * 0.16;
    let uv = in.uv;

    let p0 = vec2<f32>(0.22 + 0.12 * sin(t), 0.28 + 0.10 * cos(t * 0.8));
    let p1 = vec2<f32>(0.78 + 0.10 * cos(t * 0.7), 0.26 + 0.12 * sin(t * 0.9));
    let p2 = vec2<f32>(0.28 + 0.11 * cos(t * 0.6), 0.76 + 0.10 * sin(t * 1.1));
    let p3 = vec2<f32>(0.76 + 0.12 * sin(t * 0.5), 0.78 + 0.11 * cos(t * 0.75));

    let w0 = exp(-4.2 * dot(uv - p0, uv - p0));
    let w1 = exp(-4.2 * dot(uv - p1, uv - p1));
    let w2 = exp(-4.2 * dot(uv - p2, uv - p2));
    let w3 = exp(-4.2 * dot(uv - p3, uv - p3));
    let w_sum = max(w0 + w1 + w2 + w3, 0.0001);

    let mesh = (u.c0.rgb * w0 + u.c1.rgb * w1 + u.c2.rgb * w2 + u.c3.rgb * w3) / w_sum;

    // Gentle vignette + adaptive dark contrast scrim clamped in [0.38, 0.55].
    let vignette = 1.0 - 0.18 * dot(uv - vec2<f32>(0.5, 0.5), uv - vec2<f32>(0.5, 0.5));
    let scrim = clamp(u.params.y, 0.38, 0.55);
    let darkened = mesh * vignette * (1.0 - scrim);

    let alpha = clamp(u.params.z, 0.0, 1.0);
    return vec4<f32>(darkened, alpha);
}
