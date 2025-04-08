use crate::{EmitEvent, Event};
use tokio::sync::broadcast;
use zbus::fdo::RequestNameFlags;

struct MoxnotifyInterface {
    event_sender: calloop::channel::Sender<Event>,
    emit_receiver: broadcast::Receiver<EmitEvent>,
}

#[zbus::interface(name = "pl.mox.Notify")]
impl MoxnotifyInterface {
    async fn focus(&self) {
        if let Err(e) = self.event_sender.send(Event::FocusSurface) {
            log::warn!("{}", e);
        }
    }

    async fn dismiss(&self, all: bool, id: u32) {
        if let Err(e) = self.event_sender.send(Event::Dismiss { all, id }) {
            log::warn!("{}", e);
        }
    }

    async fn list(&mut self) -> Vec<String> {        
        if let Err(e) = self.event_sender.send(Event::List) {
            log::warn!("{}", e);
        }
        if let Ok(EmitEvent::List(list)) = self.emit_receiver.recv().await {
            list
        } else {
            Vec::new()
        }
    }
}

pub async fn serve(event_sender: calloop::channel::Sender<Event>,
    emit_receiver: broadcast::Receiver<EmitEvent>,
) -> zbus::Result<()> {
    let server = MoxnotifyInterface { event_sender, emit_receiver };

    let conn = zbus::connection::Builder::session()?
        .serve_at("/pl/mox/Notify", server)?
        .build()
        .await?;

    conn.request_name_with_flags("pl.mox.Notify", RequestNameFlags::DoNotQueue.into())
        .await?;

    std::future::pending::<()>().await;

    Ok(())
}
