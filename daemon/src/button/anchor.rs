use super::{Button, Component, Hint, State};
use crate::{
    buffers,
    component::Bounds,
    config::{button::ButtonState, Config},
    notification_manager::UiState,
    text::{Anchor, Text},
};
use std::{cell::RefCell, rc::Rc, sync::Arc};

pub struct AnchorButton {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub hint: Hint,
    pub config: Rc<Config>,
    pub text: Text,
    pub state: State,
    pub ui_state: Rc<RefCell<UiState>>,
    pub tx: calloop::channel::Sender<Arc<str>>,
    pub anchor: Rc<Anchor>,
}

impl Component for AnchorButton {
    type Style = ButtonState;

    fn ui_state(&self) -> std::cell::Ref<'_, UiState> {
        self.ui_state.borrow()
    }

    fn style(&self) -> &Self::Style {
        let style = match self
            .ui_state()
            .selected
            .is_some_and(|selected| selected == self.id)
        {
            true => &self.config.styles.hover.buttons.dismiss,
            false => &self.config.styles.default.buttons.dismiss,
        };
        match self.state() {
            State::Unhovered => &style.default,
            State::Hovered => &style.hover,
        }
    }

    fn instance(&self, urgency: &crate::Urgency) -> buffers::Instance {
        let style = self.style();
        let bounds = self.render_bounds();
        buffers::Instance {
            rect_pos: [bounds.x, bounds.y],
            rect_size: [bounds.width, bounds.height],
            rect_color: style.background.to_linear(urgency),
            border_radius: style.border.radius.into(),
            border_size: style.border.size.into(),
            border_color: style.border.color.to_linear(urgency),
            scale: 0.,
        }
    }

    fn text_area(&self, urgency: &crate::Urgency) -> glyphon::TextArea {
        let style = self.style();
        glyphon::TextArea {
            buffer: &self.text.buffer,
            left: 0.,
            top: 0.,
            scale: 0.,
            bounds: glyphon::TextBounds {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            custom_glyphs: &[],
            default_color: style.font.color.into_glyphon(urgency),
        }
    }

    fn bounds(&self) -> Bounds {
        let anchor_extents = self.anchor.extents();

        Bounds {
            x: self.x + anchor_extents.x,
            y: self.y + anchor_extents.y,
            width: anchor_extents.width,
            height: anchor_extents.height,
        }
    }

    fn render_bounds(&self) -> Bounds {
        self.bounds()
    }

    fn set_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;

        let bounds = self.render_bounds();
        self.hint.set_position(bounds.x, bounds.y);
    }
}

impl Button for AnchorButton {
    fn hint(&self) -> &Hint {
        &self.hint
    }

    fn click(&self) {
        self.tx.send(Arc::clone(&self.anchor.href));
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn button_type(&self) -> super::ButtonType {
        super::ButtonType::Anchor
    }

    fn state(&self) -> State {
        self.state
    }

    fn hover(&mut self) {
        self.state = State::Hovered;
    }

    fn unhover(&mut self) {
        self.state = State::Unhovered
    }

    fn set_hint(&mut self, hint: Hint) {
        self.hint = hint;
    }
}
