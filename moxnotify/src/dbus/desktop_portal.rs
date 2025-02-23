use crate::EmitEvent;
use std::{collections::HashMap, sync::mpmc};

#[zbus::proxy(
    interface = "org.freedesktop.portal.OpenURI",
    default_service = "org.freedesktop.portal.Desktop",
    default_path = "/org/freedesktop/portal/desktop"
)]
trait OpenURI {
    fn open_URI(
        &self,
        parent_window: &str,
        uri: &str,
        options: HashMap<&str, zbus::zvariant::Value<'_>>,
    ) -> zbus::Result<()>;
}

pub async fn serve(receiver: mpmc::Receiver<EmitEvent>) -> zbus::Result<()> {
    let conn = zbus::Connection::session().await?;
    let open_uri = OpenURIProxy::new(&conn).await?;

    tokio::spawn(async move {
        loop {
            if let Ok(EmitEvent::OpenURI { uri, token, handle }) = receiver.recv() {
                let mut options = HashMap::new();
                if let Some(token) = token.as_ref() {
                    options.insert("activation_token", zbus::zvariant::Value::new(&**token));
                }

                _ = open_uri
                    .open_URI(&handle.unwrap_or("".into()), &uri, options)
                    .await;
            }
        }
    });

    Ok(())
}
