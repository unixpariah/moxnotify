pub mod notification;
mod notification_view;

use crate::{
    buffers,
    button::{Button, ButtonType},
    config::{self, Config, Key, Queue},
    surface::Surface,
    texture_renderer::TextureArea,
    EmitEvent, Moxnotify, NotificationData,
};
use calloop::{
    timer::{TimeoutAction, Timer},
    LoopHandle,
};
use glyphon::{FontSystem, TextArea};
use notification::{Notification, NotificationId};
use notification_view::NotificationView;
use std::{cell::RefMut, ops::Range};
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

    pub fn view(&self) -> &Range<usize> {
        &self.notification_view.visible
    }

    pub fn notifications(&self) -> &[Notification] {
        &self.notifications
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
                    let instance = notification.get_instance(scale);
                    let text = notification.text_area(scale);
                    let texture = notification.textures(self.height(), scale);

                    textures.extend_from_slice(&texture);
                    instances.extend_from_slice(&instance);
                    text_areas.push(text);

                    (instances, text_areas, textures)
                },
            );

        if let Some((instance, text_area)) = self.notification_view.prev_data(scale) {
            instances.push(instance);
            text_areas.push(text_area);
        }

        if let Some((instance, text_area)) = self.notification_view.next_data(scale) {
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

    pub fn get_button_by_coordinates(&self, x: f64, y: f64) -> Option<RefMut<Button>> {
        let mut cumulative_y_offset = self
            .notification_view
            .prev
            .as_ref()
            .map(|n| n.extents().height)
            .unwrap_or_default() as f64
            + 10.;

        self.notification_view
            .visible
            .clone()
            .filter_map(|index| self.notifications.get(index))
            .scan(&mut cumulative_y_offset, |current_y, notification| {
                let style = notification.style();
                let extents = notification.extents();
                let notification_height = extents.height as f64;
                let notification_x = (extents.x + style.padding.left) as f64;
                let notification_width = extents.width as f64;

                let x_within_bounds =
                    x >= notification_x && x < (notification_x + notification_width);
                let y_within_bounds = y >= **current_y && y < (**current_y + notification_height);

                if x_within_bounds && y_within_bounds {
                    let local_x = x - notification_x;
                    let local_y = y - **current_y;
                    Some(notification.buttons.get_by_coordinates(local_x, local_y))
                } else {
                    **current_y += notification.extents().height as f64;
                    Some(None)
                }
            })
            .flatten()
            .next()
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
        self.notifications.first().map_or(0.0, |notification| {
            let extents = notification.extents();
            extents.x + extents.width
        })
    }

    pub fn selected(&self) -> Option<NotificationId> {
        self.selected
    }

    pub fn select(&mut self, id: NotificationId) {
        if Some(id) == self.selected {
            return;
        }

        if let Some(old_id) = self.selected.take() {
            self.unhover_notification(old_id);
        }

        if let Some(new_notification) = self.notifications.iter_mut().find(|n| n.id() == id) {
            new_notification.hover();

            let style = new_notification.style();

            let icon_extents = new_notification.icon_extents();

            let dismiss_button = new_notification
                .buttons
                .iter()
                .find(|button| button.borrow().button_type == ButtonType::Dismiss)
                .map(|b| b.borrow().extents().width)
                .unwrap_or(0.0);

            new_notification.text.buffer.set_size(
                &mut self.font_system,
                Some(style.width - icon_extents.0 - dismiss_button),
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
                    notification.set_y(acc);
                    acc + notification.extents().height
                } else {
                    acc
                }
            },
        );
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
                    notification.set_y(acc);
                    acc + notification.extents().height
                } else {
                    acc
                }
            },
        );
    }

    pub fn deselect(&mut self) {
        if let Some(old_id) = self.selected.take() {
            self.unhover_notification(old_id);
        }
    }

    pub fn add(&mut self, data: NotificationData) -> anyhow::Result<()> {
        let id = data.id;

        let mut notification = Notification::new(
            self.height(),
            Arc::clone(&self.config),
            &mut self.font_system,
            data,
        );
        notification.set_y(notification.extents().y);

        match self.config.queue {
            Queue::Ordered => {
                if self.notifications.is_empty() {
                    if let Some(timeout) = notification.timeout() {
                        let timer = Timer::from_duration(Duration::from_millis(timeout));

                        notification.registration_token = self
                            .loop_handle
                            .insert_source(timer, move |_, _, moxnotify| {
                                moxnotify.dismiss_notification(id);
                                TimeoutAction::Drop
                            })
                            .ok();
                    }
                }
            }
            Queue::Unordered => {
                if let Some(timeout) = notification.timeout() {
                    let timer = Timer::from_duration(Duration::from_millis(timeout));
                    notification.registration_token = self
                        .loop_handle
                        .insert_source(timer, move |_, _, moxnotify| {
                            moxnotify.dismiss_notification(id);
                            TimeoutAction::Drop
                        })
                        .ok()
                }
            }
        }

        self.notifications.push(notification);

        if self.notification_view.visible.end < self.notifications.len() {
            self.notification_view
                .update_notification_count(self.height(), self.notifications.len());
        }

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
                            moxnotify.dismiss_notification(id);
                            TimeoutAction::Drop
                        })
                        .ok();
                }
            }
        }
    }
}

impl Moxnotify {
    pub fn dismiss_notification(&mut self, id: NotificationId) {
        if let Some(i) = self.notifications.iter().position(|n| n.id() == id) {
            if !self.notifications.notification_view.visible.contains(&i) {
                return;
            }
        }

        if let Err(e) = self
            .emit_sender
            .send(EmitEvent::NotificationClosed { id, reason: 0 })
        {
            log::error!("Failed to emit NotificationClosed event: {e}");
        }
        self.notifications.notifications.retain(|n| {
            if n.id() == id {
                if let Some(token) = n.registration_token {
                    self.loop_handle.remove(token);
                }
                false
            } else {
                true
            }
        });

        if self.notifications.notification_view.visible.start >= self.notifications.len() {
            self.notifications.notification_view.visible =
                self.notifications.len().saturating_sub(1)
                    ..self.notifications.len().saturating_sub(1) + self.config.max_visible as usize;
        }

        self.notifications
            .notification_view
            .update_notification_count(self.notifications.height(), self.notifications.len());

        if self.notifications.selected == Some(id) {
            self.deselect_notification();
        }

        if self.config.queue == Queue::Ordered {
            if let Some(notification) = self.notifications.notifications.first_mut() {
                if !notification.hovered() {
                    if let Some(timeout) = notification.timeout() {
                        let timer = Timer::from_duration(Duration::from_millis(timeout));
                        let id = notification.id();
                        notification.registration_token = self
                            .loop_handle
                            .insert_source(timer, move |_, _, moxnotify| {
                                moxnotify.dismiss_notification(id);
                                TimeoutAction::Drop
                            })
                            .ok();
                    }
                }
            }
        }

        self.notifications.notification_view.visible.clone().fold(
            self.notifications
                .notification_view
                .prev
                .as_ref()
                .map(|p| p.extents().height)
                .unwrap_or(0.),
            |acc, i| {
                if let Some(notification) = self.notifications.notifications.get_mut(i) {
                    notification.set_y(acc);
                    acc + notification.extents().height
                } else {
                    acc
                }
            },
        );

        if self.notifications.height()
            == self
                .surface
                .as_ref()
                .map(|s| s.wgpu_surface.config.height)
                .unwrap_or(0) as f32
        {
            if let Some(surface) = self.surface.as_mut() {
                _ = surface.render(
                    &self.wgpu_state.device,
                    &self.wgpu_state.queue,
                    &self.notifications,
                );
            }
            return;
        }

        self.update_surface_size();
    }

    pub fn update_surface_size(&mut self) {
        let total_height = self.notifications.height();
        let total_width = self.notifications.width();

        if self.surface.is_none() {
            let wl_surface = self.compositor.create_surface(&self.qh, ());
            self.surface = Surface::new(
                &self.wgpu_state,
                wl_surface,
                &self.layer_shell,
                &self.qh,
                &self.globals,
                &self.outputs,
                Arc::clone(&self.config),
            )
            .ok();
        }

        if total_width == 0. || total_height == 0. {
            if let Some(surface) = self.surface.take() {
                drop(surface);
            }
            self.seat.keyboard.key_combination.key = Key::Character('\0');
            return;
        }

        if let Some(surface) = self.surface.as_ref() {
            surface
                .layer_surface
                .set_size(total_width as u32, total_height as u32);
            surface.wl_surface.commit();
        }
    }

    pub fn deselect_notification(&mut self) {
        self.notifications.deselect();
        if let Some(surface) = self.surface.as_mut() {
            _ = surface.render(
                &self.wgpu_state.device,
                &self.wgpu_state.queue,
                &self.notifications,
            );
        }
    }

    pub fn select_notification(&mut self, id: NotificationId) {
        self.notifications.select(id);
        if let Some(surface) = self.surface.as_mut() {
            _ = surface.render(
                &self.wgpu_state.device,
                &self.wgpu_state.queue,
                &self.notifications,
            );
        }
    }
}
