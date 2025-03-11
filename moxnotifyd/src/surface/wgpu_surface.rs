use crate::{
    config::Config, shape_renderer, text::TextContext, texture_renderer, wgpu_state::WgpuState,
};
use anyhow::Context;
use raw_window_handle::{RawWindowHandle, WaylandWindowHandle};

use std::{ptr::NonNull, sync::Arc};
use wayland_client::{protocol::wl_surface, Proxy};

pub struct WgpuSurface {
    pub texture_renderer: texture_renderer::TextureRenderer,
    pub shape_renderer: shape_renderer::ShapeRenderer,
    pub text_ctx: TextContext,
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
}

impl WgpuSurface {
    pub fn new(
        wgpu_state: &WgpuState,
        surface: &wl_surface::WlSurface,
        config: Arc<Config>,
    ) -> anyhow::Result<Self> {
        let raw_window_handle = RawWindowHandle::Wayland(WaylandWindowHandle::new(
            NonNull::new(surface.id().as_ptr() as *mut _).context("Surface id is a null ptr")?,
        ));

        let wgpu_surface = unsafe {
            wgpu_state
                .instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: wgpu_state.raw_display_handle,
                    raw_window_handle,
                })?
        };

        let surface_caps = wgpu_surface.get_capabilities(&wgpu_state.adapter);
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
            width: 1,
            height: 1,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: *alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        let texture_renderer = texture_renderer::TextureRenderer::new(
            &wgpu_state.device,
            *surface_format,
            config.max_icon_size,
        );

        let shape_renderer =
            shape_renderer::ShapeRenderer::new(&wgpu_state.device, *surface_format);

        let text_ctx =
            TextContext::new(&wgpu_state.device, &wgpu_state.queue, surface_config.format);

        Ok(Self {
            shape_renderer,
            texture_renderer,
            text_ctx,
            surface: wgpu_surface,
            config: surface_config,
        })
    }
}
