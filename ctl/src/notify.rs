// In notify.rs
pub enum Event {
    Focus,
    List,
    DismissAll,
    DismissOne(u32),
}

#[zbus::proxy(
    interface = "pl.mox.Notify",
    default_service = "pl.mox.Notify",
    default_path = "/pl/mox/Notify"
)]
trait Notify {
    async fn focus(&self) -> zbus::Result<()>;

    async fn list(&self) -> zbus::Result<()>;

    async fn dismiss(&self, all: bool, id: u32) -> zbus::Result<()>;
}

pub async fn emit(event: Event) -> zbus::Result<()> {
    let conn = zbus::Connection::session().await?;

    let notify = NotifyProxy::new(&conn).await?;

    match event {
        Event::Focus => notify.focus().await,
        Event::List => notify.list().await,
        Event::DismissAll => notify.dismiss(true, 0).await,
        Event::DismissOne(index) => notify.dismiss(false, index).await,
    }
}
