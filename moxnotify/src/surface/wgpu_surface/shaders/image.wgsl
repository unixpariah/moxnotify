struct InstanceInput {
    @location(2) pos: vec2<f32>,
    @location(3) size: vec2<f32>,
    @location(4) radius: vec4<f32>,
    @location(5) container_rect: vec4<f32>,
    @location(6) border_width: f32,
    @location(7) border_color: vec4<f32>,
};

struct VertexInput {
    @location(0) position: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) layer: u32,
    @location(2) radius: vec4<f32>,
    @location(3) size: vec2<f32>,
    @location(4) container_rect: vec4<f32>,
    @location(5) surface_position: vec2<f32>,
    @location(6) border_width: f32,
    @location(7) border_color: vec4<f32>,
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
    
    let position = model.position * instance.size + instance.pos;
    
    out.clip_position = projection.view_proj * vec4<f32>(position, 0.0, 1.0);
    out.tex_coords = model.position;
    out.layer = instance_idx;
    
    let max_radius = min(instance.size.x, instance.size.y) * 0.5;
    out.radius = vec4<f32>(
        min(instance.radius.x, max_radius),
        min(instance.radius.y, max_radius),
        min(instance.radius.z, max_radius),
        min(instance.radius.w, max_radius)
    );
    
    out.size = instance.size; 
    out.container_rect = instance.container_rect;
    out.surface_position = position;
    out.border_width = instance.border_width;
    out.border_color = instance.border_color;
    
    return out;
}

@group(0) @binding(0)
var t_diffuse: texture_2d_array<f32>; 
@group(0) @binding(1)
var s_diffuse: sampler;

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
    let tex_color = textureSample(
        t_diffuse,
        s_diffuse,
        vec2<f32>(in.tex_coords.x, 1.0 - in.tex_coords.y),
        i32(in.layer)
    );

    let half_extent = in.size / 2.0;
    let p = (in.tex_coords - 0.5) * in.size;
    let d = sdf_rounded_rect(p, half_extent, in.radius);

    let aa = fwidth(d) * 0.75;
    let border_aa = aa * 2.0;

    let outer = smoothstep(-aa, aa, -d);
    let inner = smoothstep(-border_aa, border_aa, -(d + in.border_width));
    let border_alpha = clamp(outer - inner, 0.0, 1.0);
    
    let color = mix(tex_color, in.border_color, border_alpha);
    let alpha = outer; 

    let flipped_y = in.surface_position.y - 2.0 * in.tex_coords.y * in.size.y + in.size.y;
    let inside = (in.surface_position.x >= in.container_rect.x) && 
                 (in.surface_position.x <= in.container_rect.z) &&
                 (flipped_y >= in.container_rect.y) && 
                 (flipped_y <= in.container_rect.w);

    if (!inside) {
        discard;
    }

    return vec4<f32>(color.rgb, color.a * alpha);
}
