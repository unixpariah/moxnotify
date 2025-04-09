use crate::{EmitEvent, Event};
use tokio::sync::broadcast;
use zbus::{fdo::RequestNameFlags, object_server::SignalEmitter};

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

    async fn mute(&self) {
        if let Err(e) = self.event_sender.send(Event::Mute) {
            log::warn!("{}", e);
        }
    }

    async fn unmute(&self) {
        if let Err(e) = self.event_sender.send(Event::Unmute) {
            log::warn!("{}", e);
        }
    }

    #[zbus(signal)]
    async fn mute_state_changed(
        signal_emitter: &SignalEmitter<'_>,
        muted: bool,
    ) -> zbus::Result<()>;
}

pub async fn serve(
    event_sender: calloop::channel::Sender<Event>,
    mut emit_receiver: broadcast::Receiver<EmitEvent>,
) -> zbus::Result<()> {
    let server = MoxnotifyInterface {
        event_sender,
        emit_receiver: emit_receiver.resubscribe(),
    };

    let conn = zbus::connection::Builder::session()?
        .serve_at("/pl/mox/Notify", server)?
        .build()
        .await?;

    conn.request_name_with_flags("pl.mox.Notify", RequestNameFlags::DoNotQueue.into())
        .await?;

    let iface = conn
        .object_server()
        .interface::<_, MoxnotifyInterface>("/pl/mox/Notify")
        .await?;

    tokio::spawn(async move {
        loop {
            match emit_receiver.recv().await {
                Ok(EmitEvent::MuteStateChanged(muted)) => {
                    if let Err(e) =
                        MoxnotifyInterface::mute_state_changed(iface.signal_emitter(), muted).await
                    {
                        log::error!("Failed to emmit activation token signal: {e}");
                    }
                }
                Err(e) => log::error!("Failed to receive emit event: {e}"),
                _ => {}
            };
        }
    });

    Ok(())
}
