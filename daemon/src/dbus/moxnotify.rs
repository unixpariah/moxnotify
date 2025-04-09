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
            log::error!("{}", e);
        }
    }

    async fn dismiss(&self, all: bool, id: u32) {
        if let Err(e) = self.event_sender.send(Event::Dismiss { all, id }) {
            log::error!("{}", e);
        }
    }

    async fn list(&mut self) -> Vec<String> {
        if let Err(e) = self.event_sender.send(Event::List) {
            log::error!("{}", e);
        }
        if let Ok(EmitEvent::List(list)) = self.emit_receiver.recv().await {
            list
        } else {
            Vec::new()
        }
    }

    async fn mute(&self) {
        if let Err(e) = self.event_sender.send(Event::Mute) {
            log::error!("{}", e);
        }
    }

    async fn unmute(&self) {
        if let Err(e) = self.event_sender.send(Event::Unmute) {
            log::error!("{}", e);
        }
    }

    #[zbus(signal)]
    async fn mute_state_changed(
        signal_emitter: &SignalEmitter<'_>,
        muted: bool,
    ) -> zbus::Result<()>;

    async fn show_history(&self) {
        if let Err(e) = self.event_sender.send(Event::ShowHistory) {
            log::error!("{}", e);
        }
    }

    async fn hide_history(&self) {
        if let Err(e) = self.event_sender.send(Event::HideHistory) {
            log::error!("{}", e);
        }
    }

    #[zbus(signal)]
    async fn history_state_changed(
        signal_emitter: &SignalEmitter<'_>,
        muted: bool,
    ) -> zbus::Result<()>;

    async fn inhibit(&self) {
        if let Err(e) = self.event_sender.send(Event::Inhibit) {
            log::error!("{}", e);
        }
    }

    async fn uninhibit(&self) {
        if let Err(e) = self.event_sender.send(Event::Uninhibit) {
            log::error!("{}", e);
        }
    }

    #[zbus(signal)]
    async fn inhibit_changed(
        signal_emitter: &SignalEmitter<'_>,
        inhibited: bool,
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
                        MoxnotifyInterfaceSignals::mute_state_changed(iface.signal_emitter(), muted)
                            .await
                    {
                        log::error!("{e}");
                    }
                }
                Ok(EmitEvent::HistoryStateChanged(history)) => {
                    if let Err(e) = MoxnotifyInterfaceSignals::history_state_changed(
                        iface.signal_emitter(),
                        history,
                    )
                    .await
                    {
                        log::error!("{e}");
                    }
                }
                Ok(EmitEvent::Inhibited(inhibited)) => {
                    if let Err(e) = MoxnotifyInterfaceSignals::inhibit_changed(
                        iface.signal_emitter(),
                        inhibited,
                    )
                    .await
                    {
                        log::error!("{e}");
                    }
                }
                Err(e) => log::error!("{e}"),
                _ => {}
            };
        }
    });

    Ok(())
}
