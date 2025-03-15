use crate::{
    buffers,
    config::{
        button::{self, ButtonState},
        Config,
    },
    notification_manager::notification::Extents,
    text::Text,
};
use glyphon::FontSystem;
use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

#[derive(PartialEq)]
pub enum ButtonType {
    Dismiss,
    Action,
}

#[derive(PartialEq, Copy, Clone)]
pub enum Action {
    DismissNotification,
}

#[derive(Default)]
pub struct ButtonManager {
    buttons: Vec<Button>,
}

impl Deref for ButtonManager {
    type Target = Vec<Button>;

    fn deref(&self) -> &Self::Target {
        &self.buttons
    }
}

impl DerefMut for ButtonManager {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buttons
    }
}

impl ButtonManager {
    pub fn get_by_coordinates(&mut self, x: f64, y: f64) -> Option<Action> {
        let index = self.buttons.iter_mut().position(|button| {
            let extents = button.extents();
            button.unhover();
            x >= extents.x as f64
                && y >= extents.y as f64
                && x <= (extents.x as f64 + extents.width as f64)
                && y <= (extents.y as f64 + extents.height as f64)
        })?;

        if let Some(button) = self.buttons.get_mut(index) {
            button.hover();
            Some(button.action)
        } else {
            None
        }
    }
}

pub struct Button {
    hovered: bool,
    x: f32,
    y: f32,
    config: Arc<Config>,
    text: Text,
    pub action: Action,
    pub button_type: ButtonType,
}

impl Button {
    pub fn new(
        x: f32,
        y: f32,
        action: Action,
        button_type: ButtonType,
        config: Arc<Config>,
        font_system: &mut FontSystem,
    ) -> Self {
        let font = match button_type {
            ButtonType::Dismiss => &config.styles.default.buttons.dismiss.font,
            ButtonType::Action => &config.styles.default.buttons.action.font,
        };

        let text = Text::new(font, font_system, "X", x, y);

        Self {
            text,
            hovered: false,
            x,
            y,
            config,
            action,
            button_type,
        }
    }

    pub fn set_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
    }

    pub fn hover(&mut self) {
        self.hovered = true;
    }

    pub fn unhover(&mut self) {
        self.hovered = false;
    }

    pub fn extents(&self) -> Extents {
        let button = match self.button_type {
            ButtonType::Action => &self.config.styles.default.buttons.action,
            ButtonType::Dismiss => &self.config.styles.default.buttons.dismiss,
        };

        Extents {
            x: self.x,
            y: self.y,
            width: button.width,
            height: button.height,
        }
    }

    pub fn style(&self, hovered: bool) -> (&button::Button, &ButtonState) {
        let button = match (&self.button_type, hovered) {
            (ButtonType::Dismiss, true) => &self.config.styles.hover.buttons.dismiss,
            (ButtonType::Dismiss, false) => &self.config.styles.default.buttons.dismiss,
            (ButtonType::Action, true) => &self.config.styles.hover.buttons.action,
            (ButtonType::Action, false) => &self.config.styles.default.buttons.action,
        };

        (
            button,
            if self.hovered {
                &button.hover
            } else {
                &button.default
            },
        )
    }

    pub fn text_area(&self, scale: f32) -> glyphon::TextArea {
        let extents = self.extents();

        glyphon::TextArea {
            buffer: &self.text.buffer,
            left: extents.x,
            top: extents.y,
            scale,
            bounds: glyphon::TextBounds {
                left: extents.x as i32,
                top: extents.y as i32,
                right: (extents.x + extents.width) as i32,
                bottom: (extents.y + extents.height) as i32,
            },
            custom_glyphs: &[],
            default_color: glyphon::Color::rgba(0, 0, 0, 255),
        }
    }

    pub fn get_instance(&self, hovered: bool, scale: f32) -> buffers::Instance {
        let (button, style) = self.style(hovered);
        let extents = self.extents();

        buffers::Instance {
            rect_pos: [extents.x, extents.y],
            rect_size: [extents.width, extents.height],
            rect_color: style.background_color.into(),
            border_radius: button.border.radius.into(),
            border_size: button.border.size.into(),
            border_color: style.border_color.into(),
            scale,
        }
    }
}
