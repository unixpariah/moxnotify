use super::config::Config;
use crate::{
    config::Size, image_data::ImageData, text::Text, wgpu_state::buffers, Hint, Image,
    NotificationData, Urgency,
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
        let mut x = None;
        let mut y = None;
        let mut icon = None;
        let mut urgency = None;

        for hint in data.hints {
            match hint {
                Hint::X(val) if x.is_none() => x = Some(val as f32),
                Hint::Y(val) if y.is_none() => y = Some(val as f32),
                Hint::Image(Image::Data(image_data)) => {
                    icon = Some(image_data.into_rgba(config.max_icon_size));
                }
                Hint::Urgency(level) if urgency.is_none() => urgency = Some(level),
                _ => {}
            }
        }

        // Set default values where needed
        let x = x.unwrap_or(0.0);
        let y = y.unwrap_or(start_pos);
        let urgency = urgency.unwrap_or_default();

        // Prepare text content
        let text = Text::new(
            &config.styles.default.font,
            font_system,
            &data.summary,
            &data.body,
        );

        // Calculate timeout
        let timeout = if config.ignore_timeout {
            (config.default_timeout > 0).then(|| (config.default_timeout as u64) * 1000)
        } else {
            match data.timeout {
                0 => None,
                -1 => (config.default_timeout > 0).then(|| (config.default_timeout as u64) * 1000),
                t if t > 0 => Some(t as u64),
                _ => None,
            }
        };

        Self {
            id: data.id,
            app_name: data.app_name,
            x,
            y,
            text,
            timeout,
            config,
            hovered: false,
            actions: data.actions,
            icon,
            urgency,
            registration_token: None,
        }
    }

    pub fn set_text(&mut self, summary: &str, body: &str, font_system: &mut FontSystem) {
        let styles = if self.hovered {
            &self.config.styles.hover
        } else {
            &self.config.styles.default
        };

        self.text = Text::new(&styles.font, font_system, summary, body)
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

        let min_height = match styles.min_height {
            Size::Auto => 0.,
            Size::Value(value) => value,
        };
        let max_height = match styles.max_height {
            Size::Auto => f32::INFINITY,
            Size::Value(value) => value,
        };
        match styles.height {
            Size::Value(height) => height.clamp(min_height, height),
            Size::Auto => self
                .text
                .extents()
                .1
                .max(icon_size.1 as f32)
                .clamp(min_height, max_height),
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

    pub fn text_area(&self, scale: f32) -> TextArea {
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
