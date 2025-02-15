use crate::{
    config::{self, Anchor, Config, Key, Queue},
    wgpu_state::buffers,
    EmitEvent, Moxsignal, NotificationData,
};
use calloop::{
    timer::{TimeoutAction, Timer},
    LoopHandle,
};
use glyphon::{FontSystem, TextArea};
use notification::{Notification, NotificationId};
use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
    time::Duration,
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1,
    zwlr_layer_surface_v1::{self, KeyboardInteractivity},
};

mod notification;

pub struct NotificationManager {
    notifications: Vec<Notification>,
    config: Arc<Config>,
    loop_handle: LoopHandle<'static, Moxsignal>,
    selected: Option<u32>,
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
    pub fn new(config: Arc<Config>, loop_handle: LoopHandle<'static, Moxsignal>) -> Self {
        Self {
            loop_handle,
            notifications: Vec::new(),
            selected: None,
            config,
        }
    }

    pub fn data(
        &mut self,
        scale: f32,
        font_system: &mut FontSystem,
    ) -> (Vec<buffers::Instance>, Vec<TextArea>) {
        self.notifications
            .iter_mut()
            .enumerate()
            .filter_map(|(i, notification)| {
                if i >= notification.config.max_visible as usize {
                    return None;
                }
                let instance = notification.get_instance(scale);
                let text = notification.text_area(font_system, scale);
                Some((instance, text))
            })
            .unzip()
    }

    pub fn height(&self) -> f32 {
        self.notifications.last().map_or(0.0, |notification| {
            let extents = notification.extents();
            extents.y + extents.height
        })
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
            let index = self.notifications.iter().position(|n| n.id() == old_id);
            if let Some(index) = index {
                if let Some(old_notification) = self.notifications.get_mut(index) {
                    {
                        old_notification.unhover();
                        let timer = match self.config.queue {
                            Queue::Ordered => {
                                if index == 0 {
                                    old_notification.timeout.map(|timeout| {
                                        Timer::from_duration(Duration::from_millis(timeout))
                                    })
                                } else {
                                    None
                                }
                            }
                            Queue::Unordered => old_notification.timeout.map(|timeout| {
                                Timer::from_duration(Duration::from_millis(timeout))
                            }),
                        };

                        if let Some(timer) = timer {
                            old_notification.registration_token = self
                                .loop_handle
                                .insert_source(timer, move |_, _, moxsignal| {
                                    moxsignal.dismiss_notification(old_id);
                                    TimeoutAction::Drop
                                })
                                .ok();
                        }
                    }
                }
            }
        }

        // TODO
        self.selected = Some(id);
        if let Some(new_notification) = self.notifications.iter_mut().find(|n| n.id() == id) {
            new_notification.hover();
            if let Some(token) = new_notification.registration_token.take() {
                self.loop_handle.remove(token);
            }
        }
    }

    pub fn next(&mut self) {
        let next_notification = if let Some(id) = self.selected {
            self.notifications
                .iter()
                .position(|n| n.id() == id)
                .and_then(|index| self.notifications.get(index + 1))
                .or_else(|| self.notifications.first())
        } else {
            self.notifications.first()
        };

        if let Some(notification) = next_notification {
            self.select(notification.id());
        }
    }

    pub fn prev(&mut self) {
        let prev_notification = if let Some(id) = self.selected {
            self.notifications
                .iter()
                .position(|n| n.id() == id)
                .and_then(|index| index.checked_sub(1).and_then(|i| self.notifications.get(i)))
                .or_else(|| self.notifications.last())
        } else {
            self.notifications.last()
        };

        if let Some(notification) = prev_notification {
            self.select(notification.id());
        }
    }

    pub fn deselect(&mut self) {
        if let Some(old_id) = self.selected.take() {
            let index = self.notifications.iter().position(|n| n.id() == old_id);
            if let Some(index) = index {
                if let Some(old_notification) = self.notifications.get_mut(index) {
                    old_notification.unhover();
                    let timer = match self.config.queue {
                        Queue::Ordered => {
                            if index == 0 {
                                old_notification.timeout.map(|timeout| {
                                    Timer::from_duration(Duration::from_millis(timeout))
                                })
                            } else {
                                None
                            }
                        }
                        Queue::Unordered => old_notification
                            .timeout
                            .map(|timeout| Timer::from_duration(Duration::from_millis(timeout))),
                    };

                    if let Some(timer) = timer {
                        old_notification.registration_token = self
                            .loop_handle
                            .insert_source(timer, move |_, _, moxsignal| {
                                moxsignal.dismiss_notification(old_id);
                                TimeoutAction::Drop
                            })
                            .ok();
                    }
                }
            }
        }
    }

    pub fn add(
        &mut self,
        data: NotificationData,
        font_system: &mut FontSystem,
    ) -> anyhow::Result<()> {
        let id = data.id;

        let mut notification =
            Notification::new(Arc::clone(&self.config), self.height(), font_system, data);

        match self.config.queue {
            Queue::Ordered => {
                if self.notifications.is_empty() {
                    if let Some(timeout) = notification.timeout {
                        let timer = Timer::from_duration(Duration::from_millis(timeout));

                        notification.registration_token = self
                            .loop_handle
                            .insert_source(timer, move |_, _, moxsignal| {
                                moxsignal.dismiss_notification(id);
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
                        .insert_source(timer, move |_, _, moxsignal| {
                            moxsignal.dismiss_notification(id);
                            TimeoutAction::Drop
                        })
                        .ok()
                }
            }
        }

        self.notifications.push(notification);

        Ok(())
    }
}

impl Moxsignal {
    pub fn invoke_action(&mut self, id: NotificationId, serial: u32) {
        self.create_activation_token(serial, id);
        self.dismiss_notification(id);
    }

    pub fn dismiss_notification(&mut self, id: NotificationId) {
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
                            .insert_source(timer, move |_, _, moxsignal| {
                                moxsignal.dismiss_notification(id);
                                TimeoutAction::Drop
                            })
                            .ok();
                    }
                }
            }
        }

        if self.notifications.is_empty() {
            self.seat.keyboard.key_combination.key = Key::Character('\0');
            if let Some(layer_surface) = self.surface.layer_surface.take() {
                layer_surface.destroy();
            }
            return;
        }

        self.update_surface_size();
    }

    pub fn update_surface_size(&mut self) {
        let total_height = self.notifications.height();
        let total_width = self.notifications.width();

        match &self.surface.layer_surface {
            Some(layer_surface) => {
                layer_surface.set_size(total_width as u32, total_height as u32);
                self.surface.wl_surface.commit();
            }
            None => {
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
                    "moxsignal".into(),
                    &self.qh,
                    (),
                );

                self.surface.scale = output.map(|o| o.scale).unwrap_or(1.0);

                layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);
                layer_surface.set_anchor(
                    zwlr_layer_surface_v1::Anchor::Right | zwlr_layer_surface_v1::Anchor::Top,
                );
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
                layer_surface.set_size(total_width as u32, total_height as u32);
                self.surface.wl_surface.commit();
                self.surface.layer_surface = Some(layer_surface);
            }
        }
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
