mod view;

use crate::{
    components::{
        button::ButtonType,
        notification::{Notification, NotificationId},
        text::Text,
        Component, Data,
    },
    config::{keymaps, Config, Queue},
    rendering::texture_renderer::TextureArea,
    utils::buffers,
    EmitEvent, History, Moxnotify, NotificationData,
};
use calloop::{
    timer::{TimeoutAction, Timer},
    LoopHandle,
};
use glyphon::{FontSystem, TextArea};
use rusqlite::params;
use std::{cell::RefCell, fmt, rc::Rc, time::Duration};
use view::NotificationView;

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
    config: Rc<Config>,
    loop_handle: LoopHandle<'static, Moxnotify>,
    pub font_system: Rc<RefCell<FontSystem>>,
    pub notification_view: NotificationView,
    inhibited: bool,
    pub ui_state: Rc<RefCell<UiState>>,
}

impl NotificationManager {
    pub fn new(
        config: Rc<Config>,
        loop_handle: LoopHandle<'static, Moxnotify>,
        font_system: Rc<RefCell<FontSystem>>,
    ) -> Self {
        let ui_state = Rc::new(RefCell::new(UiState::default()));

        Self {
            inhibited: false,
            waiting: 0,
            notification_view: NotificationView::new(
                Rc::clone(&config),
                Rc::clone(&ui_state),
                Rc::clone(&font_system),
            ),
            font_system,
            loop_handle,
            notifications: Vec::new(),
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

    pub fn data(&self) -> (Vec<buffers::Instance>, Vec<TextArea>, Vec<TextureArea>) {
        let mut instances = Vec::new();
        let mut text_areas = Vec::new();
        let mut textures = Vec::new();

        let all_data: Vec<Data> = self
            .notifications
            .iter()
            .enumerate()
            .filter(|(i, _)| self.notification_view.visible.contains(i))
            .flat_map(|(_, notification)| notification.get_data(notification.urgency()))
            .collect();

        for data_item in all_data {
            match data_item {
                Data::Instance(instance) => instances.push(instance),
                Data::TextArea(text_area) => text_areas.push(text_area),
                Data::Texture(texture) => textures.push(texture),
            }
        }

        let total_width = self
            .notifications
            .iter()
            .map(|notification| {
                notification.get_render_bounds().x + notification.get_render_bounds().width
            })
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or_default();

        if let Some((instance, text_area)) = self.notification_view.prev_data(total_width) {
            instances.push(instance);
            text_areas.push(text_area);
        }

        if let Some((instance, text_area)) = self.notification_view.next_data(total_width) {
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
                    let extents = notification.get_render_bounds();
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
            .map_or(0., |n| n.get_bounds().height);
        self.notification_view
            .visible
            .clone()
            .fold(height, |acc, i| {
                if let Some(notification) = self.notifications.get(i) {
                    let extents = notification.get_bounds();
                    return acc + extents.height;
                };

                acc
            })
            + self
                .notification_view
                .next
                .as_ref()
                .map_or(0., |n| n.get_bounds().height)
    }

    pub fn width(&self) -> f32 {
        let (min_x, max_x) =
            self.notifications
                .iter()
                .fold((f32::MAX, f32::MIN), |(min_x, max_x), notification| {
                    let extents = notification.get_bounds();
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
        self.deselect();

        if let Some(notification) = self.notifications.iter_mut().find(|n| n.id() == id) {
            notification.hover();

            self.ui_state.borrow_mut().selected = Some(id);
            if let Some(token) = notification.registration_token.take() {
                self.loop_handle.remove(token);
            }

            let dismiss_button = notification
                .buttons
                .buttons()
                .iter()
                .find(|button| button.button_type() == ButtonType::Dismiss)
                .map(|button| button.get_render_bounds().width)
                .unwrap_or(0.0);

            notification.body.set_size(
                &mut self.font_system.borrow_mut(),
                Some(
                    notification.get_style().width
                        - notification.icons.get_bounds().width
                        - dismiss_button,
                ),
                None,
            );

            notification.summary.set_size(
                &mut self.font_system.borrow_mut(),
                Some(
                    notification.get_style().width
                        - notification.icons.get_bounds().width
                        - dismiss_button,
                ),
                None,
            );
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
                .map(|p| p.get_bounds().height)
                .unwrap_or(0.),
            |acc, i| {
                if let Some(notification) = self.notifications.get_mut(i) {
                    notification.set_position(notification.x, acc);
                    acc + notification.get_bounds().height
                } else {
                    acc
                }
            },
        );

        self.notification_view
            .update_notification_count(self.height(), self.notifications.len());
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
                .map(|p| p.get_bounds().height)
                .unwrap_or(0.),
            |acc, i| {
                if let Some(notification) = self.notifications.get_mut(i) {
                    notification.set_position(notification.x, acc);
                    acc + notification.get_bounds().height
                } else {
                    acc
                }
            },
        );

        self.notification_view
            .update_notification_count(self.height(), self.notifications.len());
    }

    pub fn deselect(&mut self) {
        let id = self.ui_state.borrow_mut().selected.take();
        if let Some(old_id) = id {
            if let Some(index) = self.notifications.iter().position(|n| n.id() == old_id) {
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
                                moxnotify.dismiss_by_id(old_id, Some(Reason::Expired));
                                TimeoutAction::Drop
                            })
                            .ok();
                    }
                }
            }
        }
    }

    pub fn waiting(&self) -> u32 {
        self.waiting
    }

    pub fn add_many(&mut self, data: Vec<NotificationData>) -> anyhow::Result<()> {
        let mut y = 0.0;

        data.into_iter().for_each(|data| {
            let mut notification = Notification::new(
                Rc::clone(&self.config),
                &mut self.font_system.borrow_mut(),
                data,
                Rc::clone(&self.ui_state),
                None,
            );
            notification.set_position(0.0, y);
            let height = notification.get_bounds().height;
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
                let y = self.notifications[index].get_bounds().y;
                (y, Some(index))
            } else {
                (self.height(), None)
            };

        let mut notification = Notification::new(
            Rc::clone(&self.config),
            &mut self.font_system.borrow_mut(),
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
                if let Some(token) = self.notifications[index].registration_token.take() {
                    self.loop_handle.remove(token);
                }

                let replaced_height_differs = self.notifications[index].get_bounds().height
                    != notification.get_bounds().height;

                self.notifications[index] = notification;

                if replaced_height_differs {
                    self.notification_view.visible.clone().fold(
                        self.notification_view
                            .prev
                            .as_ref()
                            .map(|p| p.get_bounds().height)
                            .unwrap_or(0.),
                        |acc, i| {
                            if let Some(notification) = self.notifications.get_mut(i) {
                                notification.set_position(notification.x, acc);
                                acc + notification.get_bounds().height
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
                ..self.notifications.len().saturating_sub(1) + self.config.general.max_visible;
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
                .map(|p| p.get_bounds().height)
                .unwrap_or(0.),
            |acc, i| {
                if let Some(notification) = self.notifications.get_mut(i) {
                    notification.set_position(notification.x, acc);
                    acc + notification.get_bounds().height
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
        write!(f, "{s}")
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
                log::error!("Render error: {e}");
            }
        }

        if self.notifications.notifications().is_empty() {
            self.seat.keyboard.repeat.key = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use calloop::EventLoop;
    use glyphon::FontSystem;

    use super::NotificationManager;
    use crate::{config::Config, dbus::xdg::NotificationData};

    #[test]
    fn test_add() {
        let config = Rc::new(Config::default());
        let event_loop = EventLoop::try_new().unwrap();
        let font_system = Rc::new(RefCell::new(FontSystem::new()));
        let mut manager =
            NotificationManager::new(Rc::clone(&config), event_loop.handle(), font_system);

        let data = NotificationData::default();
        manager.add(data).unwrap();

        assert_eq!(manager.notifications().len(), 1);
    }

    #[test]
    fn test_add_with_duplicate_id() {
        let config = Rc::new(Config::default());
        let event_loop = EventLoop::try_new().unwrap();
        let font_system = Rc::new(RefCell::new(FontSystem::new()));
        let mut manager =
            NotificationManager::new(Rc::clone(&config), event_loop.handle(), font_system);

        let data = NotificationData {
            id: 42,
            ..Default::default()
        };

        manager.add(data.clone()).unwrap();

        manager.add(data).unwrap();

        assert_eq!(manager.notifications().len(), 1);
    }

    #[test]
    fn test_add_many() {
        let config = Rc::new(Config::default());
        let event_loop = EventLoop::try_new().unwrap();
        let font_system = Rc::new(RefCell::new(FontSystem::new()));
        let mut manager =
            NotificationManager::new(Rc::clone(&config), event_loop.handle(), font_system);

        let mut notifications = Vec::new();
        for i in 1..=5 {
            let data = NotificationData {
                id: i,
                ..Default::default()
            };
            notifications.push(data);
        }

        manager.add_many(notifications).unwrap();
        assert_eq!(manager.notifications().len(), 5);
    }

    #[test]
    fn test_dismiss() {
        let config = Rc::new(Config::default());
        let event_loop = EventLoop::try_new().unwrap();
        let font_system = Rc::new(RefCell::new(FontSystem::new()));
        let mut manager =
            NotificationManager::new(Rc::clone(&config), event_loop.handle(), font_system);

        let data = NotificationData {
            id: 123,
            ..Default::default()
        };
        manager.add(data).unwrap();

        assert_eq!(manager.notifications().len(), 1);

        manager.dismiss(123);
        assert_eq!(manager.notifications().len(), 0);
    }

    #[test]
    fn test_select_and_deselect() {
        let config = Rc::new(Config::default());
        let event_loop = EventLoop::try_new().unwrap();
        let font_system = Rc::new(RefCell::new(FontSystem::new()));
        let mut manager =
            NotificationManager::new(Rc::clone(&config), event_loop.handle(), font_system);

        let data = NotificationData {
            id: 1,
            ..Default::default()
        };
        manager.add(data).unwrap();

        assert_eq!(manager.selected_id(), None);

        manager.select(1);
        assert_eq!(manager.selected_id(), Some(1));

        let notification = manager.selected_notification_mut().unwrap();
        assert!(notification.hovered());

        manager.deselect();
        assert_eq!(manager.selected_id(), None);
    }

    #[test]
    fn test_next_and_prev() {
        let config = Rc::new(Config::default());
        let event_loop = EventLoop::try_new().unwrap();
        let font_system = Rc::new(RefCell::new(FontSystem::new()));
        let mut manager =
            NotificationManager::new(Rc::clone(&config), event_loop.handle(), font_system);

        for i in 1..=3 {
            let data = NotificationData {
                id: i,
                ..Default::default()
            };
            manager.add(data).unwrap();
        }

        manager.next();
        assert_eq!(manager.selected_id(), Some(1));

        manager.next();
        assert_eq!(manager.selected_id(), Some(2));

        manager.next();
        assert_eq!(manager.selected_id(), Some(3));

        manager.next();
        assert_eq!(manager.selected_id(), Some(1));

        manager.prev();
        assert_eq!(manager.selected_id(), Some(3));

        manager.prev();
        assert_eq!(manager.selected_id(), Some(2));
    }

    #[test]
    fn test_inhibit() {
        let config = Rc::new(Config::default());
        let event_loop = EventLoop::try_new().unwrap();
        let font_system = Rc::new(RefCell::new(FontSystem::new()));
        let mut manager =
            NotificationManager::new(Rc::clone(&config), event_loop.handle(), font_system);

        let data = NotificationData {
            id: 0,
            ..Default::default()
        };
        manager.add(data).unwrap();

        assert_eq!(manager.notifications().len(), 1);

        manager.inhibit();

        let data = NotificationData {
            id: 1,
            ..Default::default()
        };
        manager.add(data).unwrap();

        assert_eq!(manager.notifications().len(), 1);
        assert_eq!(manager.waiting(), 1);

        manager.uninhibit();

        assert_eq!(manager.notifications().len(), 1);
        assert_eq!(manager.waiting(), 0);
    }

    #[test]
    fn test_data() {
        let config = Rc::new(Config::default());
        let event_loop = EventLoop::try_new().unwrap();
        let font_system = Rc::new(RefCell::new(FontSystem::new()));
        let mut manager =
            NotificationManager::new(Rc::clone(&config), event_loop.handle(), font_system);

        let data = NotificationData {
            id: 123,
            ..Default::default()
        };
        manager.add(data).unwrap();

        let data = manager.data();
        // Body, summary, notification and dismiss button
        assert_eq!(data.0.len(), 4);
        // Body and summary
        assert_eq!(data.1.len(), 2);
        assert_eq!(data.2.len(), 0);
    }
}
