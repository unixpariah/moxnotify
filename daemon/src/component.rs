use crate::{buffers, notification_manager::UiState, Urgency};
use glyphon::TextArea;

#[derive(Default)]
pub struct Bounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub trait Component {
    type Style;

    fn ui_state(&self) -> std::cell::Ref<'_, UiState>;

    fn style(&self) -> &Self::Style;

    fn instance(&self, urgency: &Urgency) -> buffers::Instance;

    fn text_area(&self, urgency: &Urgency) -> TextArea;

    fn bounds(&self) -> Bounds;

    fn render_bounds(&self) -> Bounds;

    fn set_position(&mut self, x: f32, y: f32);
}
