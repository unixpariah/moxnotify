mod action;
mod dismiss;

use crate::{
    buffers,
    component::{Bounds, Component},
    config::{self, button::ButtonState, keymaps, Config},
    notification_manager::UiState,
    text::Text,
    Moxnotify, Urgency,
};
use action::ActionButton;
use calloop::LoopHandle;
use glyphon::{FontSystem, TextArea};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Copy, Debug)]
pub enum State {
    Unhovered,
    Hovered,
    Clicked,
}

pub trait Button: Component {
    fn hint(&self) -> &Hint;

    fn click(&self);

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

    fn button_type(&self) -> ButtonType;

    fn state(&self) -> State;

    fn hover(&mut self);

    fn unhover(&mut self);

    fn set_hint(&mut self, hint: Hint);
}

#[derive(Clone, PartialEq)]
pub enum ButtonType {
    Dismiss,
    Action,
}

pub struct ButtonManager {
    id: u32,
    buttons: Vec<Box<dyn Button<Style = ButtonState>>>,
    urgency: Urgency,
    pub ui_state: Rc<RefCell<UiState>>,
    loop_handle: Option<LoopHandle<'static, Moxnotify>>,
    config: Rc<Config>,
}

impl ButtonManager {
    pub fn new(
        id: u32,
        urgency: Urgency,
        ui_state: Rc<RefCell<UiState>>,
        loop_handle: Option<LoopHandle<'static, Moxnotify>>,
        config: Rc<Config>,
    ) -> Self {
        Self {
            id,
            buttons: Vec::new(),
            urgency,
            ui_state,
            loop_handle,
            config,
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

    pub fn buttons(&self) -> &[Box<dyn Button<Style = ButtonState>>] {
        &self.buttons
    }

    pub fn buttons_mut(&mut self) -> &mut [Box<dyn Button<Style = ButtonState>>] {
        &mut self.buttons
    }

    pub fn finish(mut self, font_system: &mut FontSystem) -> Self {
        let hint_chars: Vec<char> = self.config.general.hint_characters.chars().collect();
        let n = hint_chars.len() as i32;

        self.buttons.iter_mut().enumerate().for_each(|(i, button)| {
            let mut m = i as i32;
            let mut indices = Vec::new();

            loop {
                let rem = (m % n) as usize;
                indices.push(rem);
                m = (m / n) - 1;
                if m < 0 {
                    break;
                }
            }

            indices.reverse();
            let combination: String = indices.into_iter().map(|i| hint_chars[i]).collect();
            let hint = Hint::new(
                &combination,
                Rc::clone(&self.config),
                font_system,
                Rc::clone(&self.ui_state),
            );

            button.set_hint(hint);
        });

        self
    }

    pub fn click(&self, x: f64, y: f64) -> bool {
        self.buttons
            .iter()
            .filter_map(|button| {
                let bounds = button.render_bounds();
                if x >= bounds.x as f64
                    && y >= bounds.y as f64
                    && x <= (bounds.x + bounds.width) as f64
                    && y <= (bounds.y + bounds.height) as f64
                {
                    button.click();
                    Some(true)
                } else {
                    None
                }
            })
            .next()
            .is_some()
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

    pub fn hint<T>(&mut self, combination: T)
    where
        T: AsRef<str>,
    {
        if let Some(button) = self
            .buttons
            .iter()
            .find(|button| &*button.hint().combination == combination.as_ref())
        {
            button.click();
        }
    }

    pub fn instances(&self) -> Vec<buffers::Instance> {
        let mut buttons = self
            .buttons
            .iter()
            .map(|button| button.instance(&self.urgency))
            .collect::<Vec<_>>();

        let ui_state = self.ui_state.borrow();
        if ui_state.mode == keymaps::Mode::Hint && ui_state.selected == Some(self.id) {
            let hints = self
                .buttons
                .iter()
                .map(|button| button.hint().instance(&self.urgency))
                .collect::<Vec<_>>();
            buttons.extend_from_slice(&hints);
        }

        buttons
    }

    pub fn text_areas(&self) -> Vec<TextArea> {
        let mut text_areas = self
            .buttons
            .iter()
            .map(|button| button.text_area(&self.urgency))
            .collect::<Vec<_>>();

        let ui_state = self.ui_state.borrow();
        if ui_state.mode == keymaps::Mode::Hint && ui_state.selected == Some(self.id) {
            let hints = self
                .buttons
                .iter()
                .map(|button| button.hint().text_area(&self.urgency))
                .collect::<Vec<_>>();
            text_areas.extend_from_slice(&hints);
        }

        text_areas
    }
}

pub struct Hint {
    combination: Box<str>,
    text: Text,
    config: Rc<Config>,
    ui_state: Rc<RefCell<UiState>>,
}

impl Component for Hint {
    type Style = config::Hint;

    fn ui_state(&self) -> std::cell::Ref<'_, UiState> {
        self.ui_state.borrow()
    }

    fn style(&self) -> &Self::Style {
        &self.config.styles.hover.hint
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
            x: 0.,
            y: 0.,
            width,
            height,
        }
    }

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

    fn instance(&self, urgency: &Urgency) -> buffers::Instance {
        let style = &self.config.styles.hover.hint;
        let bounds = self.render_bounds();

        buffers::Instance {
            rect_pos: [bounds.x, bounds.y],
            rect_size: [bounds.width, bounds.height],
            rect_color: style.background.to_linear(urgency),
            border_radius: style.border.radius.into(),
            border_size: style.border.size.into(),
            border_color: style.border.color.to_linear(urgency),
            scale: self.ui_state.borrow().scale,
        }
    }

    fn set_position(&mut self, _: f32, _: f32) {}

    fn text_area(&self, urgency: &Urgency) -> TextArea {
        let style = self.style();
        let bounds = self.render_bounds();

        TextArea {
            buffer: &self.text.buffer,
            left: (bounds.x + style.border.size.left),
            top: (bounds.y + style.border.size.top),
            scale: self.ui_state.borrow().scale,
            bounds: glyphon::TextBounds {
                left: (bounds.x + style.border.size.left) as i32,
                top: (bounds.y + style.border.size.top) as i32,
                right: (bounds.x + bounds.width - style.border.size.left - style.border.size.right)
                    as i32,
                bottom: (bounds.y + bounds.height
                    - style.border.size.bottom
                    - style.border.size.top) as i32,
            },
            default_color: style.font.color.into_glyphon(urgency),
            custom_glyphs: &[],
        }
    }
}

impl Hint {
    pub fn new<T>(
        combination: T,
        config: Rc<Config>,
        font_system: &mut FontSystem,
        ui_state: Rc<RefCell<UiState>>,
    ) -> Self
    where
        T: AsRef<str>,
    {
        Self {
            combination: combination.as_ref().into(),
            ui_state,
            text: Text::new(
                &config.styles.default.font,
                font_system,
                combination.as_ref(),
            ),
            config,
        }
    }
}
