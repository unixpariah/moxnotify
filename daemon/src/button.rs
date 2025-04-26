use crate::{
    buffers,
    config::{button::ButtonState, Config},
    notification_manager::{notification::Extents, UiState},
    text::Text,
    Urgency,
};
use glyphon::{FontSystem, TextArea};
use std::{cell::RefCell, rc::Rc, sync::Arc};

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
    fn ui_state(&self) -> std::cell::Ref<'_, UiState>;

    fn style(&self) -> &ButtonState;

    fn instance(&self, urgency: &Urgency) -> buffers::Instance;

    fn text_area(&self, urgency: &Urgency) -> TextArea;

    fn bounds(&self) -> Bounds;

    fn render_bounds(&self) -> Bounds {
        let bounds = self.bounds();
        let style = self.style();

        Bounds {
            x: bounds.x + style.margin.left,
            y: bounds.y + style.margin.top,
            width: bounds.width - style.margin.left - style.margin.right,
            height: bounds.height - style.margin.top - style.margin.bottom,
        }
    }

    fn set_position(&mut self, x: f32, y: f32);
}

pub trait Button: Component {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

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
    ui_state: Rc<RefCell<UiState>>,
}

impl Component for DismissButton {
    fn ui_state(&self) -> std::cell::Ref<'_, UiState> {
        self.ui_state.borrow()
    }

    fn style(&self) -> &ButtonState {
        let style = match self.ui_state().container_hovered {
            true => &self.config.styles.hover.buttons.dismiss,
            false => &self.config.styles.default.buttons.dismiss,
        };
        match self.state() {
            State::Unhovered => &style.default,
            State::Hovered => &style.hover,
            State::Clicked => todo!(),
        }
    }

    fn instance(&self, urgency: &Urgency) -> buffers::Instance {
        let style = self.style();
        let bounds = self.render_bounds();

        buffers::Instance {
            rect_pos: [bounds.x, bounds.y],
            rect_size: [
                bounds.width - style.border.size.left - style.border.size.right,
                bounds.height - style.border.size.top - style.border.size.bottom,
            ],
            rect_color: style.background.to_linear(urgency),
            border_radius: style.border.radius.into(),
            border_size: style.border.size.into(),
            border_color: style.border.color.to_linear(urgency),
            scale: self.ui_state().scale,
        }
    }

    fn text_area(&self, urgency: &Urgency) -> glyphon::TextArea {
        let extents = self.render_bounds();
        let style = self.style();
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
            scale: self.ui_state().scale,
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

    fn set_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
        self.text.set_buffer_position(x, y);
    }
}

impl Button for DismissButton {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

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
    ui_state: Rc<RefCell<UiState>>,
    x: f32,
    y: f32,
    hovered: bool,
    hint: Hint,
    config: Arc<Config>,
    text: Text,
    action: Arc<str>,
    state: State,
    width: f32,
}

impl Component for ActionButton {
    fn ui_state(&self) -> std::cell::Ref<'_, UiState> {
        self.ui_state.borrow()
    }

    fn instance(&self, urgency: &Urgency) -> buffers::Instance {
        let style = self.style();
        let bounds = self.render_bounds();

        buffers::Instance {
            rect_pos: [bounds.x, bounds.y],
            rect_size: [
                bounds.width - style.border.size.left - style.border.size.right,
                bounds.height - style.border.size.top - style.border.size.bottom,
            ],
            rect_color: style.background.to_linear(urgency),
            border_radius: style.border.radius.into(),
            border_size: style.border.size.into(),
            border_color: style.border.color.to_linear(urgency),
            scale: self.ui_state().scale,
        }
    }

    fn text_area(&self, urgency: &Urgency) -> glyphon::TextArea {
        let extents = self.render_bounds();
        let style = self.style();
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
            scale: self.ui_state().scale,
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

    fn style(&self) -> &ButtonState {
        let style = match self.ui_state().container_hovered {
            true => &self.config.styles.hover.buttons.dismiss,
            false => &self.config.styles.default.buttons.dismiss,
        };
        match self.state() {
            State::Unhovered => &style.default,
            State::Hovered => &style.hover,
            State::Clicked => todo!(),
        }
    }

    fn bounds(&self) -> Bounds {
        let style = self.style();
        let text_extents = self.text.extents();

        let width = style.width.resolve(self.width)
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

    fn set_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
        self.text.set_buffer_position(x, y);
    }
}

impl Button for ActionButton {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

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

#[derive(Clone, PartialEq)]
pub enum ButtonType {
    Dismiss,
    Action,
}

pub struct ButtonManager {
    buttons: Vec<Box<dyn Button>>,
    urgency: Urgency,
    pub ui_state: Rc<RefCell<UiState>>,
}

impl ButtonManager {
    pub fn new(urgency: Urgency, ui_state: Rc<RefCell<UiState>>) -> Self {
        Self {
            buttons: Vec::new(),
            urgency,
            ui_state,
        }
    }

    pub fn set_action_widths(&mut self, width: f32) {
        self.buttons
            .iter_mut()
            .filter_map(|button| button.as_any_mut().downcast_mut::<ActionButton>())
            .for_each(|action| {
                action.width = width;
            });
    }

    pub fn click(&self) {
        self.buttons.iter().for_each(|button| {});
    }

    pub fn buttons(&self) -> &[Box<dyn Button>] {
        &self.buttons
    }

    pub fn buttons_mut(&mut self) -> &mut [Box<dyn Button>] {
        &mut self.buttons
    }

    pub fn add_dismiss(mut self, config: Arc<Config>, font_system: &mut FontSystem) -> Self {
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

        let font = &config.styles.default.buttons.dismiss.default.font;
        let text = Text::new(font, font_system, "X");

        let button = DismissButton {
            ui_state: Rc::clone(&self.ui_state),
            hint: Hint::new(&combination, Arc::clone(&config), font_system),
            text,
            hovered: false,
            x: 0.,
            y: 0.,
            config,
            state: State::Unhovered,
        };

        self.buttons.push(Box::new(button));

        self
    }

    pub fn add_actions(
        mut self,
        actions: &[(Arc<str>, Arc<str>)],
        config: Arc<Config>,
        font_system: &mut FontSystem,
    ) -> Self {
        if actions.is_empty() {
            return self;
        }

        let mut buttons = actions
            .iter()
            .cloned()
            .map(|action| {
                let font = &config.styles.default.buttons.action.default.font;
                let text = Text::new(font, font_system, &action.0);

                Box::new(ActionButton {
                    ui_state: Rc::clone(&self.ui_state),
                    hint: Hint::new("", Arc::clone(&config), font_system),
                    text,
                    hovered: false,
                    x: 0.,
                    y: 0.,
                    config: Arc::clone(&config),
                    action: action.1,
                    state: State::Unhovered,
                    width: 0.,
                }) as Box<dyn Button>
            })
            .collect::<Vec<Box<dyn Button>>>();

        self.buttons.append(&mut buttons);

        self
    }

    pub fn hover(&mut self, x: f64, y: f64) -> bool {
        self.buttons
            .iter_mut()
            .filter_map(|button| {
                let bounds = button.render_bounds();
                if x >= bounds.x as f64
                    && y >= bounds.y as f64
                    && x <= (bounds.x + bounds.width) as f64
                    && y <= (bounds.y + bounds.height) as f64
                {
                    button.hover();
                    Some(true)
                } else {
                    button.unhover();
                    None
                }
            })
            .next()
            .is_some()
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
            .map(|button| button.instance(&self.urgency))
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

    pub fn text_areas(&self) -> Vec<TextArea> {
        let mut text_areas = self
            .buttons
            .iter()
            .map(|button| button.text_area(&self.urgency))
            .collect::<Vec<_>>();

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
