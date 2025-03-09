use crate::{
    buffers,
    config::{
        button::{self, ButtonState},
        Config,
    },
    notification_manager::notification::Extents,
};
use glyphon::FontSystem;
use std::{
    cell::{RefCell, RefMut},
    ops::{Deref, DerefMut},
    sync::Arc,
};

#[derive(PartialEq)]
pub enum ButtonType {
    Dismiss,
    Action,
}

#[derive(PartialEq)]
pub enum Action {
    DismissNotification,
}

#[derive(Default)]
pub struct ButtonManager {
    buttons: Vec<RefCell<Button>>,
}

impl Deref for ButtonManager {
    type Target = Vec<RefCell<Button>>;

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
    pub fn get_by_coordinates(&self, x: f64, y: f64) -> Option<RefMut<Button>> {
        let index = self.buttons.iter().position(|button| {
            let mut b = button.borrow_mut();
            b.unhover();
            x >= b.x as f64
                && y >= b.y as f64
                && x <= (b.x as f64 + b.width as f64)
                && y <= (b.y as f64 + b.height as f64)
        })?;

        self.buttons[index].borrow_mut().hover();

        Some(self.buttons[index].borrow_mut())
    }
}

pub struct Button {
    hovered: bool,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    config: Arc<Config>,
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
        let button = match button_type {
            ButtonType::Dismiss => &config.styles.default.buttons.dismiss,
            ButtonType::Action => &config.styles.default.buttons.action,
        };

        Self {
            hovered: false,
            x,
            y,
            width: button.width,
            height: button.height,
            config,
            action,
            button_type,
        }
    }

    pub fn hover(&mut self) {
        self.hovered = true;
    }

    pub fn unhover(&mut self) {
        self.hovered = false;
    }

    pub fn extents(&self) -> Extents {
        Extents {
            x: self.x,
            width: self.width,
            height: self.height,
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

    pub fn get_instance(&self, x: f32, y: f32, hovered: bool, scale: f32) -> buffers::Instance {
        let (button, style) = self.style(hovered);

        buffers::Instance {
            rect_pos: [x + self.x, y + self.y],
            rect_size: [self.width, self.height],
            rect_color: style.background_color.into(),
            border_radius: button.border.radius.into(),
            border_size: button.border.size,
            border_color: style.border_color.into(),
            scale,
        }
    }
}
