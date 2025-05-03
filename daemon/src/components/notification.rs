use super::button::{ButtonManager, ButtonType, Finished};
use super::icons::Icons;
use super::progress::Progress;
use super::text::summary::Summary;
use super::UiState;
use crate::{
    components::{Component, Data},
    config::{Size, StyleState},
    rendering::text_renderer,
    utils::buffers,
    Config, Moxnotify, NotificationData, Urgency,
};
use calloop::{LoopHandle, RegistrationToken};
use glyphon::{FontSystem, TextArea, TextBounds};
use std::{cell::RefCell, rc::Rc, sync::Arc};

#[derive(Debug, Default)]
pub struct Extents {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub type NotificationId = u32;

pub struct Notification {
    pub y: f32,
    pub x: f32,
    pub text: text_renderer::Text,
    hovered: bool,
    config: Rc<Config>,
    pub icons: Icons,
    progress: Option<Progress>,
    pub registration_token: Option<RegistrationToken>,
    pub buttons: ButtonManager<Finished>,
    pub data: NotificationData,
    ui_state: Rc<RefCell<UiState>>,
    pub summary: Summary,
}

impl PartialEq for Notification {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl Notification {
    pub fn new(
        config: Rc<Config>,
        font_system: &mut FontSystem,
        data: NotificationData,
        ui_state: Rc<RefCell<UiState>>,
        loop_handle: Option<LoopHandle<'static, Moxnotify>>,
    ) -> Self {
        if data.app_name == "next_notification_count".into()
            || data.app_name == "prev_notification_count".into()
        {
            return Self {
                y: 0.,
                x: 0.,
                text: text_renderer::Text::new(&config.styles.default.font, font_system, ""),
                hovered: false,
                config: Rc::clone(&config),
                icons: Icons {
                    id: data.id,
                    icon: None,
                    app_icon: None,
                    x: 0.,
                    y: 0.,
                    config: Rc::clone(&config),
                    ui_state: Rc::clone(&ui_state),
                    app_name: Arc::clone(&data.app_name),
                },
                progress: None,
                registration_token: None,
                buttons: ButtonManager::new(
                    data.id,
                    data.hints.urgency,
                    Arc::clone(&data.app_name),
                    Rc::clone(&ui_state),
                    loop_handle,
                    Rc::clone(&config),
                )
                .add_dismiss(font_system)
                .finish(font_system),
                ui_state: Rc::clone(&ui_state),
                summary: Summary::new(
                    data.id,
                    Rc::clone(&config),
                    Arc::clone(&data.app_name),
                    Rc::clone(&ui_state),
                    font_system,
                    &data.summary,
                ),
                data,
            };
        }

        let icons = Icons::new(
            data.id,
            data.hints.image.as_ref(),
            data.app_icon.as_deref(),
            Rc::clone(&config),
            Rc::clone(&ui_state),
            Arc::clone(&data.app_name),
        );

        let buttons = ButtonManager::new(
            data.id,
            data.hints.urgency,
            Arc::clone(&data.app_name),
            Rc::clone(&ui_state),
            loop_handle,
            Rc::clone(&config),
        )
        .add_dismiss(font_system)
        .add_actions(&data.actions, font_system);

        let icon_bounds = icons.get_bounds();
        let text = text_renderer::Text::new_notification(
            &config.styles.default.font,
            font_system,
            data.body.to_string(),
            config.styles.default.width.resolve(0.)
                - icon_bounds.width
                - buttons
                    .buttons()
                    .first()
                    .map(|buttons| buttons.get_render_bounds().width)
                    .unwrap_or_default(),
        );

        Self {
            summary: Summary::new(
                data.id,
                Rc::clone(&config),
                Arc::clone(&data.app_name),
                Rc::clone(&ui_state),
                font_system,
                &data.summary,
            ),
            progress: data.hints.value.map(|value| {
                Progress::new(
                    data.id,
                    value,
                    Rc::clone(&ui_state),
                    Rc::clone(&config),
                    Arc::clone(&data.app_name),
                )
            }),
            y: 0.,
            x: 0.,
            icons,
            buttons: buttons
                .add_anchors(&text.anchors, font_system)
                .finish(font_system),
            data,
            text,
            config,
            hovered: false,
            registration_token: None,
            ui_state: Rc::clone(&ui_state),
        }
    }

    pub fn data(&self) -> Vec<Data> {
        let mut data = self.buttons.data();
        if let Some(progress) = self.progress.as_ref() {
            data.extend(progress.get_data(self.urgency()));
        }

        data
    }

    pub fn timeout(&self) -> Option<u64> {
        let notification_style_entry = self
            .config
            .styles
            .notification
            .iter()
            .find(|entry| entry.app == self.data.app_name);

        let ignore_timeout = notification_style_entry
            .and_then(|entry| entry.ignore_timeout)
            .unwrap_or(self.config.general.ignore_timeout);

        let default_timeout = notification_style_entry
            .and_then(|entry| entry.default_timeout.as_ref())
            .unwrap_or(&self.config.general.default_timeout);

        if ignore_timeout {
            (default_timeout.get(&self.data.hints.urgency) > 0)
                .then(|| (default_timeout.get(&self.data.hints.urgency) as u64) * 1000)
        } else {
            match self.data.timeout {
                0 => None,
                -1 => (default_timeout.get(&self.data.hints.urgency) > 0)
                    .then(|| (default_timeout.get(&self.data.hints.urgency) as u64) * 1000),
                t if t > 0 => Some(t as u64),
                _ => None,
            }
        }
    }

    pub fn set_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;

        let extents = self.rendered_extents();
        let hovered = self.hovered();
        let style = self.config.find_style(&self.data.app_name, hovered);

        {
            // Icons
            let available_height = extents.height
                - style.border.size.top
                - style.border.size.bottom
                - style.padding.top
                - style.padding.bottom
                - self
                    .progress
                    .as_ref()
                    .map(|p| p.get_bounds().height)
                    .unwrap_or_default()
                - self
                    .buttons
                    .buttons()
                    .iter()
                    .filter_map(|button| {
                        if button.button_type() == ButtonType::Action {
                            Some(button.get_bounds().height)
                        } else {
                            None
                        }
                    })
                    .max_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap_or_default();

            let vertical_offset = (available_height - self.config.general.icon_size as f32) / 2.0;

            let x = extents.x + style.border.size.left + style.padding.left;
            let y = extents.y + style.border.size.top + style.padding.top + vertical_offset;

            self.icons.set_position(x, y);
        }

        self.summary.set_position(
            extents.x + style.padding.left + style.border.size.left + self.icons.get_bounds().width,
            extents.y + style.padding.top + style.border.size.top,
        );

        if let Some(progress) = self.progress.as_mut() {
            let available_width = extents.width
                - style.border.size.left
                - style.border.size.right
                - style.padding.left
                - style.padding.right
                - style.progress.margin.left
                - style.progress.margin.right;
            progress.set_width(available_width);

            let style = self.config.find_style(
                &self.data.app_name,
                self.ui_state.borrow().selected == Some(self.data.id),
            );

            let x = extents.x + style.border.size.left + style.padding.left;
            let y = extents.y + extents.height
                - style.border.size.bottom.resolve(0.)
                - style.padding.bottom.resolve(0.)
                - progress.get_bounds().height;

            progress.set_position(x, y);
        }

        let extents = self.rendered_extents();

        let dismiss_bottom_y = self
            .buttons
            .buttons_mut()
            .iter_mut()
            .find(|button| button.button_type() == ButtonType::Dismiss)
            .map(|button| {
                let x = extents.x + extents.width
                    - style.border.size.right
                    - style.padding.right
                    - button.get_bounds().width;
                let y = extents.y + style.margin.top + style.border.size.top + style.padding.top;
                button.set_position(x, y);
                let button_extents = button.get_bounds();
                button_extents.y + button_extents.height
            })
            .unwrap_or(0.0);

        let action_buttons = self
            .buttons
            .buttons()
            .iter()
            .filter(|button| button.button_type() == ButtonType::Action)
            .count();

        if action_buttons > 0 {
            let button_style = self
                .buttons
                .buttons()
                .iter()
                .find(|button| button.button_type() == ButtonType::Action)
                .map(|button| button.get_style())
                .unwrap_or_else(|| &style.buttons.action.default);

            let side_padding = style.border.size.left
                + style.border.size.right
                + style.padding.left
                + style.padding.right;
            let button_margin = button_style.margin.left + button_style.margin.right;
            let available_width = extents.width - side_padding - button_margin;

            let action_buttons_f32 = action_buttons as f32;
            let total_spacing = (action_buttons_f32 - 1.0) * button_margin;
            let button_width = (available_width - total_spacing) / action_buttons_f32;

            let progress_height = self
                .progress
                .as_ref()
                .map(|p| p.get_bounds().height)
                .unwrap_or_default();

            let base_x = extents.x + style.border.size.left + style.padding.left;
            let bottom_padding = style.border.size.bottom + style.padding.bottom + progress_height;

            self.buttons.set_action_widths(button_width);

            self.buttons
                .buttons_mut()
                .iter_mut()
                .filter(|b| b.button_type() == ButtonType::Action)
                .enumerate()
                .for_each(|(i, button)| {
                    let x_position = base_x + (button_width + button_margin) * i as f32;
                    let y_position =
                        (extents.y + extents.height - bottom_padding - button.get_bounds().height)
                            .max(dismiss_bottom_y);

                    button.set_position(x_position, y_position);
                });

            let icons_width = self.icons.get_bounds().width;
            self.buttons
                .buttons_mut()
                .iter_mut()
                .filter(|b| b.button_type() == ButtonType::Anchor)
                .for_each(|button| button.set_position(base_x + icons_width, extents.y));
        }

        self.update_text_position();
    }

    pub fn text_extents(&self) -> Extents {
        let style = self.style();
        let icon_extents = self.icons.get_bounds();

        let dismiss_button = self
            .buttons
            .buttons()
            .iter()
            .find(|button| button.button_type() == ButtonType::Dismiss);

        let extents = self.rendered_extents();

        Extents {
            x: extents.x
                + style.padding.left.resolve(0.)
                + style.border.size.left.resolve(0.)
                + icon_extents.width,
            y: extents.y + style.border.size.top.resolve(0.) + style.padding.top.resolve(0.),
            width: style.width.resolve(0.)
                - icon_extents.width
                - dismiss_button
                    .map(|b| b.get_bounds().width)
                    .unwrap_or_default(),
            height: 0.,
        }
    }

    pub fn height(&self) -> f32 {
        let style = self.style();

        let dismiss_button = self
            .buttons
            .buttons()
            .iter()
            .find(|button| button.button_type() == ButtonType::Dismiss)
            .map(|b| b.get_bounds().height)
            .unwrap_or(0.0);

        let action_button = self
            .buttons
            .buttons()
            .iter()
            .filter_map(|button| match button.button_type() {
                ButtonType::Action => Some(button.get_bounds()),
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
                + style.progress.margin.top.resolve(0.)
                + style.progress.margin.bottom.resolve(0.)
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
                let text_height =
                    self.text.extents().height + self.summary.get_bounds().height + progress;
                let icon_height = self.icons.get_bounds().height + progress;
                let base_height = (text_height.max(icon_height).max(dismiss_button)
                    + action_button.height)
                    .max(dismiss_button + action_button.height)
                    + style.padding.bottom.resolve(0.);
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
        &self.data.hints.urgency
    }

    pub fn hovered(&self) -> bool {
        self.hovered
    }

    fn update_text_position(&mut self) {
        let style = self.style();
        let extents = self.rendered_extents();
        self.text.set_buffer_position(
            extents.x + style.padding.left + style.border.size.left + self.icons.get_bounds().width,
            extents.y
                + style.padding.top
                + style.border.size.top
                + self.summary.get_bounds().height,
        );
    }

    pub fn hover(&mut self) {
        self.hovered = true;
        self.update_text_position();
    }

    pub fn unhover(&mut self) {
        self.hovered = false;
        self.update_text_position();
    }

    pub fn id(&self) -> NotificationId {
        self.data.id
    }

    fn background_instance(&self) -> buffers::Instance {
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
            scale: self.ui_state.borrow().scale,
        }
    }

    pub fn instances(&self) -> Vec<buffers::Instance> {
        let mut instances = vec![self.background_instance()];
        if let Some(progress) = self.progress.as_ref() {
            instances.extend_from_slice(&progress.get_instances(&self.data.hints.urgency));
        }

        let button_instances = self.buttons.instances();
        let summary_instance = self.summary.get_instances(self.urgency());

        instances.extend_from_slice(&button_instances);
        instances.extend_from_slice(&summary_instance);

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
        self.config
            .styles
            .notification
            .iter()
            .find(|n| n.app == self.data.app_name)
            .map(|c| if self.hovered() { &c.hover } else { &c.default })
            .unwrap_or_else(|| {
                if self.hovered() {
                    &self.config.styles.hover
                } else {
                    &self.config.styles.default
                }
            })
    }

    pub fn rendered_extents(&self) -> Extents {
        let extents = self.extents();
        let style = self.style();

        Extents {
            x: extents.x + style.margin.left + self.x + self.data.hints.x as f32,
            y: extents.y + style.margin.top,
            width: extents.width - style.margin.left - style.margin.right,
            height: extents.height - style.margin.top - style.margin.bottom,
        }
    }

    pub fn text_areas(&self) -> Vec<TextArea> {
        let text_extents = self.text.extents();

        let style = self.style();

        let mut res = vec![TextArea {
            buffer: &self.text.buffer,
            left: text_extents.x,
            top: text_extents.y,
            scale: self.ui_state.borrow().scale,
            bounds: TextBounds {
                left: text_extents.x as i32,
                top: (text_extents.y) as i32,
                right: (text_extents.x + text_extents.width) as i32,
                bottom: (text_extents.y + text_extents.height) as i32,
            },
            default_color: style.font.color.into_glyphon(self.urgency()),
            custom_glyphs: &[],
        }];

        let button_areas = self.buttons.text_areas();

        let summary = self.summary.get_text_areas(self.urgency());

        res.extend_from_slice(&summary);
        res.extend_from_slice(&button_areas);
        res
    }
}
