use crate::Event;
use zbus::fdo::RequestNameFlags;

struct MoxalertInterface {
    event_sender: calloop::channel::Sender<Event>,
}

#[zbus::interface(name = "pl.unixpariah.moxalert")]
impl MoxalertInterface {
    async fn focus(&self) {
        if let Err(e) = self.event_sender.send(Event::FocusSurface) {
            log::warn!("{}", e);
        }
    }
}

pub async fn serve(event_sender: calloop::channel::Sender<Event>) -> zbus::Result<()> {
    let server = MoxalertInterface { event_sender };

    let conn = zbus::connection::Builder::session()?
        .serve_at("/pl/unixpariah/moxalert", server)?
        .build()
        .await?;

    if let Err(e) = conn
        .request_name_with_flags(
            "pl.unixpariah.moxalert",
            RequestNameFlags::DoNotQueue.into(),
        )
        .await
    {
        log::error!("{e}");
    }

    Ok(())
}
