use super::notification::Notification;
use crate::{buffers, config::Config, NotificationData};
use glyphon::{FontSystem, TextArea};
use std::{ops::Range, sync::Arc};

pub struct NotificationView {
    pub visible: Range<usize>,
    pub prev: Option<Notification>,
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

    pub fn prev(&mut self, total_height: f32, index: usize, notification_count: usize) {
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
        self.update_notification_count(total_height, notification_count);
    }

    pub fn next(&mut self, total_height: f32, index: usize, notification_count: usize) {
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
        self.update_notification_count(total_height, notification_count);
    }

    pub fn update_notification_count(&mut self, mut total_height: f32, notification_count: usize) {
        if self.visible.start > 0 {
            let summary = self
                .config
                .next
                .format
                .replace("{}", &self.visible.start.to_string());
            if let Some(notification) = &mut self.prev {
                notification.set_text(&summary, "", &mut self.font_system);
            } else {
                self.prev = Some(Notification::new(
                    0.,
                    Arc::clone(&self.config),
                    &mut self.font_system,
                    NotificationData {
                        app_name: "prev_notification_count".into(),
                        summary: summary.into(),
                        ..Default::default()
                    },
                ));

                total_height += self
                    .prev
                    .as_ref()
                    .map(|p| p.extents().height)
                    .unwrap_or_default();
            }
        } else {
            total_height -= self
                .prev
                .as_ref()
                .map(|p| p.extents().height)
                .unwrap_or_default();
            self.prev = None;
        };

        if notification_count > self.visible.end {
            let summary = self.config.prev.format.replace(
                "{}",
                &notification_count
                    .saturating_sub(self.visible.end)
                    .to_string(),
            );
            if let Some(notification) = &mut self.next {
                notification.set_text(&summary, "", &mut self.font_system);
                notification.set_y(total_height - notification.extents().height);
            } else {
                self.next = Some(Notification::new(
                    total_height,
                    Arc::clone(&self.config),
                    &mut self.font_system,
                    NotificationData {
                        app_name: "next_notification_count".into(),
                        summary: summary.into(),
                        ..Default::default()
                    },
                ));
            }
        } else {
            self.next = None;
        }
    }

    pub fn prev_data(&self, scale: f32) -> Option<(buffers::Instance, TextArea)> {
        if let Some(prev) = self.prev.as_ref() {
            let extents = prev.rendered_extents();
            let style = &self.config.prev;
            let instance = buffers::Instance {
                rect_pos: [extents.x, extents.y],
                rect_size: [
                    extents.width - style.border.size.left - style.border.size.right,
                    extents.height - style.border.size.top - style.border.size.bottom,
                ],
                rect_color: style.background_color.to_linear(&crate::Urgency::Low),
                border_radius: style.border.radius.into(),
                border_size: style.border.size.into(),
                border_color: style.border_color.into(),
                scale,
            };

            return Some((instance, prev.text_area(scale).swap_remove(0)));
        }

        None
    }

    pub fn next_data(&self, scale: f32) -> Option<(buffers::Instance, TextArea)> {
        if let Some(next) = self.next.as_ref() {
            let extents = next.rendered_extents();
            let style = &self.config.prev;
            let instance = buffers::Instance {
                rect_pos: [extents.x, extents.y],
                rect_size: [
                    extents.width - style.border.size.left - style.border.size.right,
                    extents.height - style.border.size.top - style.border.size.bottom,
                ],
                rect_color: style.background_color.to_linear(&crate::Urgency::Low),
                border_radius: style.border.radius.into(),
                border_size: style.border.size.into(),
                border_color: style.border_color.into(),
                scale,
            };

            return Some((instance, next.text_area(scale).swap_remove(0)));
        }

        None
    }
}
