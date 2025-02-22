mod text;

use crate::{config::Config, surface::wgpu_surface::buffers};
use glyphon::FontSystem;
use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

#[derive(PartialEq)]
pub enum Action {
    DismissNotification,
}

pub enum ButtonType {
    Dismiss,
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
    button_type: ButtonType,
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
        Self {
            x,
            y,
            width: 20.,
            height: 20.,
            config,
            action,
            button_type,
        }
    }

    pub fn get_instance(&self, x: f32, y: f32, scale: f32) -> buffers::Instance {
        buffers::Instance {
            rect_pos: [x + self.x, y + self.y],
            rect_size: [self.width, self.height],
            rect_color: [1., 0., 0., 1.],
            border_radius: [50., 50., 50., 50.],
            border_size: 0.,
            border_color: [0., 0., 0., 0.],
            scale,
        }
    }

    //pub fn text_area(&self) -> TextArea {
    //    TextArea
    //}
}
