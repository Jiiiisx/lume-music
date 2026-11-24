struct Uniforms {
    c0: vec4<f32>,
    c1: vec4<f32>,
    c2: vec4<f32>,
    c3: vec4<f32>,
    // x = art_tint_strength (0..1), y = scrim_alpha, z = acrylic_alpha, w = has_art (>0.5 when cover texture bound)
    params: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> u: Uniforms;

@group(0) @binding(1)
var art_tex: texture_2d<f32>;

@group(0) @binding(2)
var art_sampler: sampler;

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

fn hash12(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn boost_saturation(rgb: vec3<f32>, sat: f32) -> vec3<f32> {
    let luma = dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    return clamp(mix(vec3<f32>(luma), rgb, sat), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn sample_blurred_art(uv: vec2<f32>) -> vec3<f32> {
    let center = (uv - vec2<f32>(0.5, 0.5)) * 0.78 + vec2<f32>(0.5, 0.5);

    var acc = textureSampleLevel(art_tex, art_sampler, center, 0.0).rgb * 0.16;
    let r1 = 0.055;
    let r2 = 0.115;

    for (var i = 0u; i < 6u; i = i + 1u) {
        let ang = f32(i) * 1.0471975;
        let dir = vec2<f32>(cos(ang), sin(ang));
        acc += textureSampleLevel(art_tex, art_sampler, clamp(center + dir * r1, vec2<f32>(0.02), vec2<f32>(0.98)), 0.0).rgb * 0.085;
        let dir2 = vec2<f32>(cos(ang + 0.5236), sin(ang + 0.5236));
        acc += textureSampleLevel(art_tex, art_sampler, clamp(center + dir2 * r2, vec2<f32>(0.02), vec2<f32>(0.98)), 0.0).rgb * 0.055;
    }

    return boost_saturation(acc, 1.12);
}

fn render_crystal_surface(uv: vec2<f32>) -> vec4<f32> {
    // Static pure neutral clear crystal sheen (zero blue hue, zero motion).
    let diag = clamp((1.0 - uv.x) * 0.45 + (1.0 - uv.y) * 0.55, 0.0, 1.0);
    let top_rim = exp(-7.5 * uv.y);

    let g0 = exp(-5.2 * dot(uv - vec2<f32>(0.22, 0.18), uv - vec2<f32>(0.22, 0.18)));
    let g1 = exp(-6.4 * dot(uv - vec2<f32>(0.80, 0.26), uv - vec2<f32>(0.80, 0.26)));

    let pure_white = vec3<f32>(0.97, 0.97, 0.99);
    let mist = vec3<f32>(0.78, 0.79, 0.82);

    var col = mix(vec3<f32>(0.08, 0.08, 0.09), mist * 0.40, diag * 0.45);
    col += pure_white * (top_rim * 0.14 + g0 * 0.14 + g1 * 0.10);

    let local_glow = clamp(top_rim * 0.35 + g0 * 0.30 + g1 * 0.24, 0.0, 1.0);
    return vec4<f32>(clamp(col, vec3<f32>(0.0), vec3<f32>(1.0)), local_glow);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;

    let crystal = render_crystal_surface(uv);
    var color = crystal.rgb;

    // Subtle album art ambient reflection only when playing a track with artwork.
    let art_strength = clamp(u.params.x, 0.0, 0.85);
    if u.params.w > 0.5 && art_strength > 0.001 {
        let art = sample_blurred_art(uv);
        color = mix(color, art, art_strength);
    }

    // Gentle radial vignette, full adaptive scrim, and WCAG luminance floor so white text
    // stays readable (>= 4.5:1) even over pure-white or neon-yellow album covers.
    let centered = uv - vec2<f32>(0.5, 0.5);
    let vignette = 1.0 - 0.12 * dot(centered, centered);
    let scrim = clamp(u.params.y, 0.38, 0.55);
    var darkened = color * vignette * (1.0 - scrim);
    let bg_luma = dot(darkened, vec3<f32>(0.2126, 0.7152, 0.0722));
    let max_luma = 0.15;
    if bg_luma > max_luma {
        darkened = darkened * (max_luma / bg_luma);
    }

    let dither = (hash12(in.position.xy) - 0.5) / 255.0;
    let final_rgb = clamp(darkened + vec3<f32>(dither), vec3<f32>(0.0), vec3<f32>(1.0));

    let base_alpha = clamp(u.params.z, 0.0, 1.0);
    var alpha = base_alpha;
    if base_alpha < 0.40 {
        let art_lift = select(0.0, art_strength * 0.55, u.params.w > 0.5);
        alpha = base_alpha * (0.28 + 0.72 * clamp(crystal.w + art_lift, 0.0, 1.0));
    }
    return vec4<f32>(final_rgb * alpha, alpha);
}
