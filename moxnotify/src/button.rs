mod text;

use crate::{
    config::{ButtonType, Config},
    notification_manager::notification::Extents,
    surface::wgpu_surface::buffers,
};
use glyphon::FontSystem;
use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

#[derive(PartialEq)]
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
    pub fn get_by_coordinates(&self, x: f64, y: f64) -> Option<&Button> {
        self.buttons.iter().find(|button| {
            x >= button.x as f64
                && y >= button.y as f64
                && x <= button.x as f64 + button.width as f64
                && y <= button.y as f64 + button.height as f64
        })
    }
}

pub struct Button {
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
        let button = config.buttons.get(&button_type).unwrap();

        Self {
            x,
            y,
            width: button.width,
            height: button.height,
            config,
            action,
            button_type,
        }
    }

    pub fn extents(&self) -> Extents {
        Extents {
            x: self.x,
            width: self.width,
            height: self.height,
        }
    }

    pub fn get_instance(&self, x: f32, y: f32, scale: f32) -> buffers::Instance {
        let style = self.config.buttons.get(&self.button_type).unwrap();
        buffers::Instance {
            rect_pos: [x + self.x, y + self.y],
            rect_size: [self.width, self.height],
            rect_color: [1., 0., 0., 1.],
            border_radius: style.border.radius.into(),
            border_size: style.border.size,
            border_color: [0., 0., 0., 0.],
            scale,
        }
    }
}
