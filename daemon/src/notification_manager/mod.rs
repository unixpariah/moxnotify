pub mod notification;
mod notification_view;

use crate::{
    buffers,
    button::ButtonType,
    config::{self, Config, Queue},
    texture_renderer::TextureArea,
    Moxnotify, NotificationData,
};
use calloop::{
    timer::{TimeoutAction, Timer},
    LoopHandle,
};
use glyphon::{FontSystem, TextArea};
use notification::{Notification, NotificationId};
use notification_view::NotificationView;
use std::{ops::Deref, sync::Arc, time::Duration};

pub struct NotificationManager {
    notifications: Vec<Notification>,
    config: Arc<Config>,
    loop_handle: LoopHandle<'static, Moxnotify>,
    selected: Option<NotificationId>,
    font_system: FontSystem,
    notification_view: NotificationView,
}

impl Deref for NotificationManager {
    type Target = Vec<Notification>;

    fn deref(&self) -> &Self::Target {
        &self.notifications
    }
}

impl NotificationManager {
    pub fn new(config: Arc<Config>, loop_handle: LoopHandle<'static, Moxnotify>) -> Self {
        Self {
            notification_view: NotificationView::new(config.max_visible, Arc::clone(&config)),
            font_system: FontSystem::new(),
            loop_handle,
            notifications: Vec::new(),
            selected: None,
            config,
        }
    }

    pub fn notifications(&self) -> &[Notification] {
        &self.notifications
    }

    pub fn notifications_mut(&mut self) -> &mut [Notification] {
        &mut self.notifications
    }

    pub fn data(&self, scale: f32) -> (Vec<buffers::Instance>, Vec<TextArea>, Vec<TextureArea>) {
        let (mut instances, mut text_areas, textures) = self
            .notifications
            .iter()
            .enumerate()
            .filter(|(i, _)| self.notification_view.visible.contains(i))
            .fold(
                (Vec::new(), Vec::new(), Vec::new()),
                |(mut instances, mut text_areas, mut textures), (_, notification)| {
                    let instance = notification.instances(scale);
                    let text = notification.text_area(scale);
                    let texture = notification.icons.textures(
                        notification.style(),
                        &self.config,
                        self.height(),
                        scale,
                    );

                    textures.extend_from_slice(&texture);
                    instances.extend_from_slice(&instance);
                    text_areas.extend_from_slice(&text);

                    (instances, text_areas, textures)
                },
            );

        let total_width = self
            .notifications
            .iter()
            .map(|notification| {
                notification.rendered_extents().x + notification.rendered_extents().width
            })
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or_default();

        if let Some((instance, text_area)) = self.notification_view.prev_data(total_width, scale) {
            instances.push(instance);
            text_areas.push(text_area);
        }

        if let Some((instance, text_area)) = self.notification_view.next_data(total_width, scale) {
            instances.push(instance);
            text_areas.push(text_area);
        }

        (instances, text_areas, textures)
    }

    pub fn get_by_coordinates(&self, x: f64, y: f64) -> Option<&Notification> {
        let mut cumulative_y_offset: f64 = self
            .notification_view
            .prev
            .as_ref()
            .map(|n| n.extents().height)
            .unwrap_or_default()
            .into();

        self.notification_view
            .visible
            .clone()
            .filter_map(|index| self.notifications.get(index))
            .scan(&mut cumulative_y_offset, |current_y, notification| {
                let extents = notification.rendered_extents();
                let notification_height = extents.height as f64;

                let x_within_bounds =
                    x >= extents.x as f64 && x < (extents.x + extents.width) as f64;
                let y_within_bounds = y >= **current_y && y < (**current_y + notification_height);

                if x_within_bounds && y_within_bounds {
                    Some(Some(notification))
                } else {
                    **current_y += notification.extents().height as f64;
                    Some(None)
                }
            })
            .flatten()
            .next()
    }

    pub fn get_button_by_coordinates(&mut self, x: f64, y: f64) -> Option<ButtonType> {
        self.notification_view.visible.clone().find_map(|index| {
            self.notifications.get_mut(index).and_then(|notification| {
                notification
                    .buttons
                    .get_by_coordinates(notification.hovered(), x, y)
            })
        })
    }

    pub fn height(&self) -> f32 {
        let height = self
            .notification_view
            .prev
            .as_ref()
            .map_or(0., |n| n.extents().height);
        self.notification_view
            .visible
            .clone()
            .fold(height, |acc, i| {
                if let Some(notification) = self.notifications.get(i) {
                    let extents = notification.extents();
                    return acc + extents.height;
                };

                acc
            })
            + self
                .notification_view
                .next
                .as_ref()
                .map_or(0., |n| n.extents().height)
    }

    pub fn width(&self) -> f32 {
        let (min_x, max_x) =
            self.notifications
                .iter()
                .fold((f32::MAX, f32::MIN), |(min_x, max_x), notification| {
                    let extents = notification.extents();
                    let left = extents.x + notification.hints.x as f32;
                    let right = extents.x + extents.width + notification.hints.x as f32;
                    (min_x.min(left), max_x.max(right))
                });

        if min_x == f32::MAX || max_x == f32::MIN {
            0.0
        } else {
            max_x - min_x
        }
    }

    pub fn selected(&self) -> Option<NotificationId> {
        self.selected
    }

    pub fn select(&mut self, id: NotificationId) {
        if let Some(old_id) = self.selected.take() {
            self.unhover_notification(old_id);
        }

        if let Some(new_notification) = self.notifications.iter_mut().find(|n| n.id() == id) {
            new_notification.hover();

            let style = new_notification.style();

            let icon_extents = new_notification.icons.extents(new_notification.style());

            let dismiss_button = new_notification
                .buttons
                .buttons()
                .iter()
                .find(|button| button.button_type == ButtonType::Dismiss)
                .map(|b| b.rendered_extents(new_notification.hovered()).width)
                .unwrap_or(0.0);

            new_notification.text.buffer.set_size(
                &mut self.font_system,
                Some(style.width.resolve(0.) - icon_extents.width - dismiss_button),
                None,
            );

            self.selected = Some(id);
            if let Some(token) = new_notification.registration_token.take() {
                self.loop_handle.remove(token);
            }
        }
    }

    pub fn next(&mut self) {
        let next_notification_index = if let Some(id) = self.selected {
            self.notifications
                .iter()
                .position(|n| n.id() == id)
                .map_or(0, |index| {
                    if index + 1 < self.notifications.len() {
                        index + 1
                    } else {
                        0
                    }
                })
        } else {
            0
        };

        if let Some(notification) = self.notifications.get(next_notification_index) {
            self.select(notification.id());
            self.notification_view.next(
                self.height(),
                next_notification_index,
                self.notifications.len(),
            );
        }

        self.notification_view.visible.clone().fold(
            self.notification_view
                .prev
                .as_ref()
                .map(|p| p.extents().height)
                .unwrap_or(0.),
            |acc, i| {
                if let Some(notification) = self.notifications.get_mut(i) {
                    notification.set_position(notification.x, acc);
                    acc + notification.extents().height
                } else {
                    acc
                }
            },
        );

        if self.notifications.get(next_notification_index).is_some() {
            self.notification_view.next(
                self.height(),
                next_notification_index,
                self.notifications.len(),
            );
        }
    }

    pub fn prev(&mut self) {
        let notification_index = if let Some(id) = self.selected {
            self.notifications.iter().position(|n| n.id() == id).map_or(
                self.notifications.len().saturating_sub(1),
                |index| {
                    if index > 0 {
                        index - 1
                    } else {
                        self.notifications.len().saturating_sub(1)
                    }
                },
            )
        } else {
            self.notifications.len().saturating_sub(1)
        };

        if let Some(notification) = self.notifications.get(notification_index) {
            self.select(notification.id());
            self.notification_view.prev(
                self.height(),
                notification_index,
                self.notifications.len(),
            );
        }

        self.notification_view.visible.clone().fold(
            self.notification_view
                .prev
                .as_ref()
                .map(|p| p.extents().height)
                .unwrap_or(0.),
            |acc, i| {
                if let Some(notification) = self.notifications.get_mut(i) {
                    notification.set_position(notification.x, acc);
                    acc + notification.extents().height
                } else {
                    acc
                }
            },
        );

        if self.notifications.get(notification_index).is_some() {
            self.notification_view.prev(
                self.height(),
                notification_index,
                self.notifications.len(),
            );
        }
    }

    pub fn deselect(&mut self) {
        if let Some(old_id) = self.selected.take() {
            self.unhover_notification(old_id);
        }
    }

    pub fn add(&mut self, data: NotificationData) -> anyhow::Result<()> {
        let id = data.id;

        let (y, existing_index) =
            if let Some(index) = self.notifications.iter().position(|n| n.id() == id) {
                let old_notification = self.notifications.remove(index);
                (old_notification.extents().y, Some(index))
            } else {
                (self.height(), None)
            };

        let mut notification =
            Notification::new(Arc::clone(&self.config), &mut self.font_system, data);
        notification.set_position(0.0, y);

        if let Some(timeout) = notification.timeout() {
            let should_set_timer = match self.config.queue {
                Queue::Ordered => self.notifications.is_empty(),
                Queue::Unordered => true,
            };

            if should_set_timer {
                let timer = Timer::from_duration(Duration::from_millis(timeout));
                notification.registration_token = self
                    .loop_handle
                    .insert_source(timer, move |_, _, moxnotify| {
                        moxnotify.notifications.dismiss(id);
                        moxnotify.update_surface_size();
                        if let Some(surface) = moxnotify.surface.as_mut() {
                            let _ = surface.render(
                                &moxnotify.wgpu_state.device,
                                &moxnotify.wgpu_state.queue,
                                &moxnotify.notifications,
                            );
                        }
                        TimeoutAction::Drop
                    })
                    .ok();
            }
        }

        match existing_index {
            Some(index) => self.notifications.insert(index, notification),
            None => self.notifications.push(notification),
        }

        // Maintain selection if replacing
        if self.selected() == Some(id) {
            self.select(id);
        }

        if self.notification_view.visible.end < self.notifications.len() {
            self.notification_view
                .update_notification_count(self.height(), self.notifications.len());
        }

        let x_offset = self
            .notifications
            .iter()
            .map(|n| n.hints.x)
            .min()
            .unwrap_or_default()
            .abs() as f32;

        self.notifications
            .iter_mut()
            .for_each(|n| n.set_position(x_offset, n.y));

        Ok(())
    }

    fn unhover_notification(&mut self, id: NotificationId) {
        if let Some(index) = self.notifications.iter().position(|n| n.id() == id) {
            if let Some(notification) = self.notifications.get_mut(index) {
                notification.unhover();
                let timer = match self.config.queue {
                    Queue::Ordered if index == 0 => notification.timeout(),
                    Queue::Unordered => notification.timeout(),
                    _ => None,
                }
                .map(|t| Timer::from_duration(Duration::from_millis(t)));

                if let Some(timer) = timer {
                    notification.registration_token = self
                        .loop_handle
                        .insert_source(timer, move |_, _, moxnotify| {
                            moxnotify.notifications.dismiss(id);
                            TimeoutAction::Drop
                        })
                        .ok();
                }
            }
        }
    }

    pub fn dismiss(&mut self, id: NotificationId) {
        if let Some(i) = self.notifications.iter().position(|n| n.id() == id) {
            let notification = self.notifications.remove(i);
            if let Some(token) = notification.registration_token {
                self.loop_handle.remove(token);
            }

            if let Some(next_notification) = self.notifications.get(i) {
                if self.selected() == Some(notification.id()) {
                    self.select(next_notification.id());
                }
            }
        }

        if self.notification_view.visible.start >= self.notifications.len() {
            self.notification_view.visible = self.notifications.len().saturating_sub(1)
                ..self.notifications.len().saturating_sub(1) + self.config.max_visible as usize;
        }

        self.notification_view
            .update_notification_count(self.height(), self.notifications.len());

        if self.selected() == Some(id) {
            self.deselect();
        }

        if self.config.queue == Queue::Ordered {
            if let Some(notification) = self.notifications.first_mut() {
                if !notification.hovered() {
                    if let Some(timeout) = notification.timeout() {
                        let timer = Timer::from_duration(Duration::from_millis(timeout));
                        let id = notification.id();
                        notification.registration_token = self
                            .loop_handle
                            .insert_source(timer, move |_, _, moxnotify| {
                                moxnotify.notifications.dismiss(id);
                                TimeoutAction::Drop
                            })
                            .ok();
                    }
                }
            }
        }

        self.notification_view.visible.clone().fold(
            self.notification_view
                .prev
                .as_ref()
                .map(|p| p.extents().height)
                .unwrap_or(0.),
            |acc, i| {
                if let Some(notification) = self.notifications.get_mut(i) {
                    notification.set_position(notification.x, acc);
                    acc + notification.extents().height
                } else {
                    acc
                }
            },
        );

        let x_offset = self
            .notifications
            .iter()
            .map(|notification| notification.hints.x)
            .min()
            .unwrap_or_default()
            .abs();
        self.notifications
            .iter_mut()
            .for_each(|notification| notification.set_position(x_offset as f32, notification.y));
    }
}
