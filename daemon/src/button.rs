use crate::{
    buffers,
    config::{button::ButtonState, keymaps::Mode, Config},
    notification_manager::notification::Extents,
    text::{Anchor, Text},
    Urgency,
};
use glyphon::{FontSystem, TextArea};
use std::sync::Arc;

#[derive(Clone)]
pub enum ButtonType {
    Dismiss,
    Action { text: Arc<str>, action: Arc<str> },
    Anchor { anchor: Arc<Anchor> },
}

use std::mem::discriminant;

impl PartialEq for ButtonType {
    fn eq(&self, other: &Self) -> bool {
        discriminant(self) == discriminant(other)
    }
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

    pub fn add(
        &mut self,
        button_type: ButtonType,
        config: Arc<Config>,
        font_system: &mut FontSystem,
    ) {
        let hint_chars: Vec<char> = config.general.hint_characters.chars().collect();
        let n = hint_chars.len();

        let mut m = self.buttons.len() as i32;
        let mut indices = Vec::new();

        loop {
            let remainder = (m % n as i32) as usize;
            indices.push(remainder);
            m = (m / n as i32) - 1;
            if m < 0 {
                break;
            }
        }

        indices.reverse();
        let combination: String = indices.into_iter().map(|i| hint_chars[i]).collect();

        let button = Button::new(&combination, button_type, config, font_system);
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

    pub fn get_by_character(&mut self, combination: &str) -> Option<ButtonType> {
        let button = self
            .buttons
            .iter()
            .find(|button| &*button.hint.combination == combination)?;

        Some(button.button_type.clone())
    }

    pub fn instances(
        &self,
        mode: Mode,
        container_hovered: bool,
        urgency: &Urgency,
        scale: f32,
    ) -> Vec<buffers::Instance> {
        let mut buttons = self
            .buttons
            .iter()
            .map(|button| button.instance(container_hovered, scale, urgency))
            .collect::<Vec<_>>();

        if mode == Mode::Hint && container_hovered {
            let hints = self
                .buttons
                .iter()
                .map(|button| {
                    button.hint.instance(
                        &button.rendered_extents(container_hovered),
                        scale,
                        urgency,
                    )
                })
                .collect::<Vec<_>>();
            buttons.extend_from_slice(&hints);
        }

        buttons
    }

    pub fn text_areas(
        &self,
        mode: Mode,
        container_hovered: bool,
        urgency: &Urgency,
        scale: f32,
    ) -> Vec<TextArea> {
        let mut text_areas = self
            .buttons
            .iter()
            .map(|button| button.text_area(container_hovered, scale, urgency))
            .collect::<Vec<_>>();

        if mode == Mode::Hint && container_hovered {
            let hints = self
                .buttons
                .iter()
                .map(|button| {
                    button.hint.text_area(
                        &button.rendered_extents(container_hovered),
                        scale,
                        urgency,
                    )
                })
                .collect::<Vec<_>>();
            text_areas.extend_from_slice(&hints);
        }

        text_areas
    }
}

pub struct Hint {
    text: Text,
    combination: Arc<str>,
    config: Arc<Config>,
}

impl Hint {
    pub fn new(combination: &str, config: Arc<Config>, font_system: &mut FontSystem) -> Self {
        Self {
            combination: combination.into(),
            text: Text::new(&config.styles.default.font, font_system, combination),
            config,
        }
    }

    pub fn instance(
        &self,
        button_extents: &Extents,
        scale: f32,
        urgency: &Urgency,
    ) -> buffers::Instance {
        let style = &self.config.styles.hover.hint;
        let text_extents = self.text.extents();

        buffers::Instance {
            rect_pos: [
                button_extents.x - style.width.resolve(text_extents.0) / 2.,
                button_extents.y - button_extents.height / 2.,
            ],
            rect_size: [
                style.width.resolve(text_extents.0) + style.padding.left + style.padding.right,
                style.height.resolve(text_extents.1) + style.padding.top + style.padding.bottom,
            ],
            rect_color: style.background.to_linear(urgency),
            border_radius: style.border.radius.into(),
            border_size: style.border.size.into(),
            border_color: style.border.color.to_linear(urgency),
            scale,
        }
    }

    pub fn text_area(&self, button_extents: &Extents, scale: f32, urgency: &Urgency) -> TextArea {
        let style = &self.config.styles.hover.hint;
        let text_extents = self.text.extents();
        let remaining_padding = style.width.resolve(text_extents.0) - text_extents.0;
        let (pl, _) = match (style.padding.left.is_auto(), style.padding.right.is_auto()) {
            (true, true) => (remaining_padding / 2., remaining_padding / 2.),
            (true, false) => (remaining_padding, style.padding.right.resolve(0.)),
            _ => (
                style.padding.left.resolve(0.),
                style.padding.right.resolve(0.),
            ),
        };
        let remaining_padding = style.height.resolve(text_extents.1) - text_extents.1;
        let (pt, _) = match (style.padding.top.is_auto(), style.padding.bottom.is_auto()) {
            (true, true) => (remaining_padding / 2., remaining_padding / 2.),
            (true, false) => (remaining_padding, style.padding.bottom.resolve(0.)),
            _ => (
                style.padding.top.resolve(0.),
                style.padding.bottom.resolve(0.),
            ),
        };
        TextArea {
            buffer: &self.text.buffer,
            left: button_extents.x + style.padding.left.resolve(pl)
                - style.width.resolve(text_extents.0) / 2.,
            top: button_extents.y + style.padding.top.resolve(pt)
                - style.height.resolve(text_extents.1) / 2.,
            scale,
            bounds: glyphon::TextBounds {
                left: (button_extents.x + style.padding.left.resolve(pl)
                    - style.width.resolve(text_extents.0) / 2.) as i32,
                top: (button_extents.y + style.padding.top.resolve(pt)
                    - style.height.resolve(text_extents.1) / 2.) as i32,
                right: (button_extents.x
                    + style.padding.left.resolve(pl)
                    + style.width.resolve(text_extents.0) / 2.) as i32,
                bottom: (button_extents.y
                    + style.padding.top.resolve(pt)
                    + style.height.resolve(text_extents.1) / 2.) as i32,
            },
            default_color: style.font.color.into_glyphon(urgency),
            custom_glyphs: &[],
        }
    }
}

pub struct Button {
    hint: Hint,
    pub hovered: bool,
    pub button_type: ButtonType,
    x: f32,
    y: f32,
    pub width: f32,
    config: Arc<Config>,
    text: Text,
}

impl Button {
    fn new(
        combination: &str,
        button_type: ButtonType,
        config: Arc<Config>,
        font_system: &mut FontSystem,
    ) -> Self {
        let font = match button_type {
            ButtonType::Dismiss => &config.styles.default.buttons.dismiss.default.font,
            ButtonType::Action { .. } => &config.styles.default.buttons.action.default.font,
            // Adding Anchor as a button is just for hint rendering
            ButtonType::Anchor { .. } => &config.styles.default.buttons.action.default.font,
        };

        let text = match &button_type {
            ButtonType::Dismiss => Text::new(font, font_system, "X"),
            ButtonType::Action { text, .. } => Text::new(font, font_system, text),
            // Adding Anchor as a button is just for hint rendering
            ButtonType::Anchor { .. } => Text::new(font, font_system, ""),
        };

        Self {
            hint: Hint::new(combination, Arc::clone(&config), font_system),
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
        if let ButtonType::Anchor { anchor } = &self.button_type {
            return Extents {
                x: self.x + anchor.extents().x,
                y: self.y + anchor.extents().y,
                width: anchor.extents().width,
                height: anchor.extents().height,
            };
        }

        let style = self.style(container_hovered);
        let text_extents = self.text.extents();

        let width = match &self.button_type {
            ButtonType::Dismiss => style.width.resolve(text_extents.0),
            ButtonType::Action { .. } => style.width.resolve(self.width),
            ButtonType::Anchor { .. } => unreachable!(),
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
            _ => &self.config.styles.default.buttons.action,
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

        let remaining_padding = extents.width - text_extents.0;
        let (pl, _) = match (style.padding.left.is_auto(), style.padding.right.is_auto()) {
            (true, true) => (remaining_padding / 2., remaining_padding / 2.),
            (true, false) => (remaining_padding, style.padding.right.resolve(0.)),
            _ => (
                style.padding.left.resolve(0.),
                style.padding.right.resolve(0.),
            ),
        };

        let remaining_padding = extents.height - text_extents.1;
        let (pt, _) = match (style.padding.top.is_auto(), style.padding.bottom.is_auto()) {
            (true, true) => (remaining_padding / 2., remaining_padding / 2.),
            (true, false) => (remaining_padding, style.padding.bottom.resolve(0.)),
            _ => (
                style.padding.top.resolve(0.),
                style.padding.bottom.resolve(0.),
            ),
        };

        glyphon::TextArea {
            buffer: &self.text.buffer,
            left: extents.x + style.border.size.left + style.padding.left.resolve(pl),
            top: extents.y + style.border.size.top + style.padding.top.resolve(pt),
            scale,
            bounds: glyphon::TextBounds {
                left: (extents.x + style.border.size.left + style.padding.left.resolve(pl)) as i32,
                top: (extents.y + style.border.size.top + style.padding.top.resolve(pt)) as i32,
                right: (extents.x
                    + style.border.size.left
                    + style.padding.left.resolve(pl)
                    + text_extents.0) as i32,
                bottom: (extents.y
                    + style.border.size.top
                    + style.padding.top.resolve(pt)
                    + text_extents.1) as i32,
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

        if let ButtonType::Anchor { .. } = self.button_type {
            return buffers::Instance {
                rect_pos: [0., 0.],
                rect_size: [0., 0.],
                rect_color: style.background.to_linear(urgency),
                border_radius: style.border.radius.into(),
                border_size: style.border.size.into(),
                border_color: style.border.color.to_linear(urgency),
                scale,
            };
        }

        buffers::Instance {
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
        }
    }
}
