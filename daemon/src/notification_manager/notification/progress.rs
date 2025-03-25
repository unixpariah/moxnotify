use super::Extents;
use crate::{
    buffers,
    config::{border::BorderRadius, Insets, StyleState},
    Urgency,
};

#[derive(Clone, Copy)]
pub struct Progress {
    value: i32,
    x: f32,
    y: f32,
}

impl Progress {
    pub fn new(value: i32) -> Self {
        Self {
            value,
            x: 0.,
            y: 0.,
        }
    }

    pub fn set_position(&mut self, container_extents: &Extents, style: &StyleState) {
        let extents = self.extents(container_extents, style);

        self.x = (container_extents.x + container_extents.width
            - style.padding.left
            - style.padding.right
            - style.border.size.left
            - style.border.size.right)
            / 2.
            - extents.width / 2.
            + style.padding.left
            + style.border.size.left;
        self.y = container_extents.y + container_extents.height
            - style.border.size.bottom
            - style.padding.bottom
            - extents.height
    }

    pub fn extents(&self, container_extents: &Extents, style: &StyleState) -> Extents {
        let width = container_extents.width
            - style.border.size.left
            - style.border.size.right
            - style.padding.left
            - style.padding.right
            - style.progress.margin.left
            - style.progress.margin.right;

        Extents {
            x: self.x,
            y: self.y,
            width: style.progress.width.resolve(width),
            height: style.progress.height.resolve(0.)
                + style.progress.margin.top
                + style.progress.margin.bottom,
        }
    }

    pub fn rendered_extents(&self, container_extents: &Extents, style: &StyleState) -> Extents {
        let extents = self.extents(container_extents, style);

        Extents {
            x: extents.x + style.progress.margin.left,
            y: extents.y + style.progress.margin.top,
            width: extents.width - style.progress.margin.left - style.progress.margin.right,
            height: extents.height - style.progress.margin.top - style.progress.margin.bottom,
        }
    }

    pub fn instances(
        &self,
        urgency: &Urgency,
        notification_extents: &Extents,
        style: &StyleState,
        scale: f32,
    ) -> Vec<buffers::Instance> {
        let extents = self.rendered_extents(notification_extents, style);

        let progress_ratio = (self.value as f32 / 100.0).min(1.0);

        let mut instances = Vec::new();
        let complete_width = (extents.width * progress_ratio).max(0.);

        if complete_width > 0.0 {
            let border_size = if self.value < 100 {
                Insets {
                    right: 0.,
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

            instances.push(buffers::Instance {
                rect_pos: [extents.x, extents.y],
                rect_size: [complete_width, extents.height],
                rect_color: style.progress.complete_color.to_linear(urgency),
                border_radius: border_radius.into(),
                border_size: border_size.into(),
                border_color: style.progress.border.color.to_linear(urgency),
                scale,
            });
        }

        if self.value < 100 {
            let incomplete_width = extents.width - complete_width;

            if incomplete_width > 0.0 {
                let border_size = if self.value > 0 {
                    Insets {
                        left: 0.,
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
                    scale,
                });
            }
        }

        instances
    }
}
