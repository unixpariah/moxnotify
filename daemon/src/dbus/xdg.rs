use crate::{image_data::ImageData, EmitEvent, Event, Hint, Image, Urgency};
use std::{collections::HashMap, path::Path, sync::Arc};
use tokio::sync::broadcast;
use zbus::{fdo::RequestNameFlags, object_server::SignalEmitter, zvariant::Str};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Default)]
pub struct NotificationData {
    pub id: u32,
    pub app_name: Box<str>,
    pub app_icon: Option<Box<str>>,
    pub summary: Box<str>,
    pub body: Box<str>,
    pub timeout: i32,
    pub actions: Box<[(Arc<str>, Arc<str>)]>,
    pub hints: Vec<Hint>,
}

struct NotificationsImpl {
    next_id: u32,
    event_sender: calloop::channel::Sender<Event>,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl NotificationsImpl {
    async fn get_capabilities(&self) -> &[&'static str] {
        &[
            // "action-icons",
            //"actions",
            "body",
            "body-hyperlinks",
            "body-images",
            "body-markup",
            // "icon-multi",
            "icon-static",
            "persistence",
            //"sound",
        ]
    }

    #[allow(clippy::too_many_arguments)]
    async fn notify(
        &mut self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: Box<[&str]>,
        hints: HashMap<&str, zbus::zvariant::Value<'_>>,
        expire_timeout: i32,
    ) -> u32 {
        let id = match replaces_id == 0 {
            true => {
                let id = self.next_id;
                self.next_id = self.next_id.checked_add(1).unwrap_or(1);
                id
            }
            false => replaces_id,
        };

        let hints: Vec<_> = hints
            .into_iter()
            .filter_map(|(k, v)| match k {
                "action-icons" => bool::try_from(v).map(Hint::ActionIcons).ok(),
                "category" => Str::try_from(v)
                    .map(|s| Hint::Category(s.as_str().into()))
                    .ok(),
                "value" => i32::try_from(v).map(Hint::Value).ok(),
                "desktop-entry" => Str::try_from(v)
                    .map(|s| Hint::DesktopEntry(s.as_str().into()))
                    .ok(),
                "resident" => bool::try_from(v).map(Hint::Resident).ok(),
                "sound-file" => Str::try_from(v)
                    .map(|s| Hint::SoundFile(Path::new(s.as_str()).into()))
                    .ok(),
                "sound-name" => Str::try_from(v)
                    .map(|v| Hint::SoundName(v.as_str().into()))
                    .ok(),
                "suppress-sound" => bool::try_from(v).map(Hint::SuppressSound).ok(),
                "transient" => bool::try_from(v).map(Hint::Transient).ok(),
                "x" => i32::try_from(v).map(Hint::X).ok(),
                "y" => i32::try_from(v).map(Hint::Y).ok(),
                "urgency" => match u8::try_from(v) {
                    Ok(0) => Some(Hint::Urgency(Urgency::Low)),
                    Ok(1) => Some(Hint::Urgency(Urgency::Normal)),
                    Ok(2) => Some(Hint::Urgency(Urgency::Critical)),
                    Err(_) => {
                        log::error!("Invalid urgency data");
                        Some(Hint::Urgency(Urgency::Low))
                    }
                    _ => {
                        log::warn!("Unkown urgency level");
                        Some(Hint::Urgency(Urgency::Low))
                    }
                },
                "image-path" | "image_path" => Str::try_from(v).ok().map(|s| {
                    if let Ok(path) = url::Url::parse(&s) {
                        Hint::Image(Image::File(Path::new(path.as_str()).into()))
                    } else {
                        Hint::Image(Image::Name(s.as_str().into()))
                    }
                }),
                "image-data" | "image_data" | "icon_data" => match v {
                    zbus::zvariant::Value::Structure(v) => match ImageData::try_from(v) {
                        Ok(image) => Some(Hint::Image(Image::Data(image))),
                        Err(err) => {
                            log::warn!("Invalid image data: {err}");
                            None
                        }
                    },
                    _ => {
                        log::warn!("Invalid value for hint: {k}");
                        None
                    }
                },
                _ => {
                    log::warn!("Unknown hint: {k}");
                    None
                }
            })
            .collect();

        let app_icon: Option<Box<str>> = if app_icon.is_empty() {
            None
        } else {
            Some(app_icon.into())
        };

        if let Err(e) = self.event_sender.send(Event::Notify(NotificationData {
            id,
            app_name: app_name.into(),
            summary: summary.into(),
            body: body.into(),
            timeout: expire_timeout,
            actions: actions
                .chunks_exact(2)
                .map(|action| (action[0].into(), action[1].into()))
                .collect(),
            hints,
            app_icon,
        })) {
            log::error!("{e}");
        }

        id
    }

    async fn close_notification(&self, id: u32) -> zbus::fdo::Result<()> {
        if let Err(e) = self.event_sender.send(Event::CloseNotification(id)) {
            log::error!("Failed to send CloseNotification({id}) event: {e}");
        }

        Ok(())
    }

    async fn get_server_information(
        &self,
    ) -> zbus::fdo::Result<(&'static str, &'static str, &'static str, &'static str)> {
        Ok(("moxnotify", "mox", VERSION, "1.2"))
    }

    #[zbus(signal)]
    async fn notification_closed(
        signal_emitter: &SignalEmitter<'_>,
        id: u32,
        reason: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn action_invoked(
        signal_emitter: &SignalEmitter<'_>,
        id: u32,
        action_key: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn activation_token(
        signal_emitter: &SignalEmitter<'_>,
        id: u32,
        activation_token: &str,
    ) -> zbus::Result<()>;
}

pub async fn serve(
    event_sender: calloop::channel::Sender<Event>,
    mut emit_receiver: broadcast::Receiver<EmitEvent>,
) -> zbus::Result<()> {
    let server = NotificationsImpl {
        next_id: 1,
        event_sender,
    };

    let conn = zbus::connection::Builder::session()?
        .serve_at("/org/freedesktop/Notifications", server)?
        .build()
        .await?;

    if let Err(e) = conn
        .request_name_with_flags(
            "org.freedesktop.Notifications",
            RequestNameFlags::DoNotQueue.into(),
        )
        .await
    {
        log::error!("{e}, is another daemon running?");
        std::process::exit(0);
    }

    let iface = conn
        .object_server()
        .interface::<_, NotificationsImpl>("/org/freedesktop/Notifications")
        .await?;

    tokio::spawn(async move {
        loop {
            match emit_receiver.recv().await {
                Ok(EmitEvent::ActionInvoked {
                    id,
                    action_key,
                    token,
                }) => {
                    if let Err(e) =
                        NotificationsImpl::activation_token(iface.signal_emitter(), id, &token)
                            .await
                    {
                        log::error!("Failed to emmit activation token signal: {e}");
                    }

                    if let Err(e) =
                        NotificationsImpl::action_invoked(iface.signal_emitter(), id, &action_key)
                            .await
                    {
                        log::error!("Failed to emmit action invoked signal: {e}");
                    }
                }
                Ok(EmitEvent::NotificationClosed { id, reason }) => {
                    if let Err(e) =
                        NotificationsImpl::notification_closed(iface.signal_emitter(), id, reason)
                            .await
                    {
                        log::error!("Failed to emmit notification closed signal: {e}");
                    }
                }
                Err(e) => log::error!("Failed to receive emit event: {e}"),
                _ => {}
            };
        }
    });

    Ok(())
}
