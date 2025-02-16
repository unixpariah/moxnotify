use super::config::Config;
use crate::{
    image_data::ImageData, text::Text, wgpu_state::buffers, Hint, Image, NotificationData, Urgency,
};
use calloop::RegistrationToken;
use glyphon::{FontSystem, TextArea, TextBounds};
use std::sync::Arc;

#[derive(Debug)]
pub struct Extents {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub type NotificationId = u32;

pub struct Notification {
    pub id: NotificationId,
    pub app_name: Box<str>,
    pub text: Text,
    pub x: f32,
    pub y: f32,
    pub timeout: Option<u64>,
    pub hovered: bool,
    pub config: Arc<Config>,
    pub actions: Box<[(Arc<str>, Arc<str>)]>,
    pub icon: Option<ImageData>,
    pub urgency: Urgency,
    pub registration_token: Option<RegistrationToken>,
}

impl PartialEq for Notification {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl Notification {
    pub fn new(
        config: Arc<Config>,
        start_pos: f32,
        font_system: &mut FontSystem,
        data: NotificationData,
    ) -> Self {
        let x = data
            .hints
            .iter()
            .filter_map(|hint| match hint {
                Hint::X(x) => Some(*x),
                _ => None,
            })
            .next()
            .unwrap_or(0) as f32;

        let y = data
            .hints
            .iter()
            .filter_map(|hint| match hint {
                Hint::Y(y) => Some(*y as f32),
                _ => None,
            })
            .next()
            .unwrap_or(start_pos);

        let mut iter = data.hints.into_iter();

        let icon = iter.find_map(|hint| match hint {
            Hint::Image(image) => match image {
                Image::Data(image_data) => Some(image_data.into_rgba(config.max_icon_size)),
                Image::Name(_) | Image::File(_) => None,
            },
            _ => None,
        });

        let text = Text::new(
            &config.styles.default.font,
            font_system,
            &data.summary,
            &data.body,
        );

        let timeout = match config.ignore_timeout {
            true => {
                if config.default_timeout > 0 {
                    Some(config.default_timeout as u64 * 1000)
                } else {
                    None
                }
            }
            false => match data.timeout {
                0 => None,
                -1 => {
                    if config.default_timeout > 0 {
                        Some(config.default_timeout as u64 * 1000)
                    } else {
                        None
                    }
                }
                timeout if timeout > 0 => Some(timeout as u64),
                _ => None,
            },
        };

        let urgency = iter
            .filter_map(|hint| match hint {
                Hint::Urgency(level) => Some(level),
                _ => None,
            })
            .next()
            .unwrap_or(crate::Urgency::Low);

        Self {
            urgency,
            icon,
            app_name: data.app_name,
            actions: data.actions,
            registration_token: None,
            timeout,
            config,
            hovered: false,
            text,
            id: data.id,
            x,
            y,
        }
    }

    pub fn height(&self) -> f32 {
        let styles = if self.hovered {
            &self.config.styles.hover
        } else {
            &self.config.styles.default
        };

        let icon_size = self
            .icon
            .as_ref()
            .map(|i| (i.width, i.height))
            .unwrap_or((0, 0));

        let max_height = styles.max_height.unwrap_or(f32::INFINITY);
        match styles.height {
            Some(height) => height.clamp(styles.min_height, max_height),
            None => self
                .text
                .extents()
                .1
                .max(icon_size.1 as f32)
                .clamp(styles.min_height, max_height),
        }
    }

    pub fn width(&self) -> f32 {
        match self.hovered() {
            true => self.config.styles.hover.width,
            false => self.config.styles.default.width,
        }
    }

    pub fn image(&self) -> Option<&ImageData> {
        self.icon.as_ref()
    }

    pub fn urgency(&self) -> &Urgency {
        &self.urgency
    }

    pub fn hovered(&self) -> bool {
        self.hovered
    }

    pub fn hover(&mut self) {
        self.hovered = true;
    }

    pub fn unhover(&mut self) {
        self.hovered = false;
    }

    pub fn id(&self) -> NotificationId {
        self.id
    }

    pub fn get_instance(&self, scale: f32) -> buffers::Instance {
        let extents = self.rendered_extents();

        let styles = if self.hovered {
            &self.config.styles.hover
        } else {
            &self.config.styles.default
        };

        let color = match self.urgency() {
            crate::Urgency::Low => &styles.urgency_low,
            crate::Urgency::Normal => &styles.urgency_normal,
            crate::Urgency::Critical => &styles.urgency_critical,
        };

        buffers::Instance {
            rect_pos: [extents.x, extents.y],
            rect_size: [
                extents.width - styles.border.size * 2.0,
                extents.height - styles.border.size * 2.0,
            ],
            rect_color: color.background.into(),
            border_radius: styles.border.radius.into(),
            border_size: styles.border.size,
            border_color: color.border.into(),
            scale,
            rotation: 0.0,
        }
    }

    pub fn extents(&self) -> Extents {
        let styles = if self.hovered {
            &self.config.styles.hover
        } else {
            &self.config.styles.default
        };

        Extents {
            x: self.x,
            y: self.y,
            width: self.width()
                + styles.border.size * 2.
                + styles.padding.left
                + styles.padding.right
                + styles.margin.left
                + styles.margin.right,
            height: self.height()
                + styles.border.size * 2.
                + styles.padding.top
                + styles.padding.bottom
                + styles.margin.top
                + styles.margin.bottom,
        }
    }

    pub fn rendered_extents(&self) -> Extents {
        let extents = self.extents();
        let styles = if self.hovered {
            &self.config.styles.hover
        } else {
            &self.config.styles.default
        };

        Extents {
            x: extents.x + styles.margin.left,
            y: extents.y + styles.margin.top,
            width: extents.width - styles.margin.left - styles.margin.right,
            height: extents.height - styles.margin.top - styles.margin.bottom,
        }
    }

    pub fn contains_coordinates(&self, x: f64, y: f64) -> bool {
        let extents = self.rendered_extents();
        x > extents.x as f64
            && x < (extents.x + extents.width) as f64
            && y > (extents.y) as f64
            && y < (extents.y + extents.height) as f64
    }

    pub fn change_spot(&mut self, new_y: f32) {
        self.y = new_y;
    }

    pub fn text_area(&mut self, font_system: &mut FontSystem, scale: f32) -> TextArea {
        let extents = self.rendered_extents();
        let (width, height) = self.text.extents();

        let styles = if self.hovered {
            &self.config.styles.hover
        } else {
            &self.config.styles.default
        };

        let color = match self.urgency() {
            crate::Urgency::Low => &styles.urgency_low,
            crate::Urgency::Normal => &styles.urgency_normal,
            crate::Urgency::Critical => &styles.urgency_critical,
        };

        let color = color.foreground.rgba;

        let icon_width_layout = self.icon.as_ref().map(|i| i.width as f32).unwrap_or(0.);

        self.text
            .0
            .set_size(font_system, Some(self.width() - icon_width_layout), None);

        let icon_width_positioning = self
            .icon
            .as_ref()
            .map(|i| i.width as f32 + styles.padding.right)
            .unwrap_or(0.);

        TextArea {
            buffer: &self.text.0,
            left: extents.x + styles.border.size + styles.padding.left + icon_width_positioning,
            top: extents.y + styles.border.size + styles.padding.top,
            scale,
            bounds: TextBounds {
                left: (extents.x
                    + styles.border.size
                    + styles.padding.left
                    + icon_width_positioning) as i32,
                top: (extents.y + styles.border.size + styles.padding.top) as i32,
                right: (extents.x
                    + styles.border.size
                    + width
                    + styles.padding.left
                    + icon_width_positioning) as i32,
                bottom: (extents.y
                    + styles.border.size
                    + height.min(self.height())
                    + styles.padding.top) as i32,
            },
            default_color: glyphon::Color::rgba(color[0], color[1], color[2], color[3]),
            custom_glyphs: &[],
        }
    }
}
