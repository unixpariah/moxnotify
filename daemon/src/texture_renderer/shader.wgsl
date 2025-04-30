struct InstanceInput {
    @location(2) pos: vec2<f32>,
    @location(3) size: vec2<f32>,
    @location(4) radius: vec4<f32>,
    @location(5) container_rect: vec4<f32>,
    @location(6) border_width: vec4<f32>,
    @location(7) scale: f32,
};

struct VertexInput {
    @location(0) position: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) layer: u32,
    @location(2) radius: vec4<f32>,
    @location(3) size: vec2<f32>,
    @location(4) container_rect: vec4<f32>,
    @location(5) surface_position: vec2<f32>,
    @location(6) border_width: vec4<f32>,
};

struct ProjectionUniform {
    view_proj: mat4x4<f32>,
};
@group(1) @binding(0)
var<uniform> projection: ProjectionUniform;

@vertex
fn vs_main(
    model: VertexInput,
    instance: InstanceInput,
    @builtin(instance_index) instance_idx: u32,
) -> VertexOutput {
    var out: VertexOutput;

    let scaled_size = instance.size * instance.scale;
    let position = model.position * scaled_size + instance.pos;

    out.clip_position = projection.view_proj * vec4<f32>(position, 0.0, 1.0);
    out.tex_coords = model.position;
    out.layer = instance_idx;

    let scaled_radius = instance.radius * instance.scale;
    let max_radius = min(scaled_size.x, scaled_size.y) * 0.5;
    out.radius = vec4<f32>(
        min(scaled_radius.x, max_radius),
        min(scaled_radius.y, max_radius),
        min(scaled_radius.z, max_radius),
        min(scaled_radius.w, max_radius)
    );

    out.size = scaled_size;
    out.container_rect = instance.container_rect;
    out.surface_position = position;
    out.border_width = instance.border_width * instance.scale;

    return out;
}

@group(0) @binding(0)
var t_diffuse: texture_2d_array<f32>; 
@group(0) @binding(1)
var s_diffuse: sampler;

fn sdf_rounded_rect(p: vec2<f32>, b: vec2<f32>, r: vec4<f32>) -> f32 {
    var x = select(r.x, r.y, p.x > 0.0);
    var y = select(r.z, r.w, p.x > 0.0);
    let radius = select(y, x, p.y > 0.0);
    let q = abs(p) - b + radius;
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - radius;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(
        t_diffuse,
        s_diffuse,
        vec2<f32>(in.tex_coords.x, 1.0 - in.tex_coords.y),
        i32(in.layer)
    );

    let half_extent = in.size / 2.0;
    let p = (in.tex_coords - 0.5) * in.size;
    let d = sdf_rounded_rect(p, half_extent, in.radius);

    let aa = fwidth(d) * 0.6;

    let outer = smoothstep(-aa, aa, -d);
    let inner = smoothstep(-aa, aa, -(d + in.border_width.x));
    let border_alpha = clamp(outer - inner, 0.0, 1.0);

    let color = mix(tex_color, vec4<f32>(0., 0., 0., 0.), border_alpha);
    let alpha = outer;

    let final_alpha = color.a * alpha;
    return vec4<f32>(color.rgb * final_alpha, final_alpha);
}
