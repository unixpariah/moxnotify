use std::ops::Deref;
use wgpu::util::DeviceExt;

pub type Mat4 = [[f32; 4]; 4];

pub trait Matrix {
    fn projection(left: f32, right: f32, top: f32, bottom: f32) -> Self;
}

impl Matrix for Mat4 {
    fn projection(left: f32, right: f32, top: f32, bottom: f32) -> Self {
        [
            [2.0 / (right - left), 0.0, 0.0, 0.0],
            [0.0, 2.0 / (top - bottom), 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [
                -(right + left) / (right - left),
                -(top + bottom) / (top - bottom),
                0.0,
                1.0,
            ],
        ]
    }
}

pub struct IndexBuffer {
    buffer: wgpu::Buffer,
    indices: Box<[u16]>,
}

impl IndexBuffer {
    pub fn new(device: &wgpu::Device, indices: &[u16]) -> Self {
        Self {
            buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Buffer"),
                usage: wgpu::BufferUsages::INDEX,
                contents: bytemuck::cast_slice(indices),
            }),
            indices: indices.into(),
        }
    }

    pub fn size(&self) -> u32 {
        self.indices.len() as u32
    }
}

impl Deref for IndexBuffer {
    type Target = wgpu::Buffer;
    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub struct TextureInstance {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub radius: [f32; 4],
}

impl TextureInstance {
    const ATTRIBS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
        2 => Float32x2,
        3 => Float32x2,
        4 => Float32x4,
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub struct Instance {
    pub rect_pos: [f32; 2],
    pub rect_size: [f32; 2],
    pub rect_color: [f32; 4],
    pub border_radius: [f32; 4],
    pub border_size: f32,
    pub border_color: [f32; 4],
    pub scale: f32,
    pub rotation: f32,
}

impl Instance {
    const ATTRIBS: [wgpu::VertexAttribute; 8] = wgpu::vertex_attr_array![
        1 => Float32x2,
        2 => Float32x2,
        3 => Float32x4,
        4 => Float32x4,
        5 => Float32,
        6 => Float32x4,
        7 => Float32,
        8 => Float32,
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

pub struct InstanceBuffer<T> {
    pub buffer: wgpu::Buffer,
    pub instances: Box<[T]>,
}

impl<T> InstanceBuffer<T> {
    pub fn new_with_size(device: &wgpu::Device, size_bytes: usize) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Instance Buffer"),
            size: size_bytes as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        InstanceBuffer {
            buffer,
            instances: Box::new([]),
        }
    }

    pub fn new(device: &wgpu::Device, instances: &[T]) -> InstanceBuffer<T>
    where
        T: bytemuck::Pod,
    {
        Self {
            buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Instance Buffer"),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                contents: bytemuck::cast_slice(instances),
            }),
            instances: instances.into(),
        }
    }

    pub fn size(&self) -> u32 {
        self.instances.len() as u32
    }

    pub fn slice(
        &self,
        bounds: impl std::ops::RangeBounds<wgpu::BufferAddress>,
    ) -> wgpu::BufferSlice {
        self.buffer.slice(bounds)
    }
}

impl<T> Deref for InstanceBuffer<T> {
    type Target = wgpu::Buffer;
    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],
}

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x2];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

pub struct VertexBuffer {
    buffer: wgpu::Buffer,
    vertices: Box<[Vertex]>,
}

impl VertexBuffer {
    pub fn new(device: &wgpu::Device, vertices: &[Vertex]) -> Self {
        Self {
            buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Buffer"),
                usage: wgpu::BufferUsages::VERTEX,
                contents: bytemuck::cast_slice(vertices),
            }),
            vertices: vertices.into(),
        }
    }
}

impl Deref for VertexBuffer {
    type Target = wgpu::Buffer;
    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

pub struct ProjectionUniform {
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
    pub buffer: wgpu::Buffer,
}

impl ProjectionUniform {
    pub fn new(device: &wgpu::Device, left: f32, right: f32, top: f32, bottom: f32) -> Self {
        let projection = Mat4::projection(left, right, top, bottom);

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Buffer"),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            contents: bytemuck::cast_slice(&projection),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("Projection Bind Group Layout"),
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
            label: Some("Projection Bind Group"),
        });

        Self {
            bind_group,
            bind_group_layout,
            buffer,
        }
    }
}
