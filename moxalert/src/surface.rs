use crate::Moxalert;
use wayland_client::{delegate_noop, protocol::wl_surface, Connection, Dispatch, QueueHandle};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1;

pub struct Surface {
    pub wl_surface: wl_surface::WlSurface,
    pub layer_surface: Option<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1>,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub wgpu_surface: wgpu::Surface<'static>,
    pub scale: f32,
}

impl Surface {
    pub fn new(
        wl_surface: wl_surface::WlSurface,
        config: wgpu::SurfaceConfiguration,
        wgpu_surface: wgpu::Surface<'static>,
    ) -> Self {
        Self {
            scale: 1.0,
            surface_config: config,
            wgpu_surface,
            wl_surface,
            layer_surface: None,
        }
    }
}

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for Moxalert {
    fn event(
        state: &mut Self,
        _: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: <zwlr_layer_surface_v1::ZwlrLayerSurfaceV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwlr_layer_surface_v1::Event::Configure {
            serial,
            width,
            height,
        } = event
        {
            if let Some(layer_surface) = state.surface.layer_surface.as_ref() {
                layer_surface.ack_configure(serial);
                state.resize(width, height);
                state.render();
            }
        }
    }
}

delegate_noop!(Moxalert: ignore wl_surface::WlSurface);
