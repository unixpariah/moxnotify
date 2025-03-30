use crate::{
    buffers,
    config::{button::ButtonState, Config},
    notification_manager::notification::Extents,
    text::Text,
    Urgency,
};
use glyphon::{FontSystem, TextArea};
use std::sync::Arc;

#[derive(PartialEq, Clone)]
pub enum ButtonType {
    Dismiss,
    Action { text: Arc<str>, action: Arc<str> },
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
    ) -> Option<ButtonType> {
        self.buttons.iter_mut().find_map(|button| {
            let extents = button.rendered_extents(container_hovered);
            if x >= extents.x as f64
                && y >= extents.y as f64
                && x <= (extents.x + extents.width) as f64
                && y <= (extents.y + extents.height) as f64
            {
                button.hovered = true;
                Some(button.button_type.clone())
            } else {
                button.hovered = false;
                None
            }
        })
    }

    pub fn get_by_character(&mut self, character: char) -> Option<ButtonType> {
        let characters = self.buttons.first()?.config.hint_characters.clone();
        let pos = characters.chars().position(|ch| ch == character)?;

        let button = self.buttons.get_mut(pos)?;
        Some(button.button_type.clone())
    }

    pub fn instances(
        &self,
        container_hovered: bool,
        urgency: &Urgency,
        scale: f32,
    ) -> Vec<buffers::Instance> {
        self.buttons
            .iter()
            .map(|button| button.instance(container_hovered, scale, urgency))
            .collect()
    }

    pub fn text_areas(
        &self,
        container_hovered: bool,
        urgency: &Urgency,
        scale: f32,
    ) -> Vec<TextArea> {
        self.buttons
            .iter()
            .map(|button| button.text_area(container_hovered, scale, urgency))
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
}

impl Button {
    pub fn new(button_type: ButtonType, config: Arc<Config>, font_system: &mut FontSystem) -> Self {
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
            button_type,
        }
    }

    pub fn set_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
        self.text.set_buffer_position(x, y);
    }

    pub fn extents(&self, container_hovered: bool) -> Extents {
        let style = self.style(container_hovered);

        let text_extents = self.text.extents();

        let width = match &self.button_type {
            ButtonType::Dismiss => style.width.resolve(text_extents.0),
            ButtonType::Action { .. } => style.width.resolve(self.width),
        } + style.border.size.left
            + style.border.size.right
            + style.padding.left
            + style.padding.right
            + style.margin.left
            + style.margin.right;

        let height = style.height.resolve(text_extents.1)
            + style.border.size.top
            + style.border.size.bottom
            + style.padding.top
            + style.padding.bottom
            + style.margin.top
            + style.margin.bottom;

        Extents {
            x: self.x,
            y: self.y,
            width,
            height,
        }
    }

    pub fn rendered_extents(&self, container_hovered: bool) -> Extents {
        let extents = self.extents(container_hovered);
        let style = self.style(container_hovered);

        Extents {
            x: extents.x + style.margin.left,
            y: extents.y + style.margin.top,
            width: extents.width - style.margin.left - style.margin.right,
            height: extents.height - style.margin.top - style.margin.bottom,
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
        let extents = self.rendered_extents(container_hovered);
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
        let extents = self.rendered_extents(container_hovered);

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
