struct ProjectionUniform {
    projection: mat4x4<f32>,
};
@group(0) @binding(0)
var<uniform> projection: ProjectionUniform;

struct VertexInput {
    @location(0) position: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) rect_pos: vec2<f32>,
    @location(2) rect_size: vec2<f32>,
    @location(3) rect_color: vec4<f32>,
    @location(4) border_radius: vec4<f32>,
    @location(5) border_size: f32,
    @location(6) border_color: vec4<f32>,
    @location(7) scale: f32,
};

struct InstanceInput {
    @location(1) rect_pos: vec2<f32>,
    @location(2) rect_size: vec2<f32>,
    @location(3) rect_color: vec4<f32>,
    @location(4) border_radius: vec4<f32>,
    @location(5) border_size: f32,
    @location(6) border_color: vec4<f32>,
    @location(7) scale: f32,
}

@vertex
fn vs_main(
    model: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;

    let position = model.position * (instance.rect_size + instance.border_size * 2.0) * instance.scale + instance.rect_pos * instance.scale;
    out.clip_position = projection.projection * vec4<f32>(position, 0.0, 1.0);
    out.uv = position;
    out.rect_pos = (instance.rect_pos + instance.border_size) * instance.scale;
    out.rect_size = instance.rect_size * instance.scale;
    out.rect_color = instance.rect_color;

    let outer_max_radius = min(
        instance.rect_size.x + instance.border_size * 2.0,
        instance.rect_size.y + instance.border_size * 2.0
    ) * 0.5;
    out.border_radius = vec4<f32>(
        min(instance.border_radius.x + instance.border_size, outer_max_radius),
        min(instance.border_radius.y + instance.border_size, outer_max_radius),
        min(instance.border_radius.z + instance.border_size, outer_max_radius),
        min(instance.border_radius.w + instance.border_size, outer_max_radius)
    ) * instance.scale;

    out.border_size = instance.border_size * instance.scale;
    out.border_color = instance.border_color;
    out.scale = instance.scale;

    return out;
}

fn sdf_rounded_rect(p: vec2<f32>, b: vec2<f32>, r: vec4<f32>) -> f32 {
    var x = r.x;
    var y = r.y;
    x = select(r.z, r.x, p.x > 0.0);
    y = select(r.w, r.y, p.x > 0.0);
    x = select(y, x, p.y > 0.0);
    let q = abs(p) - b + x;
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - x;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Calculate inner rectangle SDF
    let inner_center = in.rect_pos + in.rect_size / 2.0;
    let inner_dist = sdf_rounded_rect(in.uv - inner_center, in.rect_size / 2.0, in.border_radius);
    
    // Calculate outer border SDF with proper radius adjustment
    let outer_size = in.rect_size + 2.0 * in.border_size;
    let outer_center = in.rect_pos - in.border_size + outer_size / 2.0;
    let outer_dist = sdf_rounded_rect(in.uv - outer_center, outer_size / 2.0, in.border_radius);

    // Compute anti-aliasing
    let inner_aa = fwidth(inner_dist);
    let outer_aa = fwidth(outer_dist);
    
    // Calculate alpha values
    let inner_alpha = smoothstep(-inner_aa, inner_aa, -inner_dist);
    let outer_alpha = smoothstep(-outer_aa, outer_aa, -outer_dist);
    let border_alpha = outer_alpha - inner_alpha;

    // Blend colors
    let inner_color = in.rect_color * inner_alpha;
    let border_color = in.border_color * border_alpha;
    
    return inner_color + border_color;
}
