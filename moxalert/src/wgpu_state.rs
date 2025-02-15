pub mod buffers;
pub mod shape_renderer;
pub mod texture_renderer;

use anyhow::Context;
use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
};
use shape_renderer::ShapeRenderer;
use std::ptr::NonNull;
use texture_renderer::TextureRenderer;
use wayland_client::{protocol::wl_surface, Connection, Proxy};

use crate::config::Config;

pub struct WgpuState {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub queue: wgpu::Queue,
    pub device: wgpu::Device,
    pub texture_renderer: texture_renderer::TextureRenderer,
    pub shape_renderer: shape_renderer::ShapeRenderer,
}

impl WgpuState {
    pub fn new(
        conn: &Connection,
        surface: &wl_surface::WlSurface,
        config: &Config,
    ) -> anyhow::Result<(Self, wgpu::Surface<'static>, wgpu::SurfaceConfiguration)> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });

        let raw_display_handle = RawDisplayHandle::Wayland(WaylandDisplayHandle::new(
            NonNull::new(conn.backend().display_ptr() as *mut _).unwrap(),
        ));

        let raw_window_handle = RawWindowHandle::Wayland(WaylandWindowHandle::new(
            NonNull::new(surface.id().as_ptr() as *mut _).context("Surface id is a null ptr")?,
        ));

        let wgpu_surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle,
                raw_window_handle,
            })?
        };

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&wgpu_surface),
            ..Default::default()
        }))
        .expect("Failed to find suitable adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default(), None))
            .expect("Failed to request device");

        let surface_caps = wgpu_surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .unwrap_or(&surface_caps.formats[0]);

        let alpha_mode = surface_caps
            .alpha_modes
            .iter()
            .find(|a| **a == wgpu::CompositeAlphaMode::PreMultiplied)
            .unwrap_or(&surface_caps.alpha_modes[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: *surface_format,
            width: 200,
            height: 200,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: *alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        let texture_renderer = TextureRenderer::new(
            &device,
            *surface_format,
            config.max_icon_size,
            config.max_visible,
        );

        let shape_renderer = ShapeRenderer::new(&device, *surface_format);

        Ok((
            Self {
                texture_renderer,
                device,
                queue,
                instance,
                adapter,
                shape_renderer,
            },
            wgpu_surface,
            surface_config,
        ))
    }
}
