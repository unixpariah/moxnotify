pub mod wgpu_surface;

use crate::{
    config::{self, keymaps::Mode, Anchor, Config},
    notification_manager::NotificationManager,
    wgpu_state, Moxnotify, Output,
};
use std::sync::Arc;
use wayland_client::{delegate_noop, protocol::wl_surface, Connection, Dispatch, QueueHandle};
use wayland_protocols::xdg::foreign::zv2::client::zxdg_exporter_v2;
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1,
    zwlr_layer_surface_v1::{self, KeyboardInteractivity},
};

#[derive(PartialEq, Debug)]
pub enum FocusReason {
    Ctl,
    MouseEnter,
}

pub struct Surface {
    pub wgpu_surface: wgpu_surface::WgpuSurface,
    pub wl_surface: wl_surface::WlSurface,
    pub layer_surface: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    scale: f32,
    configured: bool,
    pub token: Option<Arc<str>>,
    pub focus_reason: Option<FocusReason>,
}

impl Surface {
    pub fn new(
        wgpu_state: &wgpu_state::WgpuState,
        wl_surface: wl_surface::WlSurface,
        layer_shell: &zwlr_layer_shell_v1::ZwlrLayerShellV1,
        qh: &QueueHandle<Moxnotify>,
        outputs: &[Output],
        config: Arc<Config>,
    ) -> anyhow::Result<Self> {
        let (layer_surface, scale) = Surface::create_layer_surface(
            Arc::clone(&config),
            outputs,
            &wl_surface,
            qh,
            layer_shell,
        );

        Ok(Self {
            focus_reason: None,
            token: None,
            configured: false,
            scale,
            wgpu_surface: wgpu_surface::WgpuSurface::new(wgpu_state, &wl_surface, config)?,
            wl_surface,
            layer_surface,
        })
    }

    pub fn render(
        &mut self,
        mode: Mode,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        notifications: &NotificationManager,
    ) -> anyhow::Result<()> {
        if !self.configured {
            return Ok(());
        }

        let surface_texture = self
            .wgpu_surface
            .surface
            .get_current_texture()
            .expect("failed to acquire next swapchain texture");
        let texture_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = device.create_command_encoder(&Default::default());
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &texture_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        let (instances, text_data, textures) = notifications.data(mode, self.scale);

        self.wgpu_surface
            .shape_renderer
            .prepare(device, queue, &instances);
        self.wgpu_surface
            .texture_renderer
            .prepare(device, queue, &textures);
        self.wgpu_surface
            .text_ctx
            .prepare(device, queue, text_data)?;

        self.wgpu_surface.shape_renderer.render(&mut render_pass);
        self.wgpu_surface.texture_renderer.render(&mut render_pass);
        self.wgpu_surface.text_ctx.render(&mut render_pass)?;

        drop(render_pass); // Drop renderpass and release mutable borrow on encoder

        queue.submit(Some(encoder.finish()));
        surface_texture.present();

        Ok(())
    }

    pub fn resize(&mut self, queue: &wgpu::Queue, device: &wgpu::Device, width: u32, height: u32) {
        if width == self.wgpu_surface.config.height
            || height == self.wgpu_surface.config.width
            || width == 0
            || height == 0
        {
            return;
        }
        self.wgpu_surface.config.width = width;
        self.wgpu_surface.config.height = height;
        self.wgpu_surface
            .surface
            .configure(device, &self.wgpu_surface.config);
        self.wgpu_surface
            .text_ctx
            .viewport
            .update(queue, glyphon::Resolution { width, height });
        self.wgpu_surface
            .shape_renderer
            .resize(queue, width as f32, height as f32);
        self.wgpu_surface
            .texture_renderer
            .resize(queue, width as f32, height as f32);
    }

    pub fn focus(&mut self, focus_reason: FocusReason) {
        if self.focus_reason.is_some() {
            return;
        }

        match focus_reason {
            FocusReason::Ctl => self
                .layer_surface
                .set_keyboard_interactivity(KeyboardInteractivity::Exclusive),
            FocusReason::MouseEnter => self
                .layer_surface
                .set_keyboard_interactivity(KeyboardInteractivity::OnDemand),
        }

        self.focus_reason = Some(focus_reason);
    }

    pub fn unfocus(&mut self) {
        if let Some(FocusReason::Ctl) = self.focus_reason {
            self.layer_surface
                .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
        }
        self.focus_reason = None;
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
            .find(|output| output.name.as_ref() == config.output.as_ref());

        let layer_surface = layer_shell.get_layer_surface(
            wl_surface,
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
        layer_surface.set_margin(
            config.margin.top.resolve(0.) as i32,
            config.margin.right.resolve(0.) as i32,
            config.margin.bottom.resolve(0.) as i32,
            config.margin.left.resolve(0.) as i32,
        );
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
        qh: &QueueHandle<Self>,
    ) {
        if let zwlr_layer_surface_v1::Event::Configure {
            serial,
            width,
            height,
        } = event
        {
            if let Some(surface) = state.surface.as_ref() {
                let token = state.seat.xdg_activation.get_activation_token(qh, ());
                token.set_serial(serial, &state.seat.wl_seat);
                token.set_surface(&surface.wl_surface);
                token.commit();
            }

            if let Some(surface) = state.surface.as_mut() {
                surface.resize(
                    &state.wgpu_state.queue,
                    &state.wgpu_state.device,
                    width,
                    height,
                );
                surface.layer_surface.ack_configure(serial);
                surface.configured = true;
                _ = surface.render(
                    state.seat.keyboard.key_combination.mode,
                    &state.wgpu_state.device,
                    &state.wgpu_state.queue,
                    &state.notifications,
                );
            }
        }
    }
}

delegate_noop!(Moxnotify: zxdg_exporter_v2::ZxdgExporterV2);
delegate_noop!(Moxnotify: ignore wl_surface::WlSurface);

impl Moxnotify {
    pub fn update_surface_size(&mut self) {
        let total_height = self.notifications.height();
        let total_width = self.notifications.width();

        if self.surface.is_none() {
            let wl_surface = self.compositor.create_surface(&self.qh, ());
            self.surface = Surface::new(
                &self.wgpu_state,
                wl_surface,
                &self.layer_shell,
                &self.qh,
                &self.outputs,
                Arc::clone(&self.config),
            )
            .ok();
        }

        if total_width == 0. || total_height == 0. {
            if let Some(surface) = self.surface.take() {
                drop(surface);
            }
            self.seat.keyboard.key_combination.keys.clear();
            return;
        }

        if let Some(surface) = self.surface.as_ref() {
            surface
                .layer_surface
                .set_size(total_width as u32, total_height as u32);
            surface.wl_surface.commit();
        }
    }
}
