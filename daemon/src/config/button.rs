use super::{Border, BorderRadius, Color, Font, Insets};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(default)]
pub struct Buttons {
    pub dismiss: Button,
    pub action: Button,
}

impl Default for Buttons {
    fn default() -> Self {
        let action = Button {
            width: 200.0,
            default: ButtonState {
                background_color: Color::rgba([255, 0, 0, 255]),
                border_color: Color::rgba([255, 0, 0, 255]),
            },
            ..Default::default()
        };

        Self {
            dismiss: Button::default(),
            action,
        }
    }
}

#[derive(Deserialize)]
#[serde(default)]
pub struct Button {
    pub width: f32,
    pub height: f32,
    pub border: Border,
    pub default: ButtonState,
    pub hover: ButtonState,
    pub font: Font,
}

impl Default for Button {
    fn default() -> Self {
        Self {
            width: 20.0,
            height: 20.0,
            border: Border {
                size: Insets {
                    left: 0.,
                    right: 0.,
                    top: 0.,
                    bottom: 0.,
                },
                radius: BorderRadius::circle(),
                color: Color::default(),
            },
            default: ButtonState::default(),
            hover: ButtonState {
                background_color: Color::rgba([255, 0, 0, 255]),
                border_color: Color::rgba([255, 0, 0, 255]),
            },
            font: Font {
                size: 20.,
                ..Default::default()
            },
        }
    }
}

#[derive(Deserialize)]
#[serde(default)]
pub struct ButtonState {
    pub background_color: Color,
    pub border_color: Color,
}

impl Default for ButtonState {
    fn default() -> Self {
        Self {
            background_color: Color::rgba([255, 107, 107, 255]),
            border_color: Color::rgba([255, 107, 107, 255]),
        }
    }
}
