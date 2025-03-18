use crate::{
    buffers,
    config::{button::ButtonState, Config},
    notification_manager::notification::Extents,
    text::Text,
    Urgency,
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
    pub buttons: Vec<Button>,
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
        self.buttons.iter_mut().find_map(|button| {
            let extents = button.extents();
            if x >= extents.x as f64
                && y >= extents.y as f64
                && x <= (extents.x as f64 + extents.width as f64)
                && y <= (extents.y as f64 + extents.height as f64)
            {
                button.hovered = true;
                Some(button.action)
            } else {
                button.hovered = false;
                None
            }
        })
    }
}

pub struct Button {
    pub hovered: bool,
    x: f32,
    y: f32,
    config: Arc<Config>,
    text: Text,
    pub action: Action,
    pub button_type: ButtonType,
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
            ButtonType::Action => &config.styles.default.buttons.action.default.font,
        };

        let text = Text::new(font, font_system, "X", 0., 0.);

        Self {
            text,
            hovered: false,
            x: 0.,
            y: 0.,
            config,
            action,
            button_type,
        }
    }

    pub fn set_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
    }

    pub fn extents(&self) -> Extents {
        let button = match self.button_type {
            ButtonType::Action => &self.config.styles.default.buttons.action,
            ButtonType::Dismiss => &self.config.styles.default.buttons.dismiss,
        };

        let text_extents = self.text.extents();

        Extents {
            x: self.x,
            y: self.y,
            width: button.width.max(text_extents.0),
            height: button.height.max(text_extents.1),
        }
    }

    pub fn style(&self, hovered: bool) -> &ButtonState {
        let button = match (&self.button_type, hovered) {
            (ButtonType::Dismiss, true) => &self.config.styles.hover.buttons.dismiss,
            (ButtonType::Dismiss, false) => &self.config.styles.default.buttons.dismiss,
            (ButtonType::Action, true) => &self.config.styles.hover.buttons.action,
            (ButtonType::Action, false) => &self.config.styles.default.buttons.action,
        };

        if self.hovered {
            &button.hover
        } else {
            &button.default
        }
    }

    pub fn text_area(&self, urgency: &Urgency, hovered: bool, scale: f32) -> glyphon::TextArea {
        let extents = self.extents();
        let style = self.style(hovered);

        let text_extents = self.text.extents();
        glyphon::TextArea {
            buffer: &self.text.buffer,
            left: extents.x + (extents.width - text_extents.0) / 2.,
            top: extents.y + (extents.height - text_extents.1) / 2.,
            scale,
            bounds: glyphon::TextBounds {
                left: (extents.x + (extents.width - text_extents.0) / 2.) as i32,
                top: (extents.y + (extents.height - text_extents.1) / 2.) as i32,
                right: ((extents.x + (extents.width - text_extents.0) / 2.) + extents.width) as i32,
                bottom: ((extents.y + (extents.height - text_extents.1) / 2.) + extents.height)
                    as i32,
            },
            custom_glyphs: &[],
            default_color: style.font.color.into_glyphon(urgency),
        }
    }

    pub fn get_instance(&self, hovered: bool, scale: f32, urgency: &Urgency) -> buffers::Instance {
        let style = self.style(hovered);
        let extents = self.extents();

        buffers::Instance {
            rect_pos: [extents.x, extents.y],
            rect_size: [extents.width, extents.height],
            rect_color: style.background_color.to_linear(urgency),
            border_radius: style.border.radius.into(),
            border_size: style.border.size.into(),
            border_color: style.border.color.to_linear(urgency),
            scale,
        }
    }
}
