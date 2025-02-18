mod config;
mod dbus;
mod image_data;
mod notification_manager;
mod seat;
mod surface;
mod text;
mod wgpu_state;

use calloop::EventLoop;
use calloop_wayland_source::WaylandSource;
use config::Config;
use image_data::ImageData;
use notification_manager::NotificationManager;
use seat::Seat;
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{mpsc, Arc},
};
use surface::Surface;
use text::TextContext;
use wayland_client::{
    delegate_noop,
    globals::{registry_queue_init, GlobalList, GlobalListContents},
    protocol::{wl_compositor, wl_output, wl_registry},
    Connection, Dispatch, Proxy, QueueHandle,
};
use wayland_protocols::xdg::activation::v1::client::{xdg_activation_token_v1, xdg_activation_v1};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1, zwlr_layer_surface_v1::KeyboardInteractivity,
};
use wgpu_state::WgpuState;

#[derive(Debug)]
struct Output {
    id: u32,
    name: Option<Box<str>>,
    scale: f32,
    wl_output: wl_output::WlOutput,
}

impl Output {
    fn new(wl_output: wl_output::WlOutput, id: u32) -> Self {
        Self {
            id,
            name: None,
            scale: 1.0,
            wl_output,
        }
    }
}

struct Moxnotify {
    layer_shell: zwlr_layer_shell_v1::ZwlrLayerShellV1,
    text_ctx: TextContext,
    seat: Seat,
    surface: Surface,
    outputs: Vec<Output>,
    wgpu_state: wgpu_state::WgpuState,
    notifications: NotificationManager,
    config: Arc<Config>,
    qh: QueueHandle<Self>,
    globals: GlobalList,
    loop_handle: calloop::LoopHandle<'static, Self>,
    emit_sender: mpsc::Sender<EmitEvent>,
}

impl Moxnotify {
    fn new(
        conn: &Connection,
        qh: QueueHandle<Moxnotify>,
        globals: GlobalList,
        loop_handle: calloop::LoopHandle<'static, Self>,
        emit_sender: mpsc::Sender<EmitEvent>,
    ) -> anyhow::Result<Self> {
        let layer_shell = globals.bind(&qh, 1..=5, ())?;
        let compositor = globals.bind::<wl_compositor::WlCompositor, _, _>(&qh, 1..=6, ())?;
        let seat = Seat::new(conn, &qh, &globals, &compositor)?;
        let wl_surface = compositor.create_surface(&qh, ());

        let config = Arc::new(Config::load(None)?);

        let (wgpu_state, wgpu_surface, surface_config) =
            WgpuState::new(conn, &wl_surface, &config)?;

        let text_ctx = text::TextContext::new(
            &wgpu_state.device,
            &wgpu_state.queue,
            surface_config.format,
            surface_config.width,
            surface_config.height,
        );

        let surface = Surface::new(wl_surface, surface_config, wgpu_surface);

        Ok(Self {
            text_ctx,
            globals,
            qh,
            notifications: NotificationManager::new(config.clone(), loop_handle.clone()),
            config,
            wgpu_state,
            layer_shell,
            seat,
            surface,
            outputs: Vec::new(),
            loop_handle,
            emit_sender,
        })
    }

    fn render(&mut self) {
        self.notifications
            .iter_mut()
            .fold(0.0, |acc, notification| {
                notification.change_spot(acc);
                acc + notification.extents().height
            });

        if let Some(mut notification_count) = self.notifications.notification_count.take() {
            notification_count.change_spot(self.notifications.height());
            self.notifications.notification_count = Some(notification_count);
        }

        self.update_surface_size();

        let surface_texture = self
            .surface
            .wgpu_surface
            .get_current_texture()
            .expect("failed to acquire next swapchain texture");
        let texture_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .wgpu_state
            .device
            .create_command_encoder(&Default::default());
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

        let (instances, text_data, textures) = self.notifications.data(self.surface.scale);

        self.wgpu_state.shape_renderer.prepare(
            &self.wgpu_state.device,
            &self.wgpu_state.queue,
            &instances,
        );

        self.wgpu_state.shape_renderer.render(&mut render_pass);

        _ = self.text_ctx.render(
            &self.wgpu_state.device,
            &self.wgpu_state.queue,
            &mut render_pass,
            text_data,
        );

        self.wgpu_state.texture_renderer.prepare(
            &self.wgpu_state.device,
            &self.wgpu_state.queue,
            textures,
        );
        self.wgpu_state.texture_renderer.render(&mut render_pass);

        drop(render_pass); // Drop renderpass and release mutable borrow on encoder

        self.wgpu_state.queue.submit(Some(encoder.finish()));
        surface_texture.present();
    }

    fn handle_app_event(&mut self, event: Event) -> anyhow::Result<()> {
        match event {
            Event::Notify(data) => {
                self.notifications.add(data)?;
                self.update_surface_size();
                if self.notifications.visible.end <= self.notifications.len() {
                    self.render();
                }
            }
            Event::CloseNotification(id) => self.dismiss_notification(id),
            Event::FocusSurface => {
                if let Some(layer_surface) = self.surface.layer_surface.as_ref() {
                    layer_surface.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
                    self.surface.wl_surface.commit();
                    self.notifications.next();
                    self.render();
                }
            }
        };
        Ok(())
    }

    fn create_activation_token(&self, serial: u32, id: u32) {
        let token = self.seat.xdg_activation.get_activation_token(&self.qh, id);
        token.set_serial(serial, &self.seat.wl_seat);
        token.set_surface(&self.surface.wl_surface);
        token.commit();
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == self.surface.config.height
            || height == self.surface.config.width
            || width == 0
            || height == 0
        {
            return;
        }
        self.surface.config.width = width;
        self.surface.config.height = height;
        self.surface
            .wgpu_surface
            .configure(&self.wgpu_state.device, &self.surface.config);
        self.text_ctx.viewport.update(
            &self.wgpu_state.queue,
            glyphon::Resolution { width, height },
        );
        self.wgpu_state
            .shape_renderer
            .resize(&self.wgpu_state.queue, width as f32, height as f32);
        self.wgpu_state.texture_renderer.resize(
            &self.wgpu_state.queue,
            width as f32,
            height as f32,
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Image {
    Name(String),
    File(PathBuf),
    Data(ImageData),
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Urgency {
    #[default]
    Low,
    Normal,
    Critical,
}

#[derive(PartialEq, Eq, Serialize, Deserialize)]
pub enum Hint {
    ActionIcons(bool),
    Category(String),
    DesktopEntry(String),
    Image(Image),
    IconData(Vec<u8>),
    Resident(bool),
    SoundFile(PathBuf),
    SoundName(String),
    SuppressSound(bool),
    Transient(bool),
    Urgency(Urgency),
    X(i32),
    Y(i32),
}

pub struct NotificationData {
    id: u32,
    app_name: Box<str>,
    summary: Box<str>,
    body: Box<str>,
    timeout: i32,
    actions: Box<[(Arc<str>, Arc<str>)]>,
    hints: Vec<Hint>,
}

pub enum EmitEvent {
    ActionInvoked {
        id: u32,
        action_key: Arc<str>,
        token: Box<str>,
    },
    NotificationClosed {
        id: u32,
        reason: u32,
    },
}

pub enum Event {
    Notify(NotificationData),
    CloseNotification(u32),
    FocusSurface,
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Moxnotify {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: <wl_registry::WlRegistry as wayland_client::Proxy>::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => {
                if interface.as_str() == "wl_output" {
                    let output = registry.bind::<wl_output::WlOutput, _, _>(name, version, qh, ());

                    let output = Output::new(output, name);
                    state.outputs.push(output);
                }
            }
            wl_registry::Event::GlobalRemove { name } => {
                state.outputs.retain(|output| output.id != name);
            }
            _ => unreachable!(),
        }
    }
}

impl Dispatch<wl_output::WlOutput, ()> for Moxnotify {
    fn event(
        state: &mut Self,
        wl_output: &wl_output::WlOutput,
        event: <wl_output::WlOutput as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(output) = state
            .outputs
            .iter_mut()
            .find(|output| output.wl_output == *wl_output)
        else {
            return;
        };

        match event {
            wl_output::Event::Scale { factor } => output.scale = factor as f32,
            wl_output::Event::Name { name } => output.name = Some(name.into()),
            _ => {}
        }
    }
}

impl Dispatch<xdg_activation_token_v1::XdgActivationTokenV1, u32> for Moxnotify {
    fn event(
        state: &mut Self,
        _: &xdg_activation_token_v1::XdgActivationTokenV1,
        event: <xdg_activation_token_v1::XdgActivationTokenV1 as Proxy>::Event,
        id: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(notification) = state
            .notifications
            .iter()
            .find(|notification| notification.id() == *id)
        else {
            return;
        };

        if let xdg_activation_token_v1::Event::Done { token } = event {
            if let Some(action) = notification.actions.first() {
                if let Err(e) = state.emit_sender.send(EmitEvent::ActionInvoked {
                    id: *id,
                    action_key: Arc::clone(&action.0),
                    token: token.into(),
                }) {
                    log::error!("Failed to send ActionInvoked event: {e}");
                }
            }
        }
    }
}

delegate_noop!(Moxnotify: xdg_activation_v1::XdgActivationV1);
delegate_noop!(Moxnotify: wl_compositor::WlCompositor);
delegate_noop!(Moxnotify: zwlr_layer_shell_v1::ZwlrLayerShellV1);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let conn = Connection::connect_to_env().expect("Failed to connect to wayland");
    let (globals, event_queue) = registry_queue_init(&conn)?;
    let qh = event_queue.handle();

    let (emit_sender, emit_receiver) = mpsc::channel();
    let mut event_loop = EventLoop::try_new()?;
    let mut moxnotify = Moxnotify::new(&conn, qh, globals, event_loop.handle(), emit_sender)?;

    WaylandSource::new(conn, event_queue)
        .insert(event_loop.handle())
        .map_err(|e| anyhow::anyhow!("Failed to insert wayland source: {}", e))?;

    moxnotify.globals.contents().with_list(|list| {
        list.iter().for_each(|global| {
            if global.interface == wl_output::WlOutput::interface().name {
                let wl_output = moxnotify.globals.registry().bind(
                    global.name,
                    global.version,
                    &moxnotify.qh,
                    (),
                );
                let output = crate::Output::new(wl_output, global.name);
                moxnotify.outputs.push(output);
            }
        });
    });

    let (event_sender, event_receiver) = calloop::channel::channel();
    let (executor, scheduler) = calloop::futures::executor()?;

    {
        let event_sender = event_sender.clone();
        scheduler.schedule(async move {
            _ = dbus::xdg::serve(event_sender, emit_receiver).await;
        })?;
    }

    {
        let event_sender = event_sender.clone();
        scheduler.schedule(async move {
            _ = dbus::moxnotify::serve(event_sender).await;
        })?;
    }

    event_loop
        .handle()
        .insert_source(executor, |_: (), _, _| ())
        .map_err(|e| anyhow::anyhow!("Failed to insert source: {}", e))?;

    event_loop
        .handle()
        .insert_source(event_receiver, |event, _, moxnotify| {
            if let calloop::channel::Event::Msg(event) = event {
                if let Err(e) = moxnotify.handle_app_event(event) {
                    log::error!("Failed to handle event: {e}");
                }
            }
        })
        .map_err(|e| anyhow::anyhow!("Failed to insert source: {}", e))?;

    event_loop.run(None, &mut moxnotify, |_| {})?;

    Ok(())
}
