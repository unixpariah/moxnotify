use std::io::{self, Write};

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

    async fn show_history(&self) -> zbus::Result<()>;

    async fn hide_history(&self) -> zbus::Result<()>;

    async fn inhibit(&self) -> zbus::Result<()>;

    async fn uninhibit(&self) -> zbus::Result<()>;
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
        Event::ShowHistory => notify.show_history().await?,
        Event::HideHistory => notify.hide_history().await?,
        Event::Inhibit => notify.inhibit().await?,
        Event::Uninhibit => notify.uninhibit().await?,
    }

    Ok(())
}
