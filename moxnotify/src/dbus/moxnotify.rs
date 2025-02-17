use crate::Event;
use zbus::fdo::RequestNameFlags;

struct MoxnotifyInterface {
    event_sender: calloop::channel::Sender<Event>,
}

#[zbus::interface(name = "pl.mox.Notify")]
impl MoxnotifyInterface {
    async fn focus(&self) {
        if let Err(e) = self.event_sender.send(Event::FocusSurface) {
            log::warn!("{}", e);
        }
    }
}

pub async fn serve(event_sender: calloop::channel::Sender<Event>) -> zbus::Result<()> {
    let server = MoxnotifyInterface { event_sender };

    let conn = zbus::connection::Builder::session()?
        .serve_at("/pl/mox/Notify", server)?
        .build()
        .await?;

    conn.request_name_with_flags("pl.mox.Notify", RequestNameFlags::DoNotQueue.into())
        .await?;

    std::future::pending::<()>().await;

    Ok(())
}
