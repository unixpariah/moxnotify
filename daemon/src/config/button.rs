use super::{Border, BorderRadius, Color, Font, Insets};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Buttons {
    pub dismiss: Button,
    pub action: Button,
}

impl Default for Buttons {
    fn default() -> Self {
        let action = Button {
            width: 200.0,
            default: ButtonState {
                font: Font::default(),
                background_color: Color::rgba([255, 255, 255, 255]),
                border: Border {
                    size: Insets {
                        left: 0.,
                        right: 0.,
                        top: 0.,
                        bottom: 0.,
                    },
                    radius: BorderRadius::default(),
                    color: Color::rgba([0, 0, 0, 0]),
                },
            },
            hover: ButtonState {
                font: Font::default(),
                background_color: Color::rgba([255, 255, 255, 255]),
                border: Border {
                    size: Insets {
                        left: 0.,
                        right: 0.,
                        top: 0.,
                        bottom: 0.,
                    },
                    radius: BorderRadius::default(),
                    color: Color::rgba([0, 0, 0, 0]),
                },
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
pub struct Button {
    pub width: f32,
    pub height: f32,
    pub default: ButtonState,
    pub hover: ButtonState,
}

impl Default for Button {
    fn default() -> Self {
        Self {
            width: 20.0,
            height: 20.0,
            default: ButtonState::default(),
            hover: ButtonState {
                background_color: Color::rgba([255, 255, 255, 255]),
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
                ..Default::default()
            },
        }
    }
}

#[derive(Deserialize)]
pub struct ButtonState {
    pub background_color: Color,
    pub border: Border,
    pub font: Font,
}

impl Default for ButtonState {
    fn default() -> Self {
        Self {
            background_color: Color::rgba([192, 202, 245, 255]),
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
