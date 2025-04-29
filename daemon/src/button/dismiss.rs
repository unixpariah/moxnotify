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

pub struct DismissButton {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub hint: Hint,
    pub config: Rc<Config>,
    pub text: Text,
    pub state: State,
    pub ui_state: Rc<RefCell<UiState>>,
    pub tx: Option<calloop::channel::Sender<u32>>,
    pub app_name: Arc<str>,
}

impl Component for DismissButton {
    type Style = ButtonState;

    fn get_id(&self) -> u32 {
        self.id
    }

    fn get_config(&self) -> &Config {
        &self.config
    }

    fn get_app_name(&self) -> &str {
        &self.app_name
    }

    fn get_ui_state(&self) -> std::cell::Ref<'_, UiState> {
        self.ui_state.borrow()
    }

    fn get_style(&self) -> &Self::Style {
        let style = self.get_notification_style();
        match self.state() {
            State::Unhovered => &style.buttons.dismiss.default,
            State::Hovered => &style.buttons.dismiss.hover,
        }
    }

    fn get_instances(&self, urgency: &Urgency) -> Vec<buffers::Instance> {
        let style = self.get_style();
        let bounds = self.get_render_bounds();

        vec![buffers::Instance {
            rect_pos: [bounds.x, bounds.y],
            rect_size: [
                bounds.width - style.border.size.left - style.border.size.right,
                bounds.height - style.border.size.top - style.border.size.bottom,
            ],
            rect_color: style.background.to_linear(urgency),
            border_radius: style.border.radius.into(),
            border_size: style.border.size.into(),
            border_color: style.border.color.to_linear(urgency),
            scale: self.get_ui_state().scale,
        }]
    }

    fn get_text_areas(&self, urgency: &Urgency) -> Vec<glyphon::TextArea> {
        let extents = self.get_render_bounds();
        let style = self.get_style();
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

        vec![glyphon::TextArea {
            buffer: &self.text.buffer,
            left: extents.x + style.border.size.left + style.padding.left.resolve(pl),
            top: extents.y + style.border.size.top + style.padding.top.resolve(pt),
            scale: self.get_ui_state().scale,
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
        }]
    }

    fn get_bounds(&self) -> Bounds {
        let style = self.get_style();
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

    fn get_render_bounds(&self) -> Bounds {
        let bounds = self.get_bounds();
        let style = self.get_style();

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
        self.hint.set_position(x, y);
    }
}

impl Button for DismissButton {
    fn hint(&self) -> &Hint {
        &self.hint
    }

    fn click(&self) {
        if let Some(tx) = self.tx.as_ref() {
            _ = tx.send(self.id);
        }
    }

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
    use super::DismissButton;
    use crate::{
        button::{Button, Hint},
        config::Config,
        notification_manager::UiState,
        text::Text,
    };
    use glyphon::FontSystem;
    use std::{cell::RefCell, rc::Rc};

    #[test]
    fn test_dismiss_button() {
        let config = Rc::new(Config::default());
        let ui_state = Rc::new(RefCell::new(UiState::default()));
        let hint = Hint::new(
            0,
            "",
            "".into(),
            Rc::clone(&config),
            &mut FontSystem::new(),
            Rc::clone(&ui_state),
        );

        let (tx, rx) = calloop::channel::channel();
        let test_id = 10;
        let button = DismissButton {
            id: test_id,
            app_name: "".into(),
            x: 0.,
            y: 0.,
            hint,
            text: Text::new(&config.styles.default.font, &mut FontSystem::new(), ""),
            state: crate::button::State::Hovered,
            config: Rc::clone(&config),
            ui_state: Rc::clone(&ui_state),
            tx: Some(tx),
        };

        button.click();

        match rx.try_recv() {
            Ok(id) => assert_eq!(id, test_id, "Button click should send button ID"),
            Err(_) => panic!("Button click did not send ID through channel"),
        }
    }
}
