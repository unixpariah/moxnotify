pub mod wgpu_surface;

use crate::{
    config::{self, Anchor, Config},
    wgpu_state, Moxnotify, Output,
};
use std::sync::Arc;
use wayland_client::{delegate_noop, protocol::wl_surface, Connection, Dispatch, QueueHandle};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1,
    zwlr_layer_surface_v1::{self, KeyboardInteractivity},
};

pub struct Surface {
    pub wgpu_surface: wgpu_surface::WgpuSurface,
    pub wl_surface: wl_surface::WlSurface,
    pub layer_surface: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    pub scale: f32,
    pub configured: bool,
}

impl Surface {
    pub fn new(
        wgpu_state: &wgpu_state::WgpuState,
        wl_surface: wl_surface::WlSurface,
        layer_shell: &zwlr_layer_shell_v1::ZwlrLayerShellV1,
        qh: &QueueHandle<Moxnotify>,
        outputs: &[Output],
        config: Arc<Config>,
    ) -> Self {
        let (layer_surface, scale) = Surface::create_layer_surface(
            Arc::clone(&config),
            outputs,
            &wl_surface,
            qh,
            layer_shell,
        );
        Self {
            configured: false,
            scale,
            wgpu_surface: wgpu_surface::WgpuSurface::new(wgpu_state, &wl_surface, config).unwrap(),
            wl_surface,
            layer_surface,
        }
    }

    fn create_layer_surface(
        config: Arc<Config>,
        outputs: &[Output],
        wl_surface: &wl_surface::WlSurface,
        qh: &QueueHandle<Moxnotify>,
        layer_shell: &zwlr_layer_shell_v1::ZwlrLayerShellV1,
    ) -> (zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, f32) {
        let output = outputs
            .iter()
            .find(|output| output.name.as_ref() == Some(&config.output));

        let layer_surface = layer_shell.get_layer_surface(
            &wl_surface,
            output.map(|o| &o.wl_output),
            match config.layer {
                config::Layer::Top => zwlr_layer_shell_v1::Layer::Top,
                config::Layer::Background => zwlr_layer_shell_v1::Layer::Background,
                config::Layer::Bottom => zwlr_layer_shell_v1::Layer::Bottom,
                config::Layer::Overlay => zwlr_layer_shell_v1::Layer::Overlay,
            },
            "moxnotify".into(),
            qh,
            (),
        );

        let scale = output.map(|o| o.scale).unwrap_or(1.0);

        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer_surface
            .set_anchor(zwlr_layer_surface_v1::Anchor::Right | zwlr_layer_surface_v1::Anchor::Top);
        layer_surface.set_anchor(match config.anchor {
            Anchor::TopRight => {
                zwlr_layer_surface_v1::Anchor::Top | zwlr_layer_surface_v1::Anchor::Right
            }
            Anchor::TopCenter => zwlr_layer_surface_v1::Anchor::Top,
            Anchor::TopLeft => {
                zwlr_layer_surface_v1::Anchor::Top | zwlr_layer_surface_v1::Anchor::Left
            }
            Anchor::BottomRight => {
                zwlr_layer_surface_v1::Anchor::Bottom | zwlr_layer_surface_v1::Anchor::Right
            }
            Anchor::BottomCenter => zwlr_layer_surface_v1::Anchor::Bottom,
            Anchor::BottomLeft => {
                zwlr_layer_surface_v1::Anchor::Bottom | zwlr_layer_surface_v1::Anchor::Left
            }
            Anchor::CenterRight => zwlr_layer_surface_v1::Anchor::Right,
            Anchor::Center => {
                zwlr_layer_surface_v1::Anchor::Top
                    | zwlr_layer_surface_v1::Anchor::Bottom
                    | zwlr_layer_surface_v1::Anchor::Left
                    | zwlr_layer_surface_v1::Anchor::Right
            }
            Anchor::CenterLeft => zwlr_layer_surface_v1::Anchor::Left,
        });
        layer_surface.set_exclusive_zone(-1);
        (layer_surface, scale)
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        self.layer_surface.destroy();
        self.wl_surface.destroy();
    }
}

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for Moxnotify {
    fn event(
        state: &mut Self,
        _: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: <zwlr_layer_surface_v1::ZwlrLayerSurfaceV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(surface) = state.surface.as_mut() else {
            return;
        };
        if let zwlr_layer_surface_v1::Event::Configure {
            serial,
            width,
            height,
        } = event
        {
            surface.layer_surface.ack_configure(serial);
            surface.configured = true;
            state.resize(width, height);
            state.render();
        }
    }
}

delegate_noop!(Moxnotify: ignore wl_surface::WlSurface);
