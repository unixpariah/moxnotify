mod notification;
mod notification_view;

use crate::{
    config::{self, Anchor, Config, Key, Queue},
    wgpu_state::{
        buffers,
        texture_renderer::{TextureArea, TextureBounds},
    },
    EmitEvent, Moxnotify, NotificationData, Urgency,
};
use calloop::{
    timer::{TimeoutAction, Timer},
    LoopHandle,
};
use glyphon::{FontSystem, TextArea};
use notification::{Notification, NotificationId};
use notification_view::NotificationView;
use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
    time::Duration,
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1,
    zwlr_layer_surface_v1::{self, KeyboardInteractivity},
};

pub struct NotificationManager {
    notifications: Vec<Notification>,
    config: Arc<Config>,
    loop_handle: LoopHandle<'static, Moxnotify>,
    selected: Option<u32>,
    font_system: FontSystem,
    pub notification_view: NotificationView,
}

impl Deref for NotificationManager {
    type Target = Vec<Notification>;

    fn deref(&self) -> &Self::Target {
        &self.notifications
    }
}

impl DerefMut for NotificationManager {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.notifications
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

    pub fn data(
        &mut self,
        scale: f32,
    ) -> (Vec<buffers::Instance>, Vec<TextArea>, Vec<TextureArea>) {
        let mut height = 0.0;

        let mut instances = Vec::new();
        let mut text_areas = Vec::new();
        let mut textures = Vec::new();

        for (i, notification) in self.notifications.iter().enumerate() {
            if i > notification.config.max_visible as usize {
                continue;
            }

            if self.notification_view.visible.contains(&i) {
                let instance = notification.get_instance(height, scale);
                let text = notification.text_area(height, scale);
                let texture = notification.texture(height, self.height(), scale);

                height += notification.extents().height;

                instances.push(instance);
                text_areas.push(text);
                if let Some(tex) = texture {
                    textures.push(tex);
                }
            }
        }

        if let Some((instance_data, text_area_data)) =
            self.notification_view.prepare_data(height, scale)
        {
            instances.push(instance_data);
            text_areas.push(text_area_data);
        }

        (instances, text_areas, textures)
    }

    pub fn get_by_coordinates(&self, x: f64, y: f64) -> Option<&Notification> {
        let mut current_y = 0.0;
        self.notifications.iter().find(|notification| {
            let extents = notification.rendered_extents();
            if x > extents.x as f64
                && x < (extents.x + extents.width) as f64
                && y > (current_y)
                && y < (current_y + extents.height as f64)
            {
                true
            } else {
                current_y += notification.extents().height as f64;
                false
            }
        })
    }

    pub fn get_by_id(&self, id: NotificationId) -> Option<&Notification> {
        self.notifications
            .iter()
            .find(|notification| notification.id() == id)
    }

    pub fn height(&self) -> f32 {
        self.notification_view.visible.clone().fold(0.0, |acc, i| {
            if let Some(notification) = self.notifications.get(i) {
                let extents = notification.extents();
                return acc + extents.height;
            };

            acc
        }) + self
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

        let width = self.width();
        if let Some(new_notification) = self.notifications.iter_mut().find(|n| n.id() == id) {
            new_notification.hover();

            let icon_width_layout = new_notification
                .icon
                .as_ref()
                .map(|i| i.width as f32)
                .unwrap_or(0.);

            new_notification.text.0.set_size(
                &mut self.font_system,
                Some(width - icon_width_layout),
                None,
            );

            self.selected = Some(id);
            if let Some(token) = new_notification.registration_token.take() {
                self.loop_handle.remove(token);
            }
        }

        self.notifications
            .iter_mut()
            .fold(0.0, |acc, notification| {
                notification.change_spot(acc);
                acc + notification.extents().height
            });

        if let Some(mut notification_count) = self.notification_view.next.take() {
            notification_count.change_spot(self.height());
            self.notification_view.next = Some(notification_count);
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
            self.notification_view
                .next(next_notification_index, self.notifications.len());
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
            self.notification_view
                .prev(notification_index, self.notifications.len());
        }
    }

    pub fn deselect(&mut self) {
        if let Some(old_id) = self.selected.take() {
            self.unhover_notification(old_id);
        }
    }

    pub fn add(&mut self, data: NotificationData) -> anyhow::Result<()> {
        let id = data.id;

        let mut notification = Notification::new(
            Arc::clone(&self.config),
            self.height(),
            &mut self.font_system,
            data,
        );

        match self.config.queue {
            Queue::Ordered => {
                if self.notifications.is_empty() {
                    if let Some(timeout) = notification.timeout {
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
                if let Some(timeout) = notification.timeout {
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
                .update_notification_count(self.notifications.len());
        }

        Ok(())
    }

    fn unhover_notification(&mut self, id: NotificationId) {
        if let Some(index) = self.notifications.iter().position(|n| n.id() == id) {
            if let Some(notification) = self.notifications.get_mut(index) {
                notification.unhover();
                let timer = match self.config.queue {
                    Queue::Ordered if index == 0 => notification.timeout,
                    Queue::Unordered => notification.timeout,
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

            self.notifications
                .iter_mut()
                .fold(0.0, |acc, notification| {
                    notification.change_spot(acc);
                    acc + notification.extents().height
                });

            if let Some(mut notification_count) = self.notification_view.next.take() {
                notification_count.change_spot(self.height());
                self.notification_view.next = Some(notification_count);
            }
        }
    }
}

impl Moxnotify {
    pub fn invoke_action(&mut self, id: NotificationId, serial: u32) {
        self.create_activation_token(serial, id);
        self.dismiss_notification(id);
    }

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
        self.notifications.retain(|n| {
            if n.id == id {
                if let Some(token) = n.registration_token {
                    self.loop_handle.remove(token);
                }
                false
            } else {
                true
            }
        });

        let len = self.notifications.len();
        if let Some(mut notification_count) = self.notifications.notification_view.next.take() {
            notification_count.set_text(
                &format!(
                    "({} more)",
                    len - self.notifications.notification_view.visible.end
                ),
                "",
                &mut self.text_ctx.font_system,
            );

            if len - self.notifications.notification_view.visible.end > 0 {
                self.notifications.notification_view.next = Some(notification_count);
            }
        }

        //if let Some(i) = self
        //    .notifications
        //    .iter()
        //    .position(|n| Some(n.id) == self.notifications.selected())
        //{
        //    let range = self.notifications.visible.clone();
        //    if range.end == i {
        //        let mut start = range.start;
        //        let end = range.end;
        //        start = start.saturating_sub(1);

        //        self.notifications.visible = start..end;
        //    }
        //}

        if self.notifications.selected == Some(id) {
            self.deselect_notification();
        }

        self.notifications
            .iter_mut()
            .fold(0., |height_acc, notification| {
                notification.change_spot(height_acc);
                height_acc + notification.extents().height
            });

        if self.config.queue == Queue::Ordered {
            if let Some(notification) = self.notifications.first_mut() {
                if !notification.hovered() {
                    if let Some(timeout) = notification.timeout {
                        let timer = Timer::from_duration(Duration::from_millis(timeout));
                        let id = notification.id;
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

        self.update_surface_size();
    }

    fn create_layer_surface(&mut self) -> zwlr_layer_surface_v1::ZwlrLayerSurfaceV1 {
        let output = self
            .outputs
            .iter()
            .find(|output| output.name.as_ref() == Some(&self.config.output));

        let layer_surface = self.layer_shell.get_layer_surface(
            &self.surface.wl_surface,
            output.map(|o| &o.wl_output),
            match self.config.layer {
                config::Layer::Top => zwlr_layer_shell_v1::Layer::Top,
                config::Layer::Background => zwlr_layer_shell_v1::Layer::Background,
                config::Layer::Bottom => zwlr_layer_shell_v1::Layer::Bottom,
                config::Layer::Overlay => zwlr_layer_shell_v1::Layer::Overlay,
            },
            "moxnotify".into(),
            &self.qh,
            (),
        );

        self.surface.scale = output.map(|o| o.scale).unwrap_or(1.0);

        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer_surface
            .set_anchor(zwlr_layer_surface_v1::Anchor::Right | zwlr_layer_surface_v1::Anchor::Top);
        layer_surface.set_anchor(match self.config.anchor {
            Anchor::TopRight => {
                zwlr_layer_surface_v1::Anchor::Top | zwlr_layer_surface_v1::Anchor::Right
            }
            Anchor::TopCenter => zwlr_layer_surface_v1::Anchor::Top,
            Anchor::TopLeft => {
                zwlr_layer_surface_v1::Anchor::Top | zwlr_layer_surface_v1::Anchor::Left
            }
            Anchor::BottomRight => {
                zwlr_layer_surface_v1::Anchor::Bottom | zwlr_layer_surface_v1::Anchor::Right
            }
            Anchor::BottomCenter => zwlr_layer_surface_v1::Anchor::Bottom,
            Anchor::BottomLeft => {
                zwlr_layer_surface_v1::Anchor::Bottom | zwlr_layer_surface_v1::Anchor::Left
            }
            Anchor::CenterRight => zwlr_layer_surface_v1::Anchor::Right,
            Anchor::Center => {
                zwlr_layer_surface_v1::Anchor::Top
                    | zwlr_layer_surface_v1::Anchor::Bottom
                    | zwlr_layer_surface_v1::Anchor::Left
                    | zwlr_layer_surface_v1::Anchor::Right
            }
            Anchor::CenterLeft => zwlr_layer_surface_v1::Anchor::Left,
        });
        layer_surface.set_exclusive_zone(-1);
        layer_surface
    }

    pub fn update_surface_size(&mut self) {
        let total_height = self.notifications.height();
        let total_width = self.notifications.width();

        if total_width == 0. || total_height == 0. {
            if let Some(layer_surface) = self.surface.layer_surface.take() {
                layer_surface.destroy();
            }
            self.seat.keyboard.key_combination.key = Key::Character('\0');
            return;
        }

        if self.surface.layer_surface.is_none() {
            self.surface.layer_surface = Some(self.create_layer_surface());
        }

        if let Some(layer_surface) = self.surface.layer_surface.as_ref() {
            layer_surface.set_size(total_width as u32, total_height as u32);
        }
        self.surface.wl_surface.commit();
    }

    pub fn deselect_notification(&mut self) {
        self.notifications.deselect();
        if !self.notifications.is_empty() {
            self.render();
        }
    }

    pub fn select_notification(&mut self, id: NotificationId) {
        self.notifications.select(id);
        if !self.notifications.is_empty() {
            self.render();
        }
    }
}
