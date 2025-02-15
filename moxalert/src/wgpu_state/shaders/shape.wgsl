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
    @location(8) rotation: f32,
};

struct InstanceInput {
    @location(1) rect_pos: vec2<f32>,
    @location(2) rect_size: vec2<f32>,
    @location(3) rect_color: vec4<f32>,
    @location(4) border_radius: vec4<f32>,
    @location(5) border_size: f32,
    @location(6) border_color: vec4<f32>,
    @location(7) scale: f32,
    @location(8) rotation: f32,
}

fn rotation_matrix(angle: f32) -> mat2x2<f32> {
    let angle_inner = angle * 3.14159265359 / 180.0;
    let sinTheta = sin(angle_inner);
    let cosTheta = cos(angle_inner);
    return mat2x2<f32>(
        cosTheta, -sinTheta,
        sinTheta, cosTheta
    );
}

@vertex
fn vs_main(
    model: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;

    let position = model.position * (instance.rect_size + instance.border_size * 2.0) * instance.scale + instance.rect_pos * instance.scale;

    out.clip_position = projection.projection * vec4<f32>(position * rotation_matrix(instance.rotation), 0.0, 1.0);

    out.uv = position;

    out.rect_pos = (instance.rect_pos + instance.border_size * 1.0) * instance.scale;
    out.rect_size = instance.rect_size * instance.scale;
    out.rect_color = instance.rect_color;
    out.border_radius = instance.border_radius * instance.scale;
    out.border_size = instance.border_size * instance.scale;
    out.border_color = instance.border_color;
    out.scale = instance.scale;
    out.rotation = instance.rotation;

    return out;
}

// MIT License. © 2023 Inigo Quilez, Munrocket
// https://gist.github.com/munrocket/30e645d584b5300ee69295e54674b3e4
// https://compute.toys/view/398
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
    var pos: vec2<f32> = in.rect_pos;
    var size: vec2<f32> = in.rect_size;
    var dist: f32 = sdf_rounded_rect(
        in.uv - pos - (size / 2.0),
        size / 2.0,
        in.border_radius
    );
    
    let rect_aa = fwidth(dist);
    let rect_alpha = smoothstep(rect_aa, -rect_aa, dist);
    var color: vec4<f32> = vec4<f32>(in.rect_color.rgb, in.rect_color.a * rect_alpha);

    size += in.border_size * 2.0;
    pos -= in.border_size;
    let border_dist = sdf_rounded_rect(
        in.uv - pos - (size / 2.0),
        size / 2.0,
        in.border_radius
    );
    
    let border_aa = fwidth(border_dist);
    let border_alpha = smoothstep(border_aa, -border_aa, border_dist);
    let border_color = vec4<f32>(in.border_color.rgb, in.border_color.a * border_alpha);
    
    let mix_aa = fwidth(dist);
    let mix_factor = smoothstep(-mix_aa, mix_aa, dist);
    color = mix(color, border_color, mix_factor);

    return color;
}
