use crate::Event;

#[zbus::proxy(
    interface = "pl.mox.Notify",
    default_service = "pl.mox.Notify",
    default_path = "/pl/mox/Notify"
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
