mod audio;
pub mod buffers;
pub mod button;
mod config;
mod dbus;
mod image_data;
pub mod math;
mod notification_manager;
mod seat;
pub mod shape_renderer;
mod surface;
pub mod text;
pub mod texture_renderer;
mod wgpu_state;

use audio::Audio;
use calloop::EventLoop;
use calloop_wayland_source::WaylandSource;
use config::Config;
use dbus::xdg::NotificationData;
use image_data::ImageData;
use notification_manager::{NotificationManager, Reason};
use rusqlite::params;
use seat::Seat;
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Arc};
use surface::{FocusReason, Surface};
use tokio::sync::broadcast;
use wayland_client::{
    delegate_noop,
    globals::{registry_queue_init, GlobalList, GlobalListContents},
    protocol::{wl_compositor, wl_output, wl_registry},
    Connection, Dispatch, Proxy, QueueHandle,
};
use wayland_protocols::xdg::activation::v1::client::{xdg_activation_token_v1, xdg_activation_v1};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1;
use wgpu_state::WgpuState;

#[derive(Debug)]
pub struct Output {
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

#[derive(Default, PartialEq)]
enum History {
    #[default]
    Hidden,
    Shown,
}

pub struct Moxnotify {
    layer_shell: zwlr_layer_shell_v1::ZwlrLayerShellV1,
    seat: Seat,
    surface: Option<Surface>,
    outputs: Vec<Output>,
    wgpu_state: wgpu_state::WgpuState,
    notifications: NotificationManager,
    config: Arc<Config>,
    qh: QueueHandle<Self>,
    globals: GlobalList,
    loop_handle: calloop::LoopHandle<'static, Self>,
    emit_sender: broadcast::Sender<EmitEvent>,
    compositor: wl_compositor::WlCompositor,
    audio: Option<Audio>,
    db: rusqlite::Connection,
    inhibited: bool,
    history: History,
}

impl Moxnotify {
    fn new(
        conn: &Connection,
        qh: QueueHandle<Moxnotify>,
        globals: GlobalList,
        loop_handle: calloop::LoopHandle<'static, Self>,
        emit_sender: broadcast::Sender<EmitEvent>,
    ) -> anyhow::Result<Self> {
        let layer_shell = globals.bind(&qh, 1..=5, ())?;
        let compositor = globals.bind::<wl_compositor::WlCompositor, _, _>(&qh, 1..=6, ())?;
        let seat = Seat::new(&qh, &globals)?;

        let config = Arc::new(Config::load(None)?);

        let wgpu_state = WgpuState::new(conn)?;

        let db = rusqlite::Connection::open(&config.history.path)?;
        db.execute(
            "CREATE TABLE IF NOT EXISTS notifications (
            rowid UNSIGNED INTEGER PRIMARY KEY AUTOINCREMENT,
            id INTEGER,
            app_name TEXT,
            app_icon TEXT,
            summary TEXT,
            body TEXT,
            timeout INTEGER,
            actions TEXT,
            hints JSON
        );",
            (),
        )?;
        Ok(Self {
            history: History::Hidden,
            inhibited: false,
            db,
            audio: Audio::new().ok(),
            globals,
            qh,
            notifications: NotificationManager::new(Arc::clone(&config), loop_handle.clone()),
            config,
            wgpu_state,
            layer_shell,
            seat,
            surface: None,
            outputs: Vec::new(),
            loop_handle,
            emit_sender,
            compositor,
        })
    }

    fn handle_app_event(&mut self, event: Event) -> anyhow::Result<()> {
        match event {
            Event::Dismiss { all, id } => {
                if all {
                    let ids: Vec<_> = self
                        .notifications
                        .notifications()
                        .iter()
                        .map(|notification| notification.id())
                        .collect();

                    ids.iter()
                        .for_each(|id| self.dismiss(*id, Some(Reason::DismissedByUser)));
                    return Ok(());
                }

                if id == 0 {
                    if let Some(notification) = self.notifications.notifications().first() {
                        self.dismiss(notification.id(), Some(Reason::DismissedByUser));
                    }
                    return Ok(());
                }

                self.dismiss(id, Some(Reason::DismissedByUser));
            }
            Event::Notify(data) => {
                let path = match (
                    data.hints.sound_file.as_ref().map(Arc::clone),
                    data.hints.sound_name.as_ref().map(Arc::clone),
                ) {
                    (Some(sound_file), None) => Some(sound_file),
                    (None, Some(sound_name)) => freedesktop_sound::lookup(&sound_name)
                        .with_cache()
                        .find()
                        .map(|s| s.into()),
                    (None, None) => match data.hints.urgency {
                        Urgency::Low => self
                            .config
                            .default_sound_file
                            .urgency_low
                            .as_ref()
                            .map(Arc::clone),
                        Urgency::Normal => self
                            .config
                            .default_sound_file
                            .urgency_normal
                            .as_ref()
                            .map(Arc::clone),
                        Urgency::Critical => self
                            .config
                            .default_sound_file
                            .urgency_critical
                            .as_ref()
                            .map(Arc::clone),
                    },
                    (Some(sound_file), Some(_)) => Some(sound_file),
                };

                let suppress_sound = data.hints.suppress_sound;

                self.db.execute(
                    "INSERT INTO notifications (id, app_name, app_icon, summary, body, timeout, actions, hints)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        data.id,
                        data.app_name,
                        data.app_icon,
                        data.summary,
                        data.body,
                        data.timeout,
                        serde_json::to_string(&data.actions)?,
                        serde_json::to_string(&data.hints)?
                    ],
                )?;

                if self.inhibited && self.history == History::Hidden {
                    return Ok(());
                }

                let id = match self.history {
                    History::Shown => self.db.last_insert_rowid() as u32,
                    History::Hidden => data.id,
                };

                self.notifications.add(NotificationData { id, ..*data })?;

                self.update_surface_size();

                if suppress_sound {
                    return Ok(());
                }

                if let (Some(audio), Some(path)) = (self.audio.as_mut(), path) {
                    audio.play(path)?;
                }
            }
            Event::CloseNotification(id) => self.dismiss(id, Some(Reason::CloseNotificationCall)),
            Event::FocusSurface => {
                if let Some(surface) = self.surface.as_mut() {
                    if surface.focus_reason.is_none() {
                        surface.focus(FocusReason::Ctl);
                        self.notifications.next();
                    }
                }
            }
            Event::List => {
                let list = self
                    .notifications
                    .notifications()
                    .iter()
                    .map(|notification| serde_json::to_string(&notification.data).unwrap())
                    .collect::<Vec<_>>();
                _ = self.emit_sender.send(EmitEvent::List(list));
            }
            Event::Mute => {
                if let Some(audio) = self.audio.as_mut() {
                    if !audio.muted() {
                        _ = self.emit_sender.send(EmitEvent::MuteStateChanged(true));
                        audio.mute();
                    }
                }
            }
            Event::Unmute => {
                if let Some(audio) = self.audio.as_mut() {
                    if audio.muted() {
                        _ = self.emit_sender.send(EmitEvent::MuteStateChanged(false));
                        audio.unmute();
                    }
                }
            }
            Event::ShowHistory => {
                if self.history == History::Hidden {
                    _ = self.emit_sender.send(EmitEvent::HistoryStateChanged(true));
                    self.history = History::Shown;

                    let ids: Vec<_> = self
                        .notifications
                        .notifications()
                        .iter()
                        .map(|notification| notification.id())
                        .collect();

                    ids.iter()
                        .for_each(|id| self.dismiss(*id, Some(Reason::DismissedByUser)));

                    let mut stmt = self.db.prepare("SELECT rowid, app_name, app_icon, summary, body, timeout, actions, hints FROM notifications")?;
                    let rows = stmt.query_map([], |row| {
                        Ok(NotificationData {
                            id: row.get(0)?,
                            app_name: row.get::<_, String>(1)?.into_boxed_str(),
                            app_icon: row.get::<_, Option<String>>(2)?.map(|s| s.into_boxed_str()),
                            summary: row.get::<_, String>(3)?.into_boxed_str(),
                            body: row.get::<_, String>(4)?.into_boxed_str(),
                            timeout: row.get(5)?,
                            actions: {
                                let json: String = row.get(6)?;
                                serde_json::from_str(&json).unwrap()
                            },
                            hints: {
                                let json: String = row.get(7)?;
                                serde_json::from_str(&json).unwrap()
                            },
                        })
                    })?;

                    let notifications: Vec<_> = rows.collect::<Result<_, _>>()?;

                    notifications.into_iter().for_each(|notification| {
                        self.notifications.add(NotificationData {
                            timeout: 0,
                            ..notification
                        });
                    });
                    drop(stmt);

                    self.update_surface_size();
                }
            }
            Event::HideHistory => {
                if self.history == History::Shown {
                    _ = self.emit_sender.send(EmitEvent::HistoryStateChanged(false));
                    self.history = History::Hidden;

                    let ids: Vec<_> = self
                        .notifications
                        .notifications()
                        .iter()
                        .map(|notification| notification.id())
                        .collect();
                    ids.iter().for_each(|id| self.dismiss(*id, None));
                }
            }
            Event::Inhibit => {
                if !self.inhibited {
                    _ = self.emit_sender.send(EmitEvent::Inhibited(true));
                    self.inhibited = true;
                }
            }
            Event::Uninhibit => {
                if self.inhibited {
                    _ = self.emit_sender.send(EmitEvent::Inhibited(false));
                    self.inhibited = false;
                }
            }
        };

        if let Some(surface) = self.surface.as_mut() {
            surface.render(
                self.seat.keyboard.key_combination.mode,
                &self.wgpu_state.device,
                &self.wgpu_state.queue,
                &self.notifications,
            )?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Image {
    Name(Box<str>),
    File(Box<Path>),
    Data(ImageData),
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Default, Debug, Clone, Copy)]
pub enum Urgency {
    Low,
    #[default]
    Normal,
    Critical,
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum Hint {
    Value(i32),
    ActionIcons(bool),
    Category(Box<str>),
    DesktopEntry(Box<str>),
    Image(Image),
    IconData(Vec<u8>),
    Resident(bool),
    SoundFile(Box<Path>),
    SoundName(Box<str>),
    SuppressSound(bool),
    Transient(bool),
    Urgency(Urgency),
    X(i32),
    Y(i32),
}

#[derive(Clone)]
pub enum EmitEvent {
    ActionInvoked {
        id: u32,
        action_key: Arc<str>,
        token: Arc<str>,
    },
    NotificationClosed {
        id: u32,
        reason: u32,
    },
    Open {
        uri: Arc<str>,
        token: Option<Arc<str>>,
    },
    List(Vec<String>),
    MuteStateChanged(bool),
    HistoryStateChanged(bool),
    Inhibited(bool),
}

pub enum Event {
    Dismiss { all: bool, id: u32 },
    Notify(Box<NotificationData>),
    CloseNotification(u32),
    List,
    FocusSurface,
    Mute,
    Unmute,
    ShowHistory,
    HideHistory,
    Inhibit,
    Uninhibit,
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

impl Dispatch<xdg_activation_token_v1::XdgActivationTokenV1, ()> for Moxnotify {
    fn event(
        state: &mut Self,
        _: &xdg_activation_token_v1::XdgActivationTokenV1,
        event: <xdg_activation_token_v1::XdgActivationTokenV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_activation_token_v1::Event::Done { token } = event {
            if let Some(surface) = state.surface.as_mut() {
                surface.token = Some(token.into());
            }
        }
    }
}

delegate_noop!(Moxnotify: xdg_activation_v1::XdgActivationV1);
delegate_noop!(Moxnotify: wl_compositor::WlCompositor);
delegate_noop!(Moxnotify: zwlr_layer_shell_v1::ZwlrLayerShellV1);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::new()
        .filter(Some("daemon"), log::LevelFilter::Info)
        .init();

    let conn = Connection::connect_to_env().expect("Failed to connect to Wayland");
    let (globals, event_queue) = registry_queue_init(&conn)?;
    let qh = event_queue.handle();

    let (emit_sender, emit_receiver) = broadcast::channel(std::mem::size_of::<EmitEvent>());
    let mut event_loop = EventLoop::try_new()?;
    let mut moxnotify =
        Moxnotify::new(&conn, qh, globals, event_loop.handle(), emit_sender.clone())?;

    WaylandSource::new(conn, event_queue)
        .insert(event_loop.handle())
        .map_err(|e| anyhow::anyhow!("Failed to insert Wayland source: {}", e))?;

    moxnotify.globals.contents().with_list(|list| {
        list.iter().for_each(|global| {
            if global.interface == wl_output::WlOutput::interface().name {
                let wl_output = moxnotify.globals.registry().bind(
                    global.name,
                    global.version,
                    &moxnotify.qh,
                    (),
                );
                let output = Output::new(wl_output, global.name);
                moxnotify.outputs.push(output);
            }
        });
    });

    let (event_sender, event_receiver) = calloop::channel::channel();
    let (executor, scheduler) = calloop::futures::executor()?;

    {
        let event_sender = event_sender.clone();
        scheduler.schedule(async move {
            if let Err(e) = dbus::xdg::serve(event_sender, emit_receiver).await {
                log::error!("{e}");
            }
        })?;
    }

    let emit_receiver = emit_sender.subscribe();
    scheduler.schedule(async move {
        if let Err(e) = dbus::moxnotify::serve(event_sender, emit_receiver).await {
            log::error!("{e}");
        }
    })?;

    let emit_receiver = emit_sender.subscribe();
    scheduler.schedule(async move {
        if let Err(e) = dbus::portal::serve(emit_receiver).await {
            log::error!("{e}");
        }
    })?;

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
