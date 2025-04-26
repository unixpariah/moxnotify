use super::{Button, ButtonManager, ButtonType, Hint, State};
use crate::{
    buffers,
    component::{Bounds, Component},
    config::{button::ButtonState, Config},
    notification_manager::UiState,
    text::Text,
    Urgency,
};
use calloop::channel::Event;
use glyphon::FontSystem;
use std::{cell::RefCell, rc::Rc, sync::Arc};

pub struct ActionButton {
    id: u32,
    ui_state: Rc<RefCell<UiState>>,
    x: f32,
    y: f32,
    hint: Hint,
    config: Rc<Config>,
    text: Text,
    action: Arc<str>,
    state: State,
    pub width: f32,
    tx: calloop::channel::Sender<(u32, Arc<str>)>,
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

impl ButtonManager {
    pub fn add_actions(
        mut self,
        actions: &[(Arc<str>, Arc<str>)],
        font_system: &mut FontSystem,
    ) -> Self {
        if actions.is_empty() {
            return self;
        }

        let (tx, rx) = calloop::channel::channel();
        if let Some(loop_handle) = self.loop_handle.as_ref() {
            loop_handle
                .insert_source(rx, move |event, _, moxnotify| {
                    if let Event::Msg((id, action_key)) = event {
                        if let Some(surface) = moxnotify.surface.as_ref() {
                            let token = surface.token.as_ref().map(Arc::clone);
                            _ = moxnotify.emit_sender.send(crate::EmitEvent::ActionInvoked {
                                id,
                                action_key,
                                token: token.unwrap_or_default(),
                            });
                        }

                        if !moxnotify
                            .notifications
                            .notifications()
                            .iter()
                            .find(|notification| notification.id() == id)
                            .map(|n| n.data.hints.resident)
                            .unwrap_or_default()
                        {
                            moxnotify.dismiss_by_id(id, None);
                        }
                    }
                })
                .ok();
        }

        let mut buttons = actions
            .iter()
            .cloned()
            .map(|action| {
                let font = &self.config.styles.default.buttons.action.default.font;
                let text = Text::new(font, font_system, &action.0);

                Box::new(ActionButton {
                    id: self.id,
                    ui_state: Rc::clone(&self.ui_state),
                    hint: Hint::new(
                        "",
                        Rc::clone(&self.config),
                        font_system,
                        Rc::clone(&self.ui_state),
                    ),
                    text,
                    x: 0.,
                    y: 0.,
                    config: Rc::clone(&self.config),
                    action: action.0,
                    state: State::Unhovered,
                    width: 0.,
                    tx: tx.clone(),
                }) as Box<dyn Button<Style = ButtonState>>
            })
            .collect::<Vec<Box<dyn Button<Style = ButtonState>>>>();

        self.buttons.append(&mut buttons);

        self
    }
}
