use crate::{
    buffers,
    component::{Bounds, Component},
    config::{self, border::BorderRadius, Config, Insets, Size},
    notification_manager::UiState,
    Urgency,
};
use std::{cell::RefCell, rc::Rc, sync::Arc};

pub struct Progress {
    id: u32,
    app_name: Arc<str>,
    ui_state: Rc<RefCell<UiState>>,
    config: Rc<Config>,
    value: i32,
    x: f32,
    y: f32,
    width: f32,
}

impl Component for Progress {
    type Style = config::Progress;

    fn get_config(&self) -> &Config {
        &self.config
    }

    fn get_app_name(&self) -> &str {
        &self.app_name
    }

    fn get_id(&self) -> u32 {
        self.id
    }

    fn get_ui_state(&self) -> std::cell::Ref<'_, UiState> {
        self.ui_state.borrow()
    }

    fn get_style(&self) -> &Self::Style {
        &self.get_notification_style().progress
    }

    fn get_bounds(&self) -> Bounds {
        let style = self.config.find_style(
            &self.app_name,
            self.ui_state.borrow().selected == Some(self.id),
        );

        let element_width = style.progress.width.resolve(self.width);
        let remaining_space = self.width - element_width;

        let (resolved_ml, _) = match (
            style.progress.margin.left.is_auto(),
            style.progress.margin.right.is_auto(),
        ) {
            (true, true) => {
                let margin = remaining_space / 2.0;
                (margin, margin)
            }
            (true, false) => {
                let mr = style.progress.margin.right.resolve(0.);
                (remaining_space, mr)
            }
            _ => (
                style.progress.margin.left.resolve(0.),
                style.progress.margin.right.resolve(0.),
            ),
        };

        let x_position = self.x + resolved_ml;

        Bounds {
            x: x_position,
            y: self.y,
            width: element_width,
            height: style.progress.height
                + style.progress.margin.top
                + style.progress.margin.bottom,
        }
    }

    fn set_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
    }

    fn get_render_bounds(&self) -> Bounds {
        let bounds = self.get_bounds();

        let style = self.config.find_style(
            &self.app_name,
            self.ui_state.borrow().selected == Some(self.id),
        );

        let remaining_space = self.width - bounds.width;
        let (margin_left, _) = match (
            style.progress.margin.left.is_auto(),
            style.progress.margin.right.is_auto(),
        ) {
            (true, true) => {
                let margin = remaining_space / 2.0;
                (margin, margin)
            }
            (true, false) => {
                let mr = style.progress.margin.right.resolve(0.);
                (remaining_space, mr)
            }
            _ => (
                style.progress.margin.left.resolve(0.),
                style.progress.margin.right.resolve(0.),
            ),
        };

        Bounds {
            x: bounds.x + margin_left,
            y: bounds.y + style.progress.margin.top,
            width: bounds.width - margin_left - style.progress.margin.right,
            height: bounds.height - style.progress.margin.top - style.progress.margin.bottom,
        }
    }

    fn get_text_area(&self, _: &Urgency) -> Option<glyphon::TextArea> {
        None
    }

    fn get_instance(&self, urgency: &Urgency) -> buffers::Instance {
        let style = self.config.find_style(
            &self.app_name,
            self.ui_state.borrow().selected == Some(self.id),
        );

        let extents = self.get_render_bounds();

        let progress_ratio = (self.value as f32 / 100.0).min(1.0);

        let complete_width = (extents.width * progress_ratio).max(0.);

        let border_size = if self.value < 100 {
            Insets {
                right: Size::Value(0.),
                ..style.progress.border.size
            }
        } else {
            style.progress.border.size
        };

        let border_radius = if self.value < 100 {
            BorderRadius {
                top_right: 0.0,
                bottom_right: 0.0,
                ..style.progress.border.radius
            }
        } else {
            style.progress.border.radius
        };

        buffers::Instance {
            rect_pos: [extents.x, extents.y],
            rect_size: [complete_width, extents.height],
            rect_color: style.progress.complete_color.to_linear(urgency),
            border_radius: border_radius.into(),
            border_size: border_size.into(),
            border_color: style.progress.border.color.to_linear(urgency),
            scale: self.ui_state.borrow().scale,
        }
    }
}

impl Progress {
    pub fn new(
        id: u32,
        value: i32,
        ui_state: Rc<RefCell<UiState>>,
        config: Rc<Config>,
        app_name: Arc<str>,
    ) -> Self {
        Self {
            id,
            app_name,
            config,
            ui_state,
            value,
            x: 0.,
            y: 0.,
            width: 0.,
        }
    }

    pub fn set_width(&mut self, width: f32) {
        self.width = width;
    }

    pub fn instances(&self, urgency: &Urgency) -> Vec<buffers::Instance> {
        let style = self.config.find_style(
            &self.app_name,
            self.ui_state.borrow().selected == Some(self.id),
        );

        let extents = self.get_render_bounds();

        let progress_ratio = (self.value as f32 / 100.0).min(1.0);

        let mut instances = Vec::new();
        let complete_width = (extents.width * progress_ratio).max(0.);

        if complete_width > 0.0 {
            instances.push(self.get_instance(urgency));
        }

        if self.value < 100 {
            let incomplete_width = extents.width - complete_width;

            if incomplete_width > 0.0 {
                let border_size = if self.value > 0 {
                    Insets {
                        left: Size::Value(0.),
                        ..style.progress.border.size
                    }
                } else {
                    style.progress.border.size
                };

                let border_radius = if self.value > 0 {
                    BorderRadius {
                        top_left: 0.0,
                        bottom_left: 0.0,
                        ..style.progress.border.radius
                    }
                } else {
                    style.progress.border.radius
                };

                instances.push(buffers::Instance {
                    rect_pos: [extents.x + complete_width, extents.y],
                    rect_size: [incomplete_width, extents.height],
                    rect_color: style.progress.incomplete_color.to_linear(urgency),
                    border_radius: border_radius.into(),
                    border_size: border_size.into(),
                    border_color: style.progress.border.color.to_linear(urgency),
                    scale: self.ui_state.borrow().scale,
                });
            }
        }

        instances
    }
}
