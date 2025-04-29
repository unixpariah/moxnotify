use crate::{
    buffers,
    config::{Config, StyleState},
    notification_manager::UiState,
    Urgency,
};
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

    fn get_config(&self) -> &Config;

    fn get_app_name(&self) -> &str;

    fn get_id(&self) -> u32;

    fn get_ui_state(&self) -> std::cell::Ref<'_, UiState>;

    fn get_notification_style(&self) -> &StyleState {
        self.get_config().find_style(
            self.get_app_name(),
            self.get_ui_state().selected == Some(self.get_id()),
        )
    }

    fn get_style(&self) -> &Self::Style;

    fn get_instance(&self, urgency: &Urgency) -> buffers::Instance;

    fn get_text_area(&self, urgency: &Urgency) -> Option<TextArea>;

    fn get_bounds(&self) -> Bounds;

    fn get_render_bounds(&self) -> Bounds;

    fn set_position(&mut self, x: f32, y: f32);
}
