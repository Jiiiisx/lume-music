struct Uniforms {
    c0: vec4<f32>,
    c1: vec4<f32>,
    c2: vec4<f32>,
    c3: vec4<f32>,
    // x = time_s, y = scrim_alpha, z = acrylic_alpha, w = has_art (>0.5 when cover texture bound)
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

fn sample_blurred_art(uv: vec2<f32>, t: f32) -> vec3<f32> {
    // Gentle centered zoom + organic drift so edge clamping never stretches pixels.
    let drift = vec2<f32>(0.024 * sin(t * 0.55), 0.024 * cos(t * 0.45));
    let center = (uv - vec2<f32>(0.5, 0.5)) * 0.76 + vec2<f32>(0.5, 0.5) + drift;

    var acc = textureSampleLevel(art_tex, art_sampler, center, 0.0).rgb * 0.16;
    let r1 = 0.055;
    let r2 = 0.115;

    for (var i = 0u; i < 6u; i = i + 1u) {
        let ang = f32(i) * 1.0471975 + t * 0.15;
        let dir = vec2<f32>(cos(ang), sin(ang));
        acc += textureSampleLevel(art_tex, art_sampler, clamp(center + dir * r1, vec2<f32>(0.02), vec2<f32>(0.98)), 0.0).rgb * 0.085;
        let dir2 = vec2<f32>(cos(ang + 0.5236), sin(ang + 0.5236));
        acc += textureSampleLevel(art_tex, art_sampler, clamp(center + dir2 * r2, vec2<f32>(0.02), vec2<f32>(0.98)), 0.0).rgb * 0.055;
    }

    return boost_saturation(acc, 1.24);
}

fn render_luminous_mesh(uv: vec2<f32>, t: f32) -> vec3<f32> {
    // Deep studio base gradient so frosted glass has contrast and warmth even at rest.
    let base = mix(u.c2.rgb * 0.72, u.c1.rgb * 0.82, clamp(uv.x * 0.55 + (1.0 - uv.y) * 0.45, 0.0, 1.0));

    let p0 = vec2<f32>(0.18 + 0.10 * sin(t * 0.7), 0.24 + 0.09 * cos(t * 0.6));
    let p1 = vec2<f32>(0.76 + 0.11 * cos(t * 0.5), 0.30 + 0.10 * sin(t * 0.8));
    let p2 = vec2<f32>(0.34 + 0.10 * cos(t * 0.65), 0.78 + 0.09 * sin(t * 0.55));
    let p3 = vec2<f32>(0.82 + 0.09 * sin(t * 0.45), 0.76 + 0.10 * cos(t * 0.7));

    let d0 = dot(uv - p0, uv - p0);
    let d1 = dot(uv - p1, uv - p1);
    let d2 = dot(uv - p2, uv - p2);
    let d3 = dot(uv - p3, uv - p3);

    let g0 = exp(-3.4 * d0);
    let g1 = exp(-3.0 * d1);
    let g2 = exp(-3.6 * d2);
    let g3 = exp(-3.2 * d3);

    // Soft studio specular bloom orb near top-center/right like warm glass photography.
    let bloom_p = vec2<f32>(0.54 + 0.08 * sin(t * 0.4), 0.22 + 0.06 * cos(t * 0.35));
    let bloom = exp(-5.2 * dot(uv - bloom_p, uv - bloom_p));

    var col = base;
    col = mix(col, u.c0.rgb, g0 * 0.78);
    col = mix(col, u.c3.rgb, g1 * 0.72);
    col = mix(col, u.c2.rgb * 1.15, g2 * 0.58);
    col = mix(col, u.c1.rgb * 1.20, g3 * 0.65);
    col += u.c0.rgb * bloom * 0.28;

    return boost_saturation(col, 1.12);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = u.params.x * 0.18;
    let uv = in.uv;

    var color: vec3<f32>;
    if u.params.w > 0.5 {
        let art = sample_blurred_art(uv, t);
        let mesh = render_luminous_mesh(uv, t);
        color = mix(mesh, art, 0.82);
    } else {
        color = render_luminous_mesh(uv, t);
    }

    // Subtle soft radial vignette & adaptive contrast floor so white glass text always pops.
    let centered = uv - vec2<f32>(0.5, 0.5);
    let vignette = 1.0 - 0.16 * dot(centered, centered);
    let scrim = clamp(u.params.y, 0.38, 0.55);
    let darkened = color * vignette * (1.0 - scrim * 0.48);

    // Subtle micro-dither prevents smooth gradient banding.
    let dither = (hash12(in.position.xy) - 0.5) / 255.0;
    let final_rgb = clamp(darkened + vec3<f32>(dither), vec3<f32>(0.0), vec3<f32>(1.0));

    let alpha = clamp(u.params.z, 0.0, 1.0);
    return vec4<f32>(final_rgb, alpha);
}
