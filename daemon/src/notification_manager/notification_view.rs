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

    pub fn prev(&mut self, index: usize, notification_count: usize) {
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
        self.update_notification_count(notification_count);
    }

    pub fn next(&mut self, index: usize, notification_count: usize) {
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
        self.update_notification_count(notification_count);
    }

    pub fn update_notification_count(&mut self, notification_count: usize) {
        if notification_count > self.visible.end {
            let summary = self.config.prev.format.replace(
                "{}",
                &notification_count
                    .saturating_sub(self.visible.end)
                    .to_string(),
            );
            if let Some(notification) = &mut self.next {
                notification.set_text(&summary, "", &mut self.font_system);
            } else {
                self.next = Some(Notification::new(
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
                    Arc::clone(&self.config),
                    &mut self.font_system,
                    NotificationData {
                        app_name: "prev_notification_count".into(),
                        summary: summary.into(),
                        ..Default::default()
                    },
                ))
            }
        } else {
            self.prev = None;
        }
    }

    pub fn prev_data(
        &self,
        total_height: &mut f32,
        scale: f32,
    ) -> Option<(buffers::Instance, TextArea)> {
        if let Some(prev) = self.prev.as_ref() {
            let extents = prev.rendered_extents();
            let style = &self.config.prev;
            let instance = buffers::Instance {
                rect_pos: [extents.x, *total_height],
                rect_size: [
                    extents.width - style.border.size.left - style.border.size.right,
                    extents.height - style.border.size.top - style.border.size.bottom,
                ],
                rect_color: style.background_color.to_linear(),
                border_radius: style.border.radius.into(),
                border_size: style.border.size.into(),
                border_color: style.border_color.into(),
                scale,
            };

            let text_area = prev.text_area(*total_height, scale);
            *total_height += prev.extents().height - prev.style().margin.top;

            return Some((instance, text_area));
        }

        None
    }

    pub fn next_data(
        &self,
        total_height: f32,
        scale: f32,
    ) -> Option<(buffers::Instance, TextArea)> {
        if let Some(next) = self.next.as_ref() {
            let extents = next.rendered_extents();
            let style = &self.config.prev;
            let instance = buffers::Instance {
                rect_pos: [extents.x, total_height + next.style().margin.top],
                rect_size: [
                    extents.width - style.border.size.left - style.border.size.right,
                    extents.height - style.border.size.top - style.border.size.bottom,
                ],
                rect_color: style.background_color.to_linear(),
                border_radius: style.border.radius.into(),
                border_size: style.border.size.into(),
                border_color: style.border_color.into(),
                scale,
            };

            return Some((
                instance,
                next.text_area(total_height + next.style().margin.top, scale),
            ));
        }

        None
    }
}
