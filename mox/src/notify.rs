use crate::Event;

#[zbus::proxy(
    interface = "pl.unixpariah.Moxnotify",
    default_service = "pl.unixpariah.Moxnotify",
    default_path = "/pl/unixpariah/Moxnotify"
)]
trait Notify {
    async fn focus(&self) -> zbus::Result<()>;
}

pub async fn emit(event: Event) -> zbus::Result<()> {
    let conn = zbus::Connection::session().await?;

    let notify = NotifyProxy::new(&conn).await?;

    match event {
        Event::Focus => notify.focus().await,
    }
}
