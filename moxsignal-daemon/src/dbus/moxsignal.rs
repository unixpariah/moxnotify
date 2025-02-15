use crate::Event;
use zbus::fdo::RequestNameFlags;

struct MoxsignalInterface {
    event_sender: calloop::channel::Sender<Event>,
}

#[zbus::interface(name = "pl.unixpariah.moxsignal")]
impl MoxsignalInterface {
    async fn focus(&self) {
        if let Err(e) = self.event_sender.send(Event::FocusSurface) {
            log::warn!("{}", e);
        }
    }
}

pub async fn serve(event_sender: calloop::channel::Sender<Event>) -> zbus::Result<()> {
    let server = MoxsignalInterface { event_sender };

    let conn = zbus::connection::Builder::session()?
        .serve_at("/pl/unixpariah/moxsignal", server)?
        .build()
        .await?;

    if let Err(e) = conn
        .request_name_with_flags(
            "pl.unixpariah.moxsignal",
            RequestNameFlags::DoNotQueue.into(),
        )
        .await
    {
        log::error!("{e}");
    }

    Ok(())
}
