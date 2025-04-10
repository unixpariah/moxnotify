use serde::Deserialize;
use std::io::{self, Write};
use zbus::zvariant::Type;

pub enum Event {
    Focus,
    List,
    DismissAll,
    DismissOne(u32),
    Mute,
    Unmute,
    ShowHistory,
    HideHistory,
    Inhibit,
    Uninhibit,
    ToggleHistory,
    ToggleInhibit,
    ToggleMute,
}

#[derive(Default, PartialEq, Clone, Copy, Type, Deserialize)]
pub enum History {
    #[default]
    Hidden,
    Shown,
}

#[zbus::proxy(
    interface = "pl.mox.Notify",
    default_service = "pl.mox.Notify",
    default_path = "/pl/mox/Notify"
)]
trait Notify {
    async fn focus(&self) -> zbus::Result<()>;

    async fn list(&self) -> zbus::Result<Vec<String>>;

    async fn dismiss(&self, all: bool, id: u32) -> zbus::Result<()>;

    async fn mute(&self) -> zbus::Result<()>;

    async fn unmute(&self) -> zbus::Result<()>;

    async fn muted(&self) -> zbus::Result<bool>;

    async fn show_history(&self) -> zbus::Result<()>;

    async fn hide_history(&self) -> zbus::Result<()>;

    async fn history(&self) -> zbus::Result<History>;

    async fn inhibit(&self) -> zbus::Result<()>;

    async fn uninhibit(&self) -> zbus::Result<()>;

    async fn inhibited(&self) -> zbus::Result<bool>;
}

pub async fn emit(event: Event) -> zbus::Result<()> {
    let conn = zbus::Connection::session().await?;
    let notify = NotifyProxy::new(&conn).await?;
    let mut out = io::stdout().lock();

    match event {
        Event::Focus => notify.focus().await?,
        Event::List => {
            let list = notify.list().await?;
            for item in list {
                writeln!(out, "{}", item)?;
            }
        }
        Event::DismissAll => notify.dismiss(true, 0).await?,
        Event::DismissOne(index) => notify.dismiss(false, index).await?,
        Event::Unmute => notify.unmute().await?,
        Event::Mute => notify.mute().await?,
        Event::ToggleMute => {
            if notify.muted().await? {
                notify.unmute().await?
            } else {
                notify.mute().await?
            }
        }
        Event::ShowHistory => notify.show_history().await?,
        Event::HideHistory => notify.hide_history().await?,
        Event::ToggleHistory => {
            if notify.history().await? == History::Shown {
                notify.hide_history().await?
            } else {
                notify.show_history().await?
            }
        }
        Event::Inhibit => notify.inhibit().await?,
        Event::Uninhibit => notify.uninhibit().await?,
        Event::ToggleInhibit => {
            if notify.inhibited().await? {
                notify.uninhibit().await?
            } else {
                notify.inhibit().await?
            }
        }
    }

    Ok(())
}
