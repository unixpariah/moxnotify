use super::{Button, ButtonType, Hint, State};
use crate::{
    buffers,
    component::{Bounds, Component},
    config::{button::ButtonState, Config},
    notification_manager::UiState,
    text::Text,
    Urgency,
};
use std::{cell::RefCell, rc::Rc, sync::Arc};

pub struct ActionButton {
    pub id: u32,
    pub ui_state: Rc<RefCell<UiState>>,
    pub x: f32,
    pub y: f32,
    pub hint: Hint,
    pub config: Rc<Config>,
    pub text: Text,
    pub action: Arc<str>,
    pub state: State,
    pub width: f32,
    pub tx: calloop::channel::Sender<(u32, Arc<str>)>,
}

impl Component for ActionButton {
    type Style = ButtonState;

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

    fn style(&self) -> &Self::Style {
        let style = match self
            .ui_state()
            .selected
            .is_some_and(|selected| selected == self.id)
        {
            true => &self.config.styles.hover.buttons.action,
            false => &self.config.styles.default.buttons.action,
        };
        match self.state() {
            State::Unhovered => &style.default,
            State::Hovered => &style.hover,
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

    fn set_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
        self.text.set_buffer_position(x, y);

        let bounds = self.render_bounds();
        self.hint.set_position(bounds.x, bounds.y);
    }
}

impl Button for ActionButton {
    fn hint(&self) -> &Hint {
        &self.hint
    }

    fn click(&self) {
        _ = self.tx.send((self.id, Arc::clone(&self.action)));
    }

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
        self.state = State::Hovered;
    }

    fn unhover(&mut self) {
        self.state = State::Unhovered
    }

    fn set_hint(&mut self, hint: Hint) {
        self.hint = hint;
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        button::{Button, Hint},
        config::Config,
        notification_manager::UiState,
        text::Text,
    };
    use glyphon::FontSystem;
    use std::{cell::RefCell, rc::Rc, sync::Arc};

    use super::ActionButton;

    #[test]
    fn test_action_button() {
        let config = Rc::new(Config::default());
        let ui_state = Rc::new(RefCell::new(UiState::default()));
        let hint = Hint {
            combination: "".into(),
            text: Text::new(&config.styles.default.font, &mut FontSystem::new(), ""),
            config: Rc::clone(&config),
            ui_state: Rc::clone(&ui_state),
            x: 0.,
            y: 0.,
        };

        let (tx, rx) = calloop::channel::channel();
        let test_id = 10;
        let test_action: Arc<str> = "test".into();
        let button = ActionButton {
            id: test_id,
            x: 0.,
            y: 0.,
            hint,
            text: Text::new(&config.styles.default.font, &mut FontSystem::new(), ""),
            state: crate::button::State::Hovered,
            config: Rc::clone(&config),
            ui_state: Rc::clone(&ui_state),
            tx,
            width: 100.,
            action: Arc::clone(&test_action),
        };

        button.click();

        match rx.try_recv() {
            Ok((id, action)) => {
                assert_eq!(id, test_id, "Button click should send button ID");
                assert_eq!(action, test_action, "Button click should send button ID");
            }
            Err(_) => panic!("Button click did not send ID through channel"),
        }
    }

    #[test]
    fn test_multiple_action_buttons() {
        let config = Rc::new(Config::default());
        let ui_state = Rc::new(RefCell::new(UiState::default()));

        let (tx, text_rx1) = calloop::channel::channel();

        let test_id1 = 1;
        let test_action1: Arc<str> = "test1".into();
        let hint1 = Hint {
            combination: "".into(),
            text: Text::new(&config.styles.default.font, &mut FontSystem::new(), ""),
            config: Rc::clone(&config),
            ui_state: Rc::clone(&ui_state),
            x: 0.,
            y: 0.,
        };
        let button1 = ActionButton {
            id: test_id1,
            x: 0.,
            y: 0.,
            hint: hint1,
            text: Text::new(&config.styles.default.font, &mut FontSystem::new(), ""),
            state: crate::button::State::Hovered,
            config: Rc::clone(&config),
            ui_state: Rc::clone(&ui_state),
            tx: tx.clone(),
            width: 100.,
            action: Arc::clone(&test_action1),
        };

        let (tx, text_rx2) = calloop::channel::channel();

        let test_id2 = 2;
        let test_action2: Arc<str> = "test2".into();
        let hint2 = Hint {
            combination: "".into(),
            text: Text::new(&config.styles.default.font, &mut FontSystem::new(), ""),
            config: Rc::clone(&config),
            ui_state: Rc::clone(&ui_state),
            x: 0.,
            y: 0.,
        };
        let button2 = ActionButton {
            id: test_id2,
            x: 0.,
            y: 0.,
            hint: hint2,
            text: Text::new(&config.styles.default.font, &mut FontSystem::new(), ""),
            state: crate::button::State::Hovered,
            config: Rc::clone(&config),
            ui_state: Rc::clone(&ui_state),
            tx: tx.clone(),
            width: 100.,
            action: Arc::clone(&test_action2),
        };

        button1.click();
        match text_rx1.try_recv() {
            Ok((id, action)) => {
                assert_eq!(id, test_id1, "First button ID should be sent");
                assert_eq!(action, test_action1, "First button action should be sent");
            }
            Err(_) => panic!("First button click did not send message"),
        }

        assert!(text_rx2.try_recv().is_err());

        button2.click();
        match text_rx2.try_recv() {
            Ok((id, action)) => {
                assert_eq!(id, test_id2, "Second button ID should be sent");
                assert_eq!(action, test_action2, "Second button action should be sent");
            }
            Err(_) => panic!("Second button click did not send message"),
        }

        assert!(text_rx1.try_recv().is_err());
    }
}
