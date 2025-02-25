use tokio::sync::broadcast;

use crate::EmitEvent;
use std::collections::HashMap;

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
    ) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;

    fn open_file(
        &self,
        parent_window: &str,
        fd: zbus::zvariant::Fd<'_>,
        options: HashMap<&str, zbus::zvariant::Value<'_>>,
    ) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;

    fn open_directory(
        &self,
        parent_window: &str,
        fd: zbus::zvariant::Fd<'_>,
        options: HashMap<&str, zbus::zvariant::Value<'_>>,
    ) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;
}

pub async fn serve(mut receiver: broadcast::Receiver<EmitEvent>) -> zbus::Result<()> {
    let conn = zbus::Connection::session().await?;
    let open_uri = OpenURIProxy::new(&conn).await?;

    tokio::spawn(async move {
        loop {
            match receiver.recv().await {
                Ok(EmitEvent::OpenURI { uri, token, handle }) => {
                    let mut options = HashMap::new();
                    if let Some(token) = token.as_ref() {
                        options.insert("activation_token", zbus::zvariant::Value::new(&**token));
                    }

                    _ = open_uri.open_URI(&handle, &uri, options).await;
                }
                _ => {}
            }
        }
    });

    Ok(())
}
