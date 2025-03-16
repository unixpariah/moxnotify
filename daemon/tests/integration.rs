use std::collections::HashMap;

#[derive(Default)]
struct Notification<'a> {
    app_name: &'a str,
    replaces_id: u32,
    app_icon: &'a str,
    summary: &'a str,
    body: &'a str,
    actions: Box<[&'a str]>,
    hints: HashMap<&'a str, zbus::zvariant::Value<'a>>,
    expire_timeout: i32,
}

#[zbus::proxy(
    interface = "org.freedesktop.Notifications",
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
trait Notifications {
    async fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: Box<[&str]>,
        hints: HashMap<&str, zbus::zvariant::Value<'_>>,
        expire_timeout: i32,
    ) -> zbus::Result<u32>;
}

async fn emit(notification: Notification<'_>) -> zbus::Result<u32> {
    let conn = zbus::Connection::session().await?;
    let notify = NotificationsProxy::new(&conn).await?;

    notify
        .notify(
            notification.app_name,
            notification.replaces_id,
            notification.app_icon,
            notification.summary,
            notification.body,
            notification.actions,
            notification.hints,
            notification.expire_timeout,
        )
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::process::Command;

    #[tokio::test]
    async fn image_test() {
        let mut hints = HashMap::new();
        hints.insert("image-path", "zen-beta".into());
        let notification = Notification {
            summary: "image-path test",
            hints,
            ..Default::default()
        };
        emit(notification).await;

        let notification = Notification {
            summary: "app_icon test",
            app_icon: "zen-beta",
            ..Default::default()
        };
        emit(notification).await;

        let mut hints = HashMap::new();
        hints.insert("image-path", "zen-beta".into());
        let notification = Notification {
            summary: "app_icon and image-path test",
            app_icon: "zen-beta",
            hints,
            ..Default::default()
        };
        emit(notification).await;
    }

    #[tokio::test]
    async fn replaces_id_test() {
        let notification = Notification {
            summary: "replaces_id test",
            body: "Notification to be replaced",
            ..Default::default()
        };
        let id = emit(notification).await.unwrap();

        std::thread::sleep(Duration::from_secs(3));

        let notification = Notification {
            replaces_id: id,
            summary: "replaces_id test",
            body: "Replacing notification",
            ..Default::default()
        };
        emit(notification).await;
    }

    #[tokio::test]
    async fn expire_test() {
        let notification = Notification {
            summary: "expire test",
            body: "Expires in 5 seconds",
            expire_timeout: 5000,
            ..Default::default()
        };
        emit(notification).await;
    }

    #[tokio::test]
    async fn progress_test() {
        let mut hints = HashMap::new();
        hints.insert("image-path", "zen-beta".into());

        let notification = Notification {
            summary: "expire test",
            body: "Expires in 5 seconds",
            expire_timeout: 5000,
            hints,
            ..Default::default()
        };
        emit(notification).await;
    }
}
