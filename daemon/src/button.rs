use crate::{
    buffers,
    config::{button::ButtonState, Config},
    notification_manager::notification::Extents,
    text::Text,
    Urgency,
};
use glyphon::{FontSystem, TextArea};
use std::{mem::discriminant, sync::Arc};

#[derive(Clone, Copy)]
pub enum State {
    Unhovered,
    Hovered,
    Clicked,
}

#[derive(Default)]
pub struct Bounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub trait Component {
    fn style(&self) -> &ButtonState;

    fn instance(&self) -> buffers::Instance;

    fn bounds(&self) -> Bounds;

    fn render_bounds(&self) -> Bounds;
}

pub trait Button: Component {
    fn button_type(&self) -> ButtonType;

    fn state(&self) -> State;

    fn hover(&mut self);

    fn unhover(&mut self);

    fn set_hint(&mut self, hint: Hint);
}

pub struct DismissButton {
    x: f32,
    y: f32,
    hovered: bool,
    hint: Hint,
    config: Arc<Config>,
    text: Text,
    state: State,
}

impl Component for DismissButton {
    fn style(&self) -> &ButtonState {
        let style = &self.config.styles.hover.buttons.dismiss; // TODO: actually figure out the hover state of container
        match self.state() {
            State::Unhovered => &style.default,
            State::Hovered => &style.hover,
            State::Clicked => todo!(),
        }
    }

    fn instance(&self) -> buffers::Instance {
        let style = self.style();
        let bounds = self.render_bounds();

        // Properly implement color and scale
        buffers::Instance {
            rect_pos: [bounds.x, bounds.y],
            rect_size: [
                bounds.width - style.border.size.left - style.border.size.right,
                bounds.height - style.border.size.top - style.border.size.bottom,
            ],
            rect_color: style.background.to_linear(&Urgency::Normal),
            border_radius: style.border.radius.into(),
            border_size: style.border.size.into(),
            border_color: style.border.color.to_linear(&Urgency::Normal),
            scale: 1.0,
        }
    }

    fn bounds(&self) -> Bounds {
        let style = self.style();
        let text_extents = self.text.extents();

        let width = style.width.resolve(text_extents.0)
            + style.border.size.left
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

        Bounds {
            x: self.x,
            y: self.y,
            width,
            height,
        }
    }

    fn render_bounds(&self) -> Bounds {
        let bounds = self.bounds();
        Bounds {
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: bounds.height,
        }
    }
}

impl Button for DismissButton {
    fn button_type(&self) -> ButtonType {
        ButtonType::Dismiss
    }

    fn state(&self) -> State {
        self.state
    }

    fn hover(&mut self) {
        self.hovered = true;
    }

    fn unhover(&mut self) {
        self.hovered = false;
    }

    fn set_hint(&mut self, hint: Hint) {
        self.hint = hint;
    }
}

struct ActionButton {
    x: f32,
    y: f32,
    hovered: bool,
    hint: Hint,
    config: Arc<Config>,
    text: Text,
    action: Arc<str>,
    state: State,
}

impl Component for ActionButton {
    fn instance(&self) -> buffers::Instance {
        let style = self.style();
        let bounds = self.render_bounds();

        buffers::Instance {
            rect_pos: [bounds.x, bounds.y],
            rect_size: [
                bounds.width - style.border.size.left - style.border.size.right,
                bounds.height - style.border.size.top - style.border.size.bottom,
            ],
            rect_color: style.background.to_linear(&Urgency::Normal),
            border_radius: style.border.radius.into(),
            border_size: style.border.size.into(),
            border_color: style.border.color.to_linear(&Urgency::Normal),
            scale: 1.,
        }
    }

    fn style(&self) -> &ButtonState {
        let style = &self.config.styles.hover.buttons.action; // TODO: actually figure out the hover state of container
        match self.state() {
            State::Unhovered => &style.default,
            State::Hovered => &style.hover,
            State::Clicked => todo!(),
        }
    }

    fn bounds(&self) -> Bounds {
        let style = self.style();
        let text_extents = self.text.extents();

        let width = 0.;
        let width = style.width.resolve(width)
            + style.border.size.left
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

        Bounds {
            x: self.x,
            y: self.y,
            width,
            height,
        }
    }

    fn render_bounds(&self) -> Bounds {
        let bounds = self.bounds();
        Bounds {
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: bounds.height,
        }
    }
}

impl Button for ActionButton {
    fn button_type(&self) -> ButtonType {
        ButtonType::Action
    }

    fn state(&self) -> State {
        self.state
    }

    fn hover(&mut self) {
        self.hovered = true;
    }

    fn unhover(&mut self) {
        self.hovered = false;
    }

    fn set_hint(&mut self, hint: Hint) {
        self.hint = hint;
    }
}

#[derive(Clone)]
pub enum ButtonType {
    Dismiss,
    Action,
}

impl PartialEq for ButtonType {
    fn eq(&self, other: &Self) -> bool {
        discriminant(self) == discriminant(other)
    }
}

#[derive(Default)]
pub struct ButtonManager {
    buttons: Vec<Box<dyn Button>>,
}

impl ButtonManager {
    pub fn buttons(&self) -> &[Box<dyn Button>] {
        &self.buttons
    }

    pub fn buttons_mut(&mut self) -> &mut [Box<dyn Button>] {
        &mut self.buttons
    }

    pub fn add<T>(&mut self, mut button: T, config: Arc<Config>, font_system: &mut FontSystem)
    where
        T: Button + 'static,
    {
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

        button.set_hint(Hint::new(&combination, config, font_system));

        self.buttons.push(Box::new(button));
    }

    pub fn hover(&mut self, x: f64, y: f64) {
        self.buttons.iter_mut().for_each(|button| {
            let bounds = button.render_bounds();
            if x >= bounds.x as f64
                && y >= bounds.y as f64
                && x <= (bounds.x + bounds.width) as f64
                && y <= (bounds.y + bounds.height) as f64
            {
                button.hover();
            } else {
                button.unhover();
            }
        });
    }

    //pub fn get_by_character(&mut self, combination: &str) -> Option<ButtonType> {
    //let button = self
    //.buttons
    //.iter()
    //.find(|button| &*button.hint.combination == combination)?;

    //Some(button.button_type.clone())
    //}

    pub fn instances(&self) -> Vec<buffers::Instance> {
        let mut buttons = self
            .buttons
            .iter()
            .map(|button| button.instance())
            .collect::<Vec<_>>();

        //if mode == Mode::Hint && container_hovered {
        //let hints = self
        //.buttons
        //.iter()
        //.map(|button| {
        //button.hint.instance(
        //&button.rendered_extents(container_hovered),
        //scale,
        //urgency,
        //)
        //})
        //.collect::<Vec<_>>();
        //buttons.extend_from_slice(&hints);
        //}

        buttons
    }

    //pub fn text_areas(
    //&self,
    //mode: Mode,
    //container_hovered: bool,
    //urgency: &Urgency,
    //scale: f32,
    //) -> Vec<TextArea> {
    //let mut text_areas = self
    //.buttons
    //.iter()
    //.map(|button| button.text_area(container_hovered, scale, urgency))
    //.collect::<Vec<_>>();

    //if mode == Mode::Hint && container_hovered {
    //let hints = self
    //.buttons
    //.iter()
    //.map(|button| {
    //button.hint.text_area(
    //&button.rendered_extents(container_hovered),
    //scale,
    //urgency,
    //)
    //})
    //.collect::<Vec<_>>();
    //text_areas.extend_from_slice(&hints);
    //}

    //text_areas
    //}
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
