pub mod icons;
mod progress;

use super::config::Config;
use crate::{
    buffers,
    button::{Button, ButtonManager, ButtonType},
    config::{Size, StyleState},
    dbus::xdg::NotificationHints,
    text, NotificationData, Urgency,
};
use calloop::RegistrationToken;
use glyphon::{FontSystem, TextArea, TextBounds};
use icons::Icons;
use progress::Progress;
use std::sync::Arc;

#[derive(Debug, Default)]
pub struct Extents {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub type NotificationId = u32;

pub struct Notification {
    id: NotificationId,
    pub y: f32,
    pub x: f32,
    app_name: Box<str>,
    pub text: text::Text,
    timeout: Option<u64>,
    hovered: bool,
    config: Arc<Config>,
    pub icons: Icons,
    progress: Option<Progress>,
    pub registration_token: Option<RegistrationToken>,
    pub buttons: ButtonManager,
    pub hints: NotificationHints,
}

impl PartialEq for Notification {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl Notification {
    pub fn new(config: Arc<Config>, font_system: &mut FontSystem, data: NotificationData) -> Self {
        if data.app_name == "next_notification_count".into()
            || data.app_name == "prev_notification_count".into()
        {
            return Self {
                hints: NotificationHints::default(),
                id: 0,
                y: 0.,
                x: 0.,
                app_name: data.app_name,
                text: text::Text::new(&config.styles.default.font, font_system, ""),
                timeout: None,
                hovered: false,
                config: Arc::clone(&config),
                icons: Icons {
                    icon: None,
                    app_icon: None,
                    x: 0.,
                    y: 0.,
                },
                progress: None,
                registration_token: None,
                buttons: ButtonManager::default(),
            };
        }

        let icons = Icons::new(data.hints.image.as_ref(), data.app_icon, &config);

        let style = &config.styles.default;
        let mut buttons = ButtonManager::default();
        let dismiss_button = Button::new(ButtonType::Dismiss, Arc::clone(&config), font_system);

        data.actions.iter().cloned().for_each(|(action, text)| {
            buttons.add(Button::new(
                ButtonType::Action { text, action },
                Arc::clone(&config),
                font_system,
            ))
        });

        let icon_width = icons
            .icon
            .as_ref()
            .map(|i| i.width as f32 + style.padding.right)
            .unwrap_or(0.);
        let text = text::Text::new_notification(
            &config.styles.default.font,
            font_system,
            &data.summary,
            &data.body,
            config.styles.default.width.resolve(0.)
                - icon_width
                - dismiss_button.rendered_extents(false).width,
        );

        buttons.add(dismiss_button);

        let notification_style_entry = config
            .styles
            .notification
            .iter()
            .find(|entry| entry.app == data.app_name);

        let ignore_timeout = notification_style_entry
            .and_then(|entry| entry.ignore_timeout)
            .unwrap_or(config.ignore_timeout);

        let default_timeout = notification_style_entry
            .and_then(|entry| entry.default_timeout.as_ref())
            .unwrap_or(&config.default_timeout);

        let timeout = if ignore_timeout {
            (default_timeout.get(&data.hints.urgency) > 0)
                .then(|| (default_timeout.get(&data.hints.urgency) as u64) * 1000)
        } else {
            match data.timeout {
                0 => None,
                -1 => (default_timeout.get(&data.hints.urgency) > 0)
                    .then(|| (default_timeout.get(&data.hints.urgency) as u64) * 1000),
                t if t > 0 => Some(t as u64),
                _ => None,
            }
        };

        Self {
            progress: data.hints.value.map(Progress::new),
            hints: data.hints,
            y: 0.,
            x: 0.,
            icons,
            buttons,
            id: data.id,
            app_name: data.app_name,
            text,
            timeout,
            config,
            hovered: false,
            registration_token: None,
        }
    }

    pub fn set_position(&mut self, x: f32, y: f32) {
        self.y = y;
        self.x = x;
        self.update_text_position();
        let extents = self.rendered_extents();
        let style = self.config.find_style(&self.app_name, self.hovered());

        self.icons.set_position(
            &extents,
            style,
            &self.progress,
            &self.buttons,
            self.hovered(),
        );

        if let Some(progress) = self.progress.as_mut() {
            progress.set_position(&extents, style);
        }

        let extents = self.rendered_extents();
        let hovered = self.hovered();

        if let Some(button) = self
            .buttons
            .buttons_mut()
            .iter_mut()
            .find(|button| button.button_type == ButtonType::Dismiss)
        {
            let (x, y) = (
                extents.x + extents.width
                    - style.border.size.right
                    - style.padding.right
                    - button.extents(hovered).width,
                extents.y + style.margin.top + style.border.size.top + style.padding.top,
            );

            button.set_position(x, y)
        }

        let height = self
            .buttons
            .buttons_mut()
            .iter_mut()
            .find(|button| button.button_type == ButtonType::Dismiss)
            .map(|b| {
                let extents = b.extents(hovered);
                extents.y + extents.height
            })
            .unwrap_or(0.);

        let actions_count = self
            .buttons
            .buttons()
            .iter()
            .filter(|button| matches!(button.button_type, ButtonType::Action { .. }))
            .count() as f32;

        self.buttons
            .buttons_mut()
            .iter_mut()
            .filter(|b| matches!(b.button_type, ButtonType::Action { .. }))
            .enumerate()
            .for_each(|(i, button)| {
                let button_style = button.style(hovered);

                let available_width = extents.width
                    - style.border.size.left
                    - style.border.size.right
                    - style.padding.left
                    - style.padding.right
                    - button_style.margin.left
                    - button_style.margin.right;

                let spacing_between = button_style.margin.left + button_style.margin.right;

                let total_spacing = (actions_count - 1.) * spacing_between;

                let button_width = (available_width - total_spacing) / actions_count;
                button.width = button_width - button_style.margin.left - button_style.margin.right;

                let (x, y) = if let ButtonType::Action { .. } = button.button_type {
                    let base_x = extents.x + style.border.size.left + style.padding.left;
                    let x_position = base_x + (button_width + spacing_between) * i as f32;

                    let y_position = (extents.y + extents.height
                        - style.border.size.bottom
                        - style.padding.bottom
                        - self
                            .progress
                            .map(|p| p.extents(&extents, style).height)
                            .unwrap_or_default()
                        - button.extents(hovered).height)
                        .max(height);

                    (x_position, y_position)
                } else {
                    return;
                };

                button.set_position(x, y);
            });
    }

    pub fn timeout(&self) -> Option<u64> {
        self.timeout
    }

    fn text_extents(&self) -> Extents {
        let style = self.style();
        let icon_extents = self.icons.extents(style);

        let dismiss_button = self
            .buttons
            .buttons()
            .iter()
            .find(|button| button.button_type == ButtonType::Dismiss);

        Extents {
            x: style.padding.left + style.border.size.left + style.margin.left + icon_extents.width,
            y: style.margin.top + style.border.size.top,
            width: style.width.resolve(0.)
                - icon_extents.width
                - dismiss_button
                    .map(|b| b.extents(self.hovered()).width)
                    .unwrap_or_default(),
            height: 0.,
        }
    }

    pub fn set_text(&mut self, summary: &str, body: &str, font_system: &mut FontSystem) {
        let style = self.style();
        let text_extents = self.text_extents();

        self.text = text::Text::new_notification(
            &style.font,
            font_system,
            summary,
            body,
            text_extents.width,
        );
    }

    pub fn height(&self) -> f32 {
        let style = self.style();

        let dismiss_button = self
            .buttons
            .buttons()
            .iter()
            .find(|button| button.button_type == ButtonType::Dismiss)
            .map(|b| b.extents(self.hovered()).height)
            .unwrap_or(0.0);

        let action_button = self
            .buttons
            .buttons()
            .iter()
            .filter_map(|button| match button.button_type {
                ButtonType::Action { .. } => Some(button.extents(self.hovered())),
                _ => None,
            })
            .max_by(|a, b| {
                a.height
                    .partial_cmp(&b.height)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or_default();

        let progress = if self.progress.is_some() {
            style.progress.height.resolve(0.)
                + style.progress.margin.top
                + style.progress.margin.bottom
        } else {
            0.0
        };

        let min_height = match style.min_height {
            Size::Auto => 0.0,
            Size::Value(value) => value,
        };

        let max_height = match style.max_height {
            Size::Auto => f32::INFINITY,
            Size::Value(value) => value,
        };

        match style.height {
            Size::Value(height) => height.clamp(min_height, max_height),
            Size::Auto => {
                let text_height = self.text.extents().1 + progress;
                let icon_height = self.icons.extents(style).height + progress;
                let base_height = (text_height.max(icon_height).max(dismiss_button)
                    + action_button.height)
                    .max(dismiss_button + action_button.height)
                    + style.padding.bottom;
                base_height.clamp(min_height, max_height)
            }
        }
    }

    pub fn width(&self) -> f32 {
        match self.hovered() {
            true => self.config.styles.hover.width.resolve(0.),
            false => self.config.styles.default.width.resolve(0.),
        }
    }

    pub fn urgency(&self) -> &Urgency {
        &self.hints.urgency
    }

    pub fn hovered(&self) -> bool {
        self.hovered
    }

    fn update_text_position(&mut self) {
        let style = self.style();
        let icon_width = self.icons.extents(style).width;
        self.text.set_buffer_position(
            style.padding.left + style.border.size.left + style.margin.left + icon_width,
            style.margin.top + style.border.size.top,
        );
    }

    pub fn hover(&mut self) {
        self.hovered = true;
        self.set_position(self.x, self.y);
    }

    pub fn unhover(&mut self) {
        self.hovered = false;
        self.update_text_position();
    }

    pub fn id(&self) -> NotificationId {
        self.id
    }

    fn background_instance(&self, scale: f32) -> buffers::Instance {
        let extents = self.rendered_extents();
        let style = self.style();

        buffers::Instance {
            rect_pos: [extents.x, extents.y],
            rect_size: [
                extents.width - style.border.size.left - style.border.size.right,
                extents.height - style.border.size.top - style.border.size.bottom,
            ],
            rect_color: style.background.to_linear(self.urgency()),
            border_radius: style.border.radius.into(),
            border_size: style.border.size.into(),
            border_color: style.border.color.to_linear(self.urgency()),
            scale,
        }
    }

    pub fn instances(&self, scale: f32) -> Vec<buffers::Instance> {
        let mut instances = vec![self.background_instance(scale)];
        if let Some(progress) = self.progress.as_ref() {
            instances.extend_from_slice(&progress.instances(
                self.urgency(),
                &self.rendered_extents(),
                self.style(),
                scale,
            ));
        }

        let button_instances = self
            .buttons
            .instances(self.hovered(), self.urgency(), scale);

        instances.extend_from_slice(&button_instances);

        instances
    }

    pub fn extents(&self) -> Extents {
        let style = self.style();

        Extents {
            x: 0.,
            y: self.y,
            width: self.width()
                + style.border.size.left
                + style.border.size.right
                + style.padding.left
                + style.padding.right
                + style.margin.left
                + style.margin.right,
            height: self.height()
                + style.border.size.top
                + style.border.size.bottom
                + style.padding.top
                + style.padding.bottom
                + style.margin.top
                + style.margin.bottom,
        }
    }

    pub fn style(&self) -> &StyleState {
        let styles = self
            .config
            .styles
            .notification
            .iter()
            .find(|n| n.app == self.app_name)
            .map(|c| &c.styles)
            .unwrap_or(&self.config.styles);

        if self.hovered {
            &styles.hover
        } else {
            &styles.default
        }
    }

    pub fn rendered_extents(&self) -> Extents {
        let extents = self.extents();
        let style = self.style();

        Extents {
            x: extents.x + style.margin.left + self.x + self.hints.x as f32,
            y: extents.y + style.margin.top,
            width: extents.width - style.margin.left - style.margin.right,
            height: extents.height - style.margin.top - style.margin.bottom,
        }
    }

    pub fn text_area(&self, scale: f32) -> Vec<TextArea> {
        let extents = self.rendered_extents();
        let (width, height) = self.text.extents();

        let style = self.style();

        let icon_width_positioning = self
            .icons
            .icon
            .as_ref()
            .map(|i| i.width as f32 + style.padding.left)
            .unwrap_or(0.);

        let mut res = vec![TextArea {
            buffer: &self.text.buffer,
            left: extents.x + style.border.size.left + style.padding.left + icon_width_positioning,
            top: extents.y + style.border.size.top + style.padding.top,
            scale,
            bounds: TextBounds {
                left: (extents.x
                    + style.border.size.left
                    + style.padding.left
                    + icon_width_positioning) as i32,
                top: (extents.y + style.border.size.top + style.padding.top) as i32,
                right: (extents.x
                    + style.border.size.left
                    + width
                    + style.padding.left
                    + icon_width_positioning) as i32,
                bottom: (extents.y
                    + style.border.size.top
                    + height.min(self.height())
                    + style.padding.top) as i32,
            },
            default_color: style.font.color.into_glyphon(self.urgency()),
            custom_glyphs: &[],
        }];

        let button_areas = self
            .buttons
            .text_areas(self.hovered(), self.urgency(), scale);

        res.extend_from_slice(&button_areas);
        res
    }
}
