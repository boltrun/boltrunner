#import bevy_pbr::forward_io::VertexOutput
#ifdef MAP_ANIMATED
#import bevy_pbr::mesh_view_bindings::globals
#endif

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> settings: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var diffuse_map: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var diffuse_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var lightmap: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var lightmap_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var glow_map: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var glow_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(7) var<uniform> flow: vec4<f32>;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var uv = in.uv;
#ifdef MAP_ANIMATED
    let time = globals.time;
    uv += fract(flow.xy * time);
#endif
#ifdef MAP_WATER
    // Two overlapping currents break up repetition without moving the baked
    // shadows. The slower layer remains aligned with the authored flow.
    let ripple = vec2<f32>(
        sin(in.uv.y * 9.0 + time * 0.9),
        cos(in.uv.x * 11.0 - time * 0.7)
    ) * 0.014;
    uv += ripple;
    let detail_uv = in.uv * vec2<f32>(1.37, 1.19)
        + fract(flow.xy * time * 0.61) - ripple;
    let detail = textureSample(diffuse_map, diffuse_sampler, detail_uv);
    var diffuse = mix(textureSample(diffuse_map, diffuse_sampler, uv), detail, 0.24);
    let crest = sin((uv.x + uv.y) * 17.0 + time * 0.6)
        * sin((detail_uv.x - detail_uv.y) * 13.0 - time * 0.4);
    diffuse = vec4<f32>(diffuse.rgb * (1.0 + 0.04 * crest), diffuse.a);
#else
    let diffuse = textureSample(diffuse_map, diffuse_sampler, uv);
#endif
#ifdef MAP_MASKED
    if diffuse.a < settings.z { discard; }
#endif
    // All color textures are uploaded as sRGB; sampling decodes them to linear.
    // Level 0 prevents light bleeding between islands in the baked atlas.
    let irradiance = textureSampleLevel(lightmap, lightmap_sampler, in.uv_b, 0.0).rgb;
#ifdef MAP_GLOW
    let glow = textureSample(glow_map, glow_sampler, uv).rgb * settings.y;
#else
    let glow = vec3<f32>(0.0);
#endif
#ifdef MAP_OPAQUE
    let alpha = 1.0;
#else
    let alpha = diffuse.a * in.color.a;
#endif
    return vec4<f32>(diffuse.rgb * irradiance * in.color.rgb * settings.x + glow, alpha);
}
