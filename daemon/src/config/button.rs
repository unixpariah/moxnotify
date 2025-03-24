use super::{Border, BorderRadius, Color, Font, Insets, Size};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Buttons {
    #[serde(default)]
    pub dismiss: Button,
    #[serde(default = "Button::default_action")]
    pub action: Button,
}

impl Default for Buttons {
    fn default() -> Self {
        Self {
            dismiss: Button::default(),
            action: Button::default_action(),
        }
    }
}

#[derive(Deserialize)]
pub struct Button {
    pub default: ButtonState,
    pub hover: ButtonState,
}

impl Button {
    fn default_action() -> Self {
        Self {
            default: ButtonState {
                width: Size::Auto,
                height: Size::Value(30.),
                font: Font::default(),
                background: Color::rgba([22, 22, 30, 0]),
                border: Border::default(),
            },
            hover: ButtonState {
                width: Size::Auto,
                height: Size::Value(30.),
                font: Font::default(),
                background: Color::rgba([247, 118, 142, 255]),
                border: Border::default(),
            },
        }
    }
}

impl Default for Button {
    fn default() -> Self {
        Self {
            default: ButtonState::default(),
            hover: ButtonState::default_hover(),
        }
    }
}

#[derive(Deserialize)]
pub struct ButtonState {
    pub width: Size,
    pub height: Size,
    pub background: Color,
    pub border: Border,
    pub font: Font,
}

impl ButtonState {
    fn default_hover() -> Self {
        Self {
            background: Color::rgba([255, 255, 255, 255]),
            ..Default::default()
        }
    }
}

impl Default for ButtonState {
    fn default() -> Self {
        Self {
            width: Size::Value(20.),
            height: Size::Value(20.),
            background: Color::rgba([192, 202, 245, 255]),
            border: Border {
                size: Insets {
                    left: 0.,
                    right: 0.,
                    top: 0.,
                    bottom: 0.,
                },
                radius: BorderRadius::circle(),
                ..Default::default()
            },
            font: Font {
                color: Color::rgba([47, 53, 73, 255]),
                ..Default::default()
            },
        }
    }
}
