use raw_window_handle::{RawDisplayHandle, WaylandDisplayHandle};
use std::ptr::NonNull;
use wayland_client::Connection;

pub struct WgpuState {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub queue: wgpu::Queue,
    pub device: wgpu::Device,
    pub raw_display_handle: RawDisplayHandle,
}

impl WgpuState {
    pub fn new(conn: &Connection) -> anyhow::Result<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });

        let raw_display_handle = RawDisplayHandle::Wayland(WaylandDisplayHandle::new(
            NonNull::new(conn.backend().display_ptr() as *mut _).unwrap(),
        ));

        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .expect("Failed to find suitable adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default(), None))
            .expect("Failed to request device");

        Ok(Self {
            device,
            queue,
            instance,
            adapter,
            raw_display_handle,
        })
    }
}
