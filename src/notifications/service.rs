use tokio::sync::broadcast;

use crate::notifications::dto::{AutomationAlertStreamEvent, UserNotificationEvent};

#[derive(Clone)]
pub struct NotificationService {
    sender: broadcast::Sender<UserNotificationEvent>,
}

impl NotificationService {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(512);

        Self { sender }
    }

    pub fn publish_alert(&self, user_id: &str, alert: AutomationAlertStreamEvent) {
        let _ = self.sender.send(UserNotificationEvent {
            alert,
            user_id: user_id.to_owned(),
        });
    }

    pub fn subscribe(&self) -> broadcast::Receiver<UserNotificationEvent> {
        self.sender.subscribe()
    }
}
