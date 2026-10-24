#import bevy_pbr::forward_io::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> parameters: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> dimensions: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> color_bottom: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var<uniform> color_top: vec4<f32>;

fn box_distance(p: vec2<f32>, half_size: vec2<f32>) -> f32 {
    let d = abs(p) - half_size;
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0);
}

fn segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let ab = b - a;
    return length(p - a - ab * clamp(dot(p - a, ab) / dot(ab, ab), 0.0, 1.0));
}

fn polygon(p: vec2<f32>, vertices: array<vec2<f32>, 8>, count: u32) -> f32 {
    var distance = 1000000.0;
    var inside = false;
    var j = count - 1u;
    for (var i = 0u; i < count; i += 1u) {
        let a = vertices[i];
        let b = vertices[j];
        distance = min(distance, segment(p, a, b));
        if (a.y > p.y) != (b.y > p.y) {
            if p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
                inside = !inside;
            }
        }
        j = i;
    }
    return select(distance, -distance, inside);
}

fn coverage(distance: f32, aa: f32) -> f32 {
    return 1.0 - smoothstep(-aa, aa, distance);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let glow = parameters.w > 0.5;
    let padding = select(0.0, dimensions.z, glow);
    let uv = (in.uv - vec2(0.5)) * (dimensions.xy + 2.0 * padding) / dimensions.xy + vec2(0.5);
    let p = uv - vec2<f32>(0.5);
    let aa = max(fwidth(p.x), fwidth(p.y)) * 0.75;
    let kind = u32(parameters.x + 0.5);
    let progress = parameters.y;
    let tint = mix(color_bottom.rgb, color_top.rgb, 1.0 - clamp(uv.y, 0.0, 1.0));
    var alpha = 0.0;
    var d = 1.0;
    var glow_distance = 0.0;
    var glow_strength = 1.0;

    if kind == 0u {
        let radius = length(p);
        let ring = coverage(abs(radius - 0.445) - 0.028, aa);
        let angle = fract(atan2(p.x, -p.y) / 6.2831853 + 1.0);
        alpha = ring * select(0.16, 1.0, angle <= progress);
        alpha += coverage(abs(radius - 0.365) - 0.006, aa) * 0.2;
        glow_distance = (abs(radius - 0.445) - 0.028) * dimensions.x;
        glow_strength = select(0.16, 1.0, angle <= progress);
    } else if kind == 1u {
        d = polygon(p, array<vec2<f32>, 8>(
            vec2(-0.44, -0.42), vec2(-0.03, -0.42), vec2(0.44, 0.0),
            vec2(-0.03, 0.42), vec2(-0.44, 0.42), vec2(0.02, 0.0),
            vec2(0.02, 0.0), vec2(0.02, 0.0)), 6u);
        let fill = coverage(d, aa);
        let outline = coverage(abs(d) - 0.026, aa);
        let charged = 1.0 - smoothstep(progress - aa, progress + aa, uv.x);
        alpha = max(outline * 0.42, fill * charged);
        let fill_distance = max(d, uv.x - progress);
        let outline_distance = abs(d) - 0.026;
        glow_distance = min(fill_distance, outline_distance) * min(dimensions.x, dimensions.y);
        glow_strength = select(0.42, 1.0, fill_distance <= outline_distance);
    } else if kind == 2u {
        let square = box_distance(p, vec2(0.40)) - 0.065;
        let cross = min(box_distance(p, vec2(0.085, 0.29)), box_distance(p, vec2(0.29, 0.085)));
        d = max(square, -cross);
        alpha = coverage(d, aa);
    } else if kind == 3u {
        for (var i = 0u; i < 3u; i += 1u) {
            let q = p - vec2((f32(i) - 1.0) * 0.29, 0.0);
            let body = box_distance(q - vec2(0.0, 0.07), vec2(0.083, 0.29));
            let tip = max(abs(q.x) * 1.7 + (-q.y - 0.21) - 0.16, max(q.y + 0.21, -q.y - 0.38));
            d = min(d, min(body, tip));
        }
    } else if kind == 4u {
        d = polygon(p, array<vec2<f32>, 8>(
            vec2(0.06, -0.46), vec2(-0.33, 0.08), vec2(-0.04, 0.08),
            vec2(-0.15, 0.46), vec2(0.34, -0.10), vec2(0.06, -0.10),
            vec2(0.06, -0.46), vec2(0.06, -0.46)), 6u);
    } else if kind == 5u {
        d = polygon(p, array<vec2<f32>, 8>(
            vec2(-0.43, -0.27), vec2(-0.20, -0.05), vec2(0.0, -0.40),
            vec2(0.20, -0.05), vec2(0.43, -0.27), vec2(0.33, 0.25),
            vec2(-0.33, 0.25), vec2(-0.43, -0.27)), 7u);
        d = min(d, box_distance(p - vec2(0.0, 0.37), vec2(0.33, 0.035)));
    } else if kind == 6u {
        d = min(segment(p, vec2(-0.35, -0.34), vec2(0.35, 0.0)), segment(p, vec2(-0.35, 0.34), vec2(0.35, 0.0))) - 0.085;
    } else if kind == 7u {
        let q = p * dimensions.xy;
        let radius = 6.0;
        let edge = box_distance(q, dimensions.xy * 0.5 - vec2(radius + 2.0)) - radius;
        let pixel_aa = max(fwidth(q.x), fwidth(q.y)) * 0.75;
        let body = coverage(edge, pixel_aa);
        let border = coverage(abs(edge) - 2.0, pixel_aa);
        let fill = 1.0 - smoothstep(progress - fwidth(uv.x), progress + fwidth(uv.x), uv.x);
        alpha = max(border, body * fill);
        glow_distance = min(abs(edge) - 2.0, max(edge, (uv.x - progress) * dimensions.x));
    } else {
        alpha = 0.3;
        glow_distance = box_distance(p * dimensions.xy, dimensions.xy * 0.5);
        glow_strength = 0.3;
    }
    if kind >= 2u && kind <= 6u {
        alpha = coverage(d, aa);
        glow_distance = d * min(dimensions.x, dimensions.y);
    }
    if glow {
        let distance = max(glow_distance, 0.0) / (dimensions.z * 0.4);
        alpha = exp(-distance * distance) * 0.20 * glow_strength;
        return vec4<f32>(tint * alpha * parameters.z, 0.0);
    }
    return vec4<f32>(tint, alpha * parameters.z);
}
