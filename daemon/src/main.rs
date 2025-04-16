mod animation;
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
use clap::Parser;
use config::Config;
use dbus::xdg::NotificationData;
use env_logger::Builder;
use image_data::ImageData;
use log::LevelFilter;
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
use zbus::zvariant::Type;

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

#[derive(Default, PartialEq, Clone, Copy, Type, Serialize)]
pub enum History {
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
    history: History,
}

impl Moxnotify {
    async fn new(
        conn: &Connection,
        qh: QueueHandle<Moxnotify>,
        globals: GlobalList,
        loop_handle: calloop::LoopHandle<'static, Self>,
        emit_sender: broadcast::Sender<EmitEvent>,
        config_path: Option<Box<Path>>,
    ) -> anyhow::Result<Self> {
        let layer_shell = globals.bind(&qh, 1..=5, ())?;
        let compositor = globals.bind::<wl_compositor::WlCompositor, _, _>(&qh, 1..=6, ())?;
        let seat = Seat::new(&qh, &globals)?;

        let config = Arc::new(Config::load(config_path)?);

        let wgpu_state = WgpuState::new(conn).await?;

        let db = rusqlite::Connection::open(&config.general.history.path)?;
        db.execute(
            "CREATE TABLE IF NOT EXISTS notifications (
            rowid INTEGER PRIMARY KEY AUTOINCREMENT,
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
                    self.dismiss_range(.., Some(Reason::DismissedByUser));
                    return Ok(());
                }

                if id == 0 {
                    if let Some(notification) = self.notifications.notifications().first() {
                        self.dismiss_by_id(notification.id(), Some(Reason::DismissedByUser));
                    }
                    return Ok(());
                }

                self.dismiss_by_id(id, Some(Reason::DismissedByUser));
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
                            .general
                            .default_sound_file
                            .urgency_low
                            .as_ref()
                            .map(Arc::clone),
                        Urgency::Normal => self
                            .config
                            .general
                            .default_sound_file
                            .urgency_normal
                            .as_ref()
                            .map(Arc::clone),
                        Urgency::Critical => self
                            .config
                            .general
                            .default_sound_file
                            .urgency_critical
                            .as_ref()
                            .map(Arc::clone),
                    },
                    (Some(sound_file), Some(_)) => Some(sound_file),
                };

                let suppress_sound = data.hints.suppress_sound;

                let count: i64 =
                    self.db
                        .query_row("SELECT COUNT(*) FROM notifications", [], |row| row.get(0))?;

                if count >= self.config.general.history.size {
                    self.db.execute(
                        "DELETE FROM notifications WHERE rowid = (
                        SELECT rowid FROM notifications ORDER BY rowid ASC LIMIT 1
                         )",
                        [],
                    )?;
                }

                self.db.execute(
                    "INSERT INTO notifications (id, app_name, app_icon, timeout, summary, body, actions, hints)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        data.id,
                        data.app_name,
                        data.app_icon,
                        data.timeout,
                        data.summary,
                        data.body,
                        serde_json::to_string(&data.actions)?,
                        serde_json::to_string(&data.hints)?
                    ],
                )?;

                let id = match self.history {
                    History::Shown => self.db.last_insert_rowid() as u32,
                    History::Hidden => data.id,
                };

                self.notifications.add(NotificationData { id, ..*data })?;
                self.update_surface_size();

                if self.notifications.inhibited() || suppress_sound {
                    return Ok(());
                }

                if let (Some(audio), Some(path)) = (self.audio.as_mut(), path) {
                    audio.play(path)?;
                }
            }
            Event::CloseNotification(id) => {
                self.dismiss_by_id(id, Some(Reason::CloseNotificationCall))
            }
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
                        audio.unmute();
                        _ = self
                            .emit_sender
                            .send(EmitEvent::MuteStateChanged(audio.muted()));
                    }
                }
            }
            Event::ShowHistory => {
                if self.history == History::Hidden {
                    self.history = History::Shown;
                    _ = self
                        .emit_sender
                        .send(EmitEvent::HistoryStateChanged(self.history));

                    self.dismiss_range(.., Some(Reason::Expired));

                    let mut stmt = self.db.prepare("SELECT rowid, app_name, app_icon, summary, body, actions, hints FROM notifications ORDER BY rowid DESC")?;
                    let rows = stmt.query_map([], |row| {
                        Ok(NotificationData {
                            id: row.get(0)?,
                            app_name: row.get::<_, Box<str>>(1)?,
                            app_icon: row.get::<_, Option<Box<str>>>(2)?,
                            summary: row.get::<_, Box<str>>(3)?,
                            body: row.get::<_, Box<str>>(4)?,
                            timeout: 0,
                            actions: {
                                let json: Box<str> = row.get(5)?;
                                serde_json::from_str(&json).unwrap()
                            },
                            hints: {
                                let json: Box<str> = row.get(6)?;
                                serde_json::from_str(&json).unwrap()
                            },
                        })
                    })?;

                    self.notifications
                        .add_many(rows.collect::<Result<Vec<_>, _>>()?)?;
                    drop(stmt);

                    self.update_surface_size();
                }
            }
            Event::HideHistory => {
                if self.history == History::Shown {
                    self.history = History::Hidden;
                    _ = self
                        .emit_sender
                        .send(EmitEvent::HistoryStateChanged(self.history));

                    self.dismiss_range(.., None);
                }
            }
            Event::Inhibit => {
                if !self.notifications.inhibited() {
                    self.notifications.inhibit();
                    _ = self.emit_sender.send(EmitEvent::InhibitStateChanged(
                        self.notifications.inhibited(),
                    ));
                }
            }
            Event::Uninhibit => {
                if self.notifications.inhibited() {
                    let count = self.notifications.waiting();

                    let mut stmt = self.db.prepare("SELECT id, app_name, app_icon, summary, body, timeout, actions, hints FROM notifications ORDER BY rowid DESC LIMIT ?1")?;
                    let rows = stmt.query_map([count], |row| {
                        Ok(NotificationData {
                            id: row.get(0)?,
                            app_name: row.get::<_, Box<str>>(1)?,
                            app_icon: row.get::<_, Option<Box<str>>>(2)?,
                            summary: row.get::<_, Box<str>>(3)?,
                            body: row.get::<_, Box<str>>(4)?,
                            timeout: row.get(5)?,
                            actions: {
                                let json: Box<str> = row.get(6)?;
                                serde_json::from_str(&json).unwrap()
                            },
                            hints: {
                                let json: Box<str> = row.get(7)?;
                                serde_json::from_str(&json).unwrap()
                            },
                        })
                    })?;

                    self.notifications.uninhibit();
                    _ = self.emit_sender.send(EmitEvent::InhibitStateChanged(
                        self.notifications.inhibited(),
                    ));

                    rows.into_iter()
                        .try_for_each(|notification| self.notifications.add(notification?))?;
                    drop(stmt);

                    self.update_surface_size();
                }
            }
            Event::GetMuted => {
                _ = self.emit_sender.send(EmitEvent::Muted(
                    self.audio.as_ref().map(|a| a.muted()).unwrap_or(true),
                ))
            }
            Event::GetInhibited => {
                _ = self
                    .emit_sender
                    .send(EmitEvent::Inhibited(self.notifications.inhibited()));
            }
            Event::GetHistory => {
                _ = self.emit_sender.send(EmitEvent::HistoryState(self.history));
            }
            Event::Waiting => {
                _ = self
                    .emit_sender
                    .send(EmitEvent::Waiting(self.notifications.waiting()));
            }
        };

        if let Some(surface) = self.surface.as_mut() {
            surface.render(
                self.seat.keyboard.mode,
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
    Waiting(u32),
    ActionInvoked {
        id: u32,
        action_key: Arc<str>,
        token: Arc<str>,
    },
    NotificationClosed {
        id: u32,
        reason: Reason,
    },
    Open {
        uri: Arc<str>,
        token: Option<Arc<str>>,
    },
    List(Vec<String>),
    MuteStateChanged(bool),
    HistoryStateChanged(History),
    InhibitStateChanged(bool),
    Muted(bool),
    HistoryState(History),
    Inhibited(bool),
}

pub enum Event {
    Waiting,
    Dismiss { all: bool, id: u32 },
    Notify(Box<NotificationData>),
    CloseNotification(u32),
    List,
    FocusSurface,
    Mute,
    Unmute,
    GetMuted,
    ShowHistory,
    HideHistory,
    GetHistory,
    Inhibit,
    Uninhibit,
    GetInhibited,
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

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    #[arg(short, long, action = clap::ArgAction::Count)]
    quiet: u8,

    #[arg(short, long, value_name = "FILE", help = "Path to the config file")]
    config: Option<Box<Path>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let mut log_level = LevelFilter::Info;

    (0..cli.verbose).for_each(|_| {
        log_level = match log_level {
            LevelFilter::Error => LevelFilter::Warn,
            LevelFilter::Warn => LevelFilter::Info,
            LevelFilter::Info => LevelFilter::Debug,
            LevelFilter::Debug => LevelFilter::Trace,
            _ => log_level,
        };
    });

    (0..cli.quiet).for_each(|_| {
        log_level = match log_level {
            LevelFilter::Warn => LevelFilter::Error,
            LevelFilter::Info => LevelFilter::Warn,
            LevelFilter::Debug => LevelFilter::Info,
            LevelFilter::Trace => LevelFilter::Debug,
            _ => log_level,
        };
    });

    Builder::new().filter(Some("daemon"), log_level).init();

    let conn = Connection::connect_to_env().expect("Failed to connect to Wayland");
    let (globals, event_queue) = registry_queue_init(&conn)?;
    let qh = event_queue.handle();

    let (emit_sender, emit_receiver) = broadcast::channel(std::mem::size_of::<EmitEvent>());
    let mut event_loop = EventLoop::try_new()?;
    let mut moxnotify = Moxnotify::new(
        &conn,
        qh,
        globals,
        event_loop.handle(),
        emit_sender.clone(),
        cli.config,
    )
    .await?;

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
