use crate::{
    buffers,
    config::{button::ButtonState, Config},
    notification_manager::notification::Extents,
    text::Text,
    Urgency,
};
use glyphon::{FontSystem, TextArea};
use std::sync::Arc;

#[derive(PartialEq)]
pub enum ButtonType {
    Dismiss,
    Action { text: Arc<str>, action: Arc<str> },
}

#[derive(PartialEq, Clone)]
pub enum Action {
    DismissNotification,
    Action,
}

#[derive(Default)]
pub struct ButtonManager {
    buttons: Vec<Button>,
}

impl ButtonManager {
    pub fn buttons(&self) -> &[Button] {
        &self.buttons
    }

    pub fn buttons_mut(&mut self) -> &mut [Button] {
        &mut self.buttons
    }

    pub fn add(&mut self, button: Button) {
        self.buttons.push(button);
    }

    pub fn get_by_coordinates(
        &mut self,
        container_hovered: bool,
        x: f64,
        y: f64,
    ) -> Option<Action> {
        self.buttons.iter_mut().find_map(|button| {
            let extents = button.extents(container_hovered);
            if x >= extents.x as f64
                && y >= extents.y as f64
                && x <= (extents.x + extents.width) as f64
                && y <= (extents.y + extents.height) as f64
            {
                button.hovered = true;
                Some(button.action.clone())
            } else {
                button.hovered = false;
                None
            }
        })
    }

    pub fn instances(
        &self,
        container_extents: &Extents,
        container_hovered: bool,
        urgency: &Urgency,
        scale: f32,
    ) -> Vec<buffers::Instance> {
        self.buttons
            .iter()
            .filter_map(|button| {
                let extents = button.extents(container_hovered);
                if container_extents.x <= extents.x
                    && container_extents.y <= extents.y
                    && container_extents.x + container_extents.width >= extents.x
                    && container_extents.y + container_extents.height >= extents.y
                {
                    Some(button.instance(container_hovered, scale, urgency))
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn text_areas(
        &self,
        container_extents: &Extents,
        container_hovered: bool,
        urgency: &Urgency,
        scale: f32,
    ) -> Vec<TextArea> {
        self.buttons
            .iter()
            .filter_map(|button| {
                let extents = button.extents(container_hovered);
                if container_extents.x <= extents.x
                    && container_extents.y <= extents.y
                    && container_extents.x + container_extents.width >= extents.x
                    && container_extents.y + container_extents.height >= extents.y
                {
                    Some(button.text_area(container_hovered, scale, urgency))
                } else {
                    None
                }
            })
            .collect()
    }
}

pub struct Button {
    pub hovered: bool,
    pub button_type: ButtonType,
    x: f32,
    y: f32,
    pub width: f32,
    config: Arc<Config>,
    text: Text,
    action: Action,
}

impl Button {
    pub fn new(
        action: Action,
        button_type: ButtonType,
        config: Arc<Config>,
        font_system: &mut FontSystem,
    ) -> Self {
        let font = match button_type {
            ButtonType::Dismiss => &config.styles.default.buttons.dismiss.default.font,
            ButtonType::Action { .. } => &config.styles.default.buttons.action.default.font,
        };

        let text = match &button_type {
            ButtonType::Dismiss => Text::new(font, font_system, "X"),
            ButtonType::Action { text, .. } => Text::new(font, font_system, text),
        };

        Self {
            text,
            hovered: false,
            x: 0.,
            y: 0.,
            width: 0.,
            config,
            action,
            button_type,
        }
    }

    pub fn set_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
        self.text.set_buffer_position(x, y);
    }

    pub fn extents(&self, container_hovered: bool) -> Extents {
        let button = match self.button_type {
            ButtonType::Action { .. } => &self.config.styles.default.buttons.action,
            ButtonType::Dismiss => &self.config.styles.default.buttons.dismiss,
        };
        let style = self.style(container_hovered);

        let text_extents = self.text.extents();

        let width = match (&self.button_type, container_hovered) {
            (ButtonType::Dismiss, true) => {
                button.hover.width.resolve(text_extents.0)
                    + style.border.size.left
                    + style.border.size.right
            }
            (ButtonType::Dismiss, false) => {
                button.default.width.resolve(text_extents.0)
                    + style.border.size.left
                    + style.border.size.right
            }
            (ButtonType::Action { .. }, true) => button.hover.width.resolve(self.width),
            (ButtonType::Action { .. }, false) => button.default.width.resolve(self.width),
        };
        let height = match container_hovered {
            true => button.hover.height.resolve(text_extents.1),
            false => button.default.height.resolve(text_extents.1),
        } + style.border.size.top
            + style.border.size.bottom;

        Extents {
            x: self.x,
            y: self.y,
            width,
            height,
        }
    }

    pub fn style(&self, container_hovered: bool) -> &ButtonState {
        let button = match (&self.button_type, container_hovered) {
            (ButtonType::Dismiss, true) => &self.config.styles.hover.buttons.dismiss,
            (ButtonType::Dismiss, false) => &self.config.styles.default.buttons.dismiss,
            (ButtonType::Action { .. }, true) => &self.config.styles.hover.buttons.action,
            (ButtonType::Action { .. }, false) => &self.config.styles.default.buttons.action,
        };

        if self.hovered {
            &button.hover
        } else {
            &button.default
        }
    }

    fn text_area(
        &self,
        container_hovered: bool,
        scale: f32,
        urgency: &Urgency,
    ) -> glyphon::TextArea {
        let extents = self.extents(container_hovered);
        let style = self.style(container_hovered);

        let text_extents = self.text.extents();
        glyphon::TextArea {
            buffer: &self.text.buffer,
            left: extents.x - style.border.size.left - style.border.size.right
                + (extents.width - text_extents.0) / 2.,
            top: extents.y + (extents.height - text_extents.1) / 2.,
            scale,
            bounds: glyphon::TextBounds {
                left: (extents.x + style.border.size.left) as i32,
                top: (extents.y + (extents.height - text_extents.1) / 2.) as i32,
                right: ((extents.x - style.border.size.left - style.border.size.right
                    + (extents.width - text_extents.0) / 2.)
                    + extents.width) as i32,
                bottom: ((extents.y + (extents.height - text_extents.1) / 2.) + extents.height)
                    as i32,
            },
            custom_glyphs: &[],
            default_color: style.font.color.into_glyphon(urgency),
        }
    }

    fn instance(
        &self,
        container_hovered: bool,
        scale: f32,
        urgency: &Urgency,
    ) -> buffers::Instance {
        let style = self.style(container_hovered);
        let extents = self.extents(container_hovered);

        match self.button_type {
            ButtonType::Dismiss => buffers::Instance {
                rect_pos: [extents.x, extents.y],
                rect_size: [
                    extents.width - style.border.size.left - style.border.size.right,
                    extents.height - style.border.size.top - style.border.size.bottom,
                ],
                rect_color: style.background.to_linear(urgency),
                border_radius: style.border.radius.into(),
                border_size: style.border.size.into(),
                border_color: style.border.color.to_linear(urgency),
                scale,
            },
            ButtonType::Action { .. } => buffers::Instance {
                rect_pos: [
                    extents.x + style.border.size.left,
                    extents.y + style.border.size.top,
                ],
                rect_size: [
                    extents.width - style.border.size.left - style.border.size.right,
                    extents.height - style.border.size.top - style.border.size.bottom,
                ],
                rect_color: style.background.to_linear(urgency),
                border_radius: style.border.radius.into(),
                border_size: style.border.size.into(),
                border_color: style.border.color.to_linear(urgency),
                scale,
            },
        }
    }
}
