use super::notification::Notification;
use crate::{config::Config, wgpu_state::buffers, NotificationData};
use glyphon::{FontSystem, TextArea};
use std::{ops::Range, sync::Arc};

pub struct NotificationView {
    pub visible: Range<usize>,
    prev: Option<Notification>,
    pub next: Option<Notification>,
    max_visible: usize,
    font_system: FontSystem,
    config: Arc<Config>,
}

impl NotificationView {
    pub fn new(max_visible: u32, config: Arc<Config>) -> Self {
        Self {
            config,
            font_system: FontSystem::new(),
            max_visible: max_visible as usize,
            visible: 0..max_visible as usize,
            prev: None,
            next: None,
        }
    }

    pub fn prev(&mut self, index: usize, notification_count: usize, total_height: f32) {
        if index + 1 == notification_count {
            self.visible = (notification_count
                .max(self.max_visible)
                .saturating_sub(self.max_visible))
                ..notification_count.max(self.max_visible);
        } else {
            let first_visible = self.visible.start;
            if index < first_visible {
                let start = index;
                let end = index + self.max_visible;
                self.visible = start..end;
            }
        }
        self.update_notification_count(notification_count, total_height);
    }

    pub fn next(&mut self, index: usize, notification_count: usize, total_height: f32) {
        if index == 0 {
            self.visible = 0..self.max_visible;
        } else {
            let last_visible = self.visible.end.saturating_sub(1);
            if index > last_visible {
                let start = index + 1 - self.max_visible;
                let end = index + 1;
                self.visible = start..end;
            }
        }
        self.update_notification_count(notification_count, total_height);
    }

    pub fn update_notification_count(&mut self, notification_count: usize, total_height: f32) {
        if notification_count <= self.visible.end {
            self.next = None;
            return;
        }

        let summary = format!(
            "({} more)",
            notification_count.saturating_sub(self.visible.end)
        );
        if let Some(notification) = &mut self.next {
            notification.set_text(&summary, "", &mut self.font_system);
        } else {
            self.next = Some(Notification::new(
                Arc::clone(&self.config),
                total_height,
                &mut self.font_system,
                NotificationData {
                    id: 0,
                    actions: [].into(),
                    app_name: "".into(),
                    summary: summary.into(),
                    body: "".into(),
                    hints: Vec::new(),
                    timeout: 0,
                },
            ));
        }
    }

    pub fn prepare_data(&self, scale: f32) -> Option<(buffers::Instance, TextArea)> {
        if let Some(next) = self.next.as_ref() {
            return Some((next.get_instance(scale), next.text_area(scale)));
        }
        None
    }
}
