pub mod notification;
mod notification_view;

use crate::{
    buffers,
    button::ButtonType,
    config::{self, keymaps, Config, Queue},
    texture_renderer::TextureArea,
    EmitEvent, History, Moxnotify, NotificationData,
};
use calloop::{
    timer::{TimeoutAction, Timer},
    LoopHandle,
};
use glyphon::{FontSystem, TextArea};
use notification::{Notification, NotificationId};
use notification_view::NotificationView;
use rusqlite::params;
use std::{cell::RefCell, fmt, rc::Rc, sync::Arc, time::Duration};

#[derive(Clone)]
pub struct UiState {
    pub scale: f32,
    pub mode: keymaps::Mode,
    pub selected: Option<NotificationId>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            mode: keymaps::Mode::Normal,
            scale: 1.0,
            selected: None,
        }
    }
}

pub struct NotificationManager {
    notifications: Vec<Notification>,
    waiting: u32,
    config: Arc<Config>,
    loop_handle: LoopHandle<'static, Moxnotify>,
    font_system: FontSystem,
    notification_view: NotificationView,
    inhibited: bool,
    pub ui_state: Rc<RefCell<UiState>>,
}

impl NotificationManager {
    pub fn new(config: Arc<Config>, loop_handle: LoopHandle<'static, Moxnotify>) -> Self {
        let ui_state = Rc::new(RefCell::new(UiState::default()));

        Self {
            inhibited: false,
            waiting: 0,
            font_system: FontSystem::new(),
            loop_handle,
            notifications: Vec::new(),
            notification_view: NotificationView::new(
                config.general.max_visible,
                Arc::clone(&config),
                Rc::clone(&ui_state),
            ),
            config,
            ui_state: Rc::clone(&ui_state),
        }
    }

    pub fn inhibit(&mut self) {
        self.inhibited = true;
    }

    pub fn uninhibit(&mut self) {
        self.waiting = 0;
        self.inhibited = false;
    }

    pub fn inhibited(&mut self) -> bool {
        self.inhibited
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
                    let instance = notification.instances();
                    let text = notification.text_areas();
                    let texture = notification.icons.textures(
                        notification.style(),
                        &self.config,
                        self.height(),
                        scale,
                    );

                    textures.extend_from_slice(&texture);
                    text_areas.extend_from_slice(&text);
                    instances.extend_from_slice(&instance);

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
        self.notification_view
            .visible
            .clone()
            .filter_map(|index| {
                if let Some(notification) = self.notifications.get(index) {
                    let extents = notification.rendered_extents();
                    let x_within_bounds =
                        x >= extents.x as f64 && x < (extents.x + extents.width) as f64;
                    let y_within_bounds =
                        y >= extents.y as f64 && y < (extents.y + extents.height) as f64;

                    if x_within_bounds && y_within_bounds {
                        return Some(notification);
                    }
                }

                None
            })
            .next()
    }

    pub fn click(&mut self, x: f64, y: f64) -> bool {
        self.notification_view.visible.clone().any(|index| {
            self.notifications
                .get_mut(index)
                .map(|notification| notification.buttons.click(x, y))
                .unwrap_or_default()
        })
    }

    pub fn hover(&mut self, x: f64, y: f64) -> bool {
        self.notification_view.visible.clone().any(|index| {
            self.notifications
                .get_mut(index)
                .map(|notification| notification.buttons.hover(x, y))
                .unwrap_or_default()
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
                    let left = extents.x + notification.data.hints.x as f32;
                    let right = extents.x + extents.width + notification.data.hints.x as f32;
                    (min_x.min(left), max_x.max(right))
                });

        if min_x == f32::MAX || max_x == f32::MIN {
            0.0
        } else {
            max_x - min_x
        }
    }

    pub fn selected_id(&self) -> Option<NotificationId> {
        self.ui_state.borrow().selected
    }

    pub fn selected_notification_mut(&mut self) -> Option<&mut Notification> {
        let id = self.selected_id();
        self.notifications
            .iter_mut()
            .find(|notification| Some(notification.id()) == id)
    }

    pub fn select(&mut self, id: NotificationId) {
        let old_id = self.ui_state.borrow_mut().selected.take();
        if let Some(old_id) = old_id {
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
                .find(|button| button.button_type() == ButtonType::Dismiss)
                .map(|button| button.render_bounds().width)
                .unwrap_or(0.0);

            new_notification.text.buffer.set_size(
                &mut self.font_system,
                Some(style.width.resolve(0.) - icon_extents.width - dismiss_button),
                None,
            );

            self.ui_state.borrow_mut().selected = Some(id);
            if let Some(token) = new_notification.registration_token.take() {
                self.loop_handle.remove(token);
            }
        }
    }

    pub fn next(&mut self) {
        let next_notification_index = if let Some(id) = self.ui_state.borrow().selected {
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
        let notification_index = if let Some(id) = self.ui_state.borrow().selected {
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
        let mut ui_state = self.ui_state.borrow_mut();
        if let Some(old_id) = ui_state.selected.take() {
            drop(ui_state);
            self.unhover_notification(old_id);
        }
    }

    pub fn waiting(&self) -> u32 {
        self.waiting
    }

    pub fn add_many(&mut self, data: Vec<NotificationData>) -> anyhow::Result<()> {
        let mut y = 0.0;

        data.into_iter().for_each(|data| {
            let mut notification = Notification::new(
                Arc::clone(&self.config),
                &mut self.font_system,
                data,
                Rc::clone(&self.ui_state),
                Some(self.loop_handle.clone()),
            );
            notification.set_position(0.0, y);
            let height = notification.extents().height;
            y += height;

            self.notifications.push(notification);
        });

        if self.notification_view.visible.end < self.notifications.len() {
            self.notification_view
                .update_notification_count(self.height(), self.notifications.len());
        }

        let x_offset = self
            .notifications
            .iter()
            .map(|n| n.data.hints.x)
            .min()
            .unwrap_or_default()
            .abs() as f32;

        self.notifications
            .iter_mut()
            .for_each(|n| n.set_position(x_offset, n.y));

        Ok(())
    }

    pub fn add(&mut self, data: NotificationData) -> anyhow::Result<()> {
        if self.inhibited {
            self.waiting += 1;
            return Ok(());
        }

        let id = data.id;
        let (y, existing_index) =
            if let Some(index) = self.notifications.iter().position(|n| n.id() == id) {
                let y = self.notifications[index].extents().y;
                (y, Some(index))
            } else {
                (self.height(), None)
            };

        let mut notification = Notification::new(
            Arc::clone(&self.config),
            &mut self.font_system,
            data,
            Rc::clone(&self.ui_state),
            Some(self.loop_handle.clone()),
        );
        notification.set_position(0.0, y);

        if let Some(timeout) = notification.timeout() {
            let should_set_timer = match self.config.general.queue {
                Queue::FIFO => self.notifications.is_empty(),
                Queue::Unordered => true,
            };

            if should_set_timer {
                let timer = Timer::from_duration(Duration::from_millis(timeout));
                notification.registration_token = self
                    .loop_handle
                    .insert_source(timer, move |_, _, moxnotify| {
                        moxnotify.dismiss_by_id(id, Some(Reason::Expired));
                        TimeoutAction::Drop
                    })
                    .ok();
            }
        }

        match existing_index {
            Some(index) => {
                let replaced_height_differs =
                    self.notifications[index].extents().height != notification.extents().height;

                self.notifications[index] = notification;

                if replaced_height_differs {
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
                }
            }
            None => self.notifications.push(notification),
        }

        // Maintain selection if replaced
        if let Some(id) = self.selected_id() {
            self.select(id);
        }

        if self.notification_view.visible.end < self.notifications.len() {
            self.notification_view
                .update_notification_count(self.height(), self.notifications.len());
        }

        let x_offset = self
            .notifications
            .iter()
            .map(|n| n.data.hints.x)
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
                let timer = match self.config.general.queue {
                    Queue::FIFO if index == 0 => notification.timeout(),
                    Queue::Unordered => notification.timeout(),
                    _ => None,
                }
                .map(|t| Timer::from_duration(Duration::from_millis(t)));

                if let Some(timer) = timer {
                    notification.registration_token = self
                        .loop_handle
                        .insert_source(timer, move |_, _, moxnotify| {
                            moxnotify.dismiss_by_id(id, Some(Reason::Expired));
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
                if self.selected_id() == Some(notification.id()) {
                    self.select(next_notification.id());
                }
            }
        }

        if self.notification_view.visible.start >= self.notifications.len() {
            self.notification_view.visible = self.notifications.len().saturating_sub(1)
                ..self.notifications.len().saturating_sub(1)
                    + self.config.general.max_visible as usize;
        }

        self.notification_view
            .update_notification_count(self.height(), self.notifications.len());

        if self.selected_id() == Some(id) {
            self.deselect();
        }

        if self.config.general.queue == Queue::FIFO {
            if let Some(notification) = self.notifications.first_mut() {
                if !notification.hovered() {
                    if let Some(timeout) = notification.timeout() {
                        let timer = Timer::from_duration(Duration::from_millis(timeout));
                        let id = notification.id();
                        notification.registration_token = self
                            .loop_handle
                            .insert_source(timer, move |_, _, moxnotify| {
                                moxnotify.dismiss_by_id(id, Some(Reason::Expired));
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
            .map(|notification| notification.data.hints.x)
            .min()
            .unwrap_or_default()
            .abs();
        self.notifications
            .iter_mut()
            .for_each(|notification| notification.set_position(x_offset as f32, notification.y));
    }
}

#[derive(Clone, Copy)]
pub enum Reason {
    Expired = 1,
    DismissedByUser = 2,
    CloseNotificationCall = 3,
    Unkown = 4,
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Reason::Expired => "Expired",
            Reason::DismissedByUser => "DismissedByUser",
            Reason::CloseNotificationCall => "CloseNotificationCall",
            Reason::Unkown => "Unknown",
        };
        write!(f, "{}", s)
    }
}

impl Moxnotify {
    pub fn dismiss_range<T>(&mut self, range: T, reason: Option<Reason>)
    where
        T: std::slice::SliceIndex<[Notification], Output = [Notification]>,
    {
        let ids: Vec<_> = self.notifications.notifications()[range]
            .iter()
            .map(|notification| notification.id())
            .collect();

        if let Some(reason) = reason {
            ids.iter().for_each(|id| {
                _ = self
                    .emit_sender
                    .send(EmitEvent::NotificationClosed { id: *id, reason });
            });
        }

        if ids.len() == self.notifications.notifications.len() {
            self.notifications.notifications.clear();
            self.notifications
                .notification_view
                .update_notification_count(0., 0);
            return;
        }

        ids.iter().for_each(|id| self.notifications.dismiss(*id));
    }

    pub fn dismiss_by_id(&mut self, id: u32, reason: Option<Reason>) {
        match self.history {
            History::Shown => {
                _ = self
                    .db
                    .execute("DELETE FROM notifications WHERE rowid = ?1", params![id]);
                self.notifications.dismiss(id);
            }
            History::Hidden => {
                if let Some(index) = self
                    .notifications
                    .notifications
                    .iter()
                    .position(|n| n.id() == id)
                {
                    if self.notifications.selected_id() == Some(id) {
                        self.notifications.ui_state.borrow_mut().mode = keymaps::Mode::Normal;
                    }

                    self.notifications.dismiss(id);
                    if let Some(reason) = reason {
                        _ = self
                            .emit_sender
                            .send(EmitEvent::NotificationClosed { id, reason });
                    }
                    if self.notifications.selected_id() == Some(id) {
                        let new_index = if index >= self.notifications.notifications.len() {
                            self.notifications.notifications.len().saturating_sub(1)
                        } else {
                            index
                        };

                        if let Some(notification) = self.notifications.notifications.get(new_index)
                        {
                            self.notifications.select(notification.id());
                        }
                    }
                }
            }
        }

        self.update_surface_size();
        if let Some(surface) = self.surface.as_mut() {
            if let Err(e) = surface.render(
                &self.wgpu_state.device,
                &self.wgpu_state.queue,
                &self.notifications,
            ) {
                log::error!("Render error: {}", e);
            }
        }

        if self.notifications.notifications().is_empty() {
            self.seat.keyboard.repeat.key = None;
        }
    }
}
