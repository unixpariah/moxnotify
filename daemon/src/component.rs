use crate::{buffers, config::button::ButtonState, notification_manager::UiState, Urgency};
use glyphon::TextArea;

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
