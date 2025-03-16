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
                font: Font {
                    size: 10.,
                    color: Color {
                        urgency_low: [255, 255, 255, 255],
                        urgency_normal: [255, 255, 255, 255],
                        urgency_critical: [255, 255, 255, 255],
                    },
                    ..Default::default()
                },
                background_color: Color::rgba([255, 0, 0, 255]),
                border: Border {
                    size: Insets {
                        left: 0.,
                        right: 0.,
                        top: 0.,
                        bottom: 0.,
                    },
                    radius: BorderRadius::circle(),
                    color: Color::rgba([255, 0, 0, 255]),
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
#[serde(default)]
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
                background_color: Color::rgba([255, 0, 0, 255]),
                border: Border {
                    size: Insets {
                        left: 0.,
                        right: 0.,
                        top: 0.,
                        bottom: 0.,
                    },
                    radius: BorderRadius::circle(),
                    color: Color::rgba([255, 0, 0, 255]),
                },
                font: Font {
                    size: 10.,
                    color: Color {
                        urgency_low: [255, 255, 255, 255],
                        urgency_normal: [255, 255, 255, 255],
                        urgency_critical: [255, 255, 255, 255],
                    },
                    ..Default::default()
                },
            },
        }
    }
}

#[derive(Deserialize)]
#[serde(default)]
pub struct ButtonState {
    pub background_color: Color,
    pub border: Border,
    pub font: Font,
}

impl Default for ButtonState {
    fn default() -> Self {
        Self {
            background_color: Color::rgba([255, 107, 107, 255]),
            border: Border {
                size: Insets {
                    left: 0.,
                    right: 0.,
                    top: 0.,
                    bottom: 0.,
                },
                radius: BorderRadius::circle(),
                color: Color::rgba([255, 107, 107, 255]),
            },
            font: Font {
                size: 10.,
                color: Color {
                    urgency_low: [255, 255, 255, 255],
                    urgency_normal: [255, 255, 255, 255],
                    urgency_critical: [255, 255, 255, 255],
                },
                ..Default::default()
            },
        }
    }
}
