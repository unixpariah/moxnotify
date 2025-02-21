use super::config::Config;
use crate::{
    config::{Size, StyleState},
    image_data::ImageData,
    surface::wgpu_surface::{
        buffers,
        text::Text,
        texture_renderer::{TextureArea, TextureBounds},
    },
    Hint, Image, NotificationData, Urgency,
};
use calloop::RegistrationToken;
use glyphon::{FontSystem, TextArea, TextBounds};
use std::sync::Arc;

#[derive(Debug)]
pub struct Extents {
    pub x: f32,
    pub width: f32,
    pub height: f32,
}

pub type NotificationId = u32;

pub struct Notification {
    pub id: NotificationId,
    pub app_name: Box<str>,
    pub text: Text,
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
    pub fn new(config: Arc<Config>, font_system: &mut FontSystem, data: NotificationData) -> Self {
        let mut icon = None;
        let mut urgency = None;

        data.hints.into_iter().for_each(|hint| match hint {
            Hint::Image(Image::Data(image_data)) => {
                icon = Some(image_data.into_rgba(config.max_icon_size));
            }
            Hint::Urgency(level) if urgency.is_none() => urgency = Some(level),
            _ => {}
        });

        let style = &config.styles.default;
        let icon_width = icon
            .as_ref()
            .map(|i| i.width as f32 + style.padding.right)
            .unwrap_or(0.);

        let text = Text::new(
            &config.styles.default.font,
            font_system,
            &data.summary,
            &data.body,
            config.styles.default.width - icon_width,
        );

        let notification_style_entry = config
            .notification
            .iter()
            .find(|entry| entry.app == data.app_name);

        let ignore_timeout = notification_style_entry
            .and_then(|entry| entry.ignore_timeout)
            .unwrap_or(config.ignore_timeout);

        let default_timeout = notification_style_entry
            .and_then(|entry| entry.default_timeout)
            .unwrap_or(config.default_timeout);

        let timeout = if ignore_timeout {
            (default_timeout > 0).then(|| (default_timeout as u64) * 1000)
        } else {
            match data.timeout {
                0 => None,
                -1 => (default_timeout > 0).then(|| (default_timeout as u64) * 1000),
                t if t > 0 => Some(t as u64),
                _ => None,
            }
        };

        Self {
            id: data.id,
            app_name: data.app_name,
            text,
            timeout,
            config,
            hovered: false,
            actions: data.actions,
            icon,
            urgency: urgency.unwrap_or_default(),
            registration_token: None,
        }
    }

    pub fn set_text(&mut self, summary: &str, body: &str, font_system: &mut FontSystem) {
        let style = self.style();

        let icon_extents = self.icon_extents();

        self.text = Text::new(
            &style.font,
            font_system,
            summary,
            body,
            style.width - icon_extents.0,
        )
    }

    pub fn height(&self) -> f32 {
        let style = self.style();

        let icon_extents = self.icon_extents();

        let min_height = match style.min_height {
            Size::Auto => 0.,
            Size::Value(value) => value,
        };
        let max_height = match style.max_height {
            Size::Auto => f32::INFINITY,
            Size::Value(value) => value,
        };
        match style.height {
            Size::Value(height) => height.clamp(min_height, height),
            Size::Auto => self
                .text
                .extents()
                .1
                .max(icon_extents.1)
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

    pub fn get_instance(&self, y: f32, scale: f32) -> buffers::Instance {
        let extents = self.rendered_extents();

        let style = self.style();

        let color = match self.urgency() {
            crate::Urgency::Low => &style.urgency_low,
            crate::Urgency::Normal => &style.urgency_normal,
            crate::Urgency::Critical => &style.urgency_critical,
        };

        buffers::Instance {
            rect_pos: [extents.x, y],
            rect_size: [
                extents.width - style.border.size * 2.0,
                extents.height - style.border.size * 2.0,
            ],
            rect_color: color.background.to_linear(),
            border_radius: style.border.radius.into(),
            border_size: style.border.size,
            border_color: color.border.into(),
            scale,
        }
    }

    pub fn extents(&self) -> Extents {
        let style = self.style();

        Extents {
            x: 0.,
            width: self.width()
                + style.border.size * 2.
                + style.padding.left
                + style.padding.right
                + style.margin.left
                + style.margin.right,
            height: self.height()
                + style.border.size * 2.
                + style.padding.top
                + style.padding.bottom
                + style.margin.top
                + style.margin.bottom,
        }
    }

    pub fn style(&self) -> &StyleState {
        let styles = self
            .config
            .notification
            .iter()
            .find(|n| n.app == self.app_name)
            .map(|c| &c.styles)
            .unwrap_or(&self.config.styles);

        if self.hovered {
            &styles.hover
        } else {
            &styles.default
        }
    }

    pub fn rendered_extents(&self) -> Extents {
        let extents = self.extents();
        let style = self.style();

        Extents {
            x: extents.x + style.margin.left,
            width: extents.width - style.margin.left - style.margin.right,
            height: extents.height - style.margin.top - style.margin.bottom,
        }
    }

    pub fn texture(&self, y: f32, total_height: f32, scale: f32) -> Option<TextureArea> {
        if let Some(image) = self.image() {
            let extents = self.rendered_extents();
            let style = self.style();

            let urgency = match self.urgency() {
                Urgency::Low => &style.urgency_low,
                Urgency::Normal => &style.urgency_normal,
                Urgency::Critical => &style.urgency_critical,
            };

            let x = extents.x + style.border.size + style.padding.left;
            let y = y + style.border.size + style.padding.top;
            let width =
                extents.width - 2.0 * style.border.size - style.padding.left - style.padding.right;
            let height =
                extents.height - 2.0 * style.border.size - style.padding.top - style.padding.bottom;

            let image_y = y + (height - image.height as f32) / 2.0;

            return Some(TextureArea {
                left: x,
                top: total_height - image_y - image.height as f32,
                width: image.width as f32,
                height: image.height as f32,
                scale,
                border_size: style.border.size,
                border_color: urgency.border.into(),
                bounds: TextureBounds {
                    left: x as u32,
                    top: (total_height - y - height) as u32,
                    right: (x + width) as u32,
                    bottom: (total_height - y) as u32,
                },
                data: &image.data,
                radius: style.icon.border.radius.into(),
            });
        }

        None
    }

    pub fn icon_extents(&self) -> (f32, f32) {
        let style = self.style();
        self.icon
            .as_ref()
            .map(|i| (i.width as f32 + style.padding.right, i.height as f32))
            .unwrap_or((0., 0.))
    }

    pub fn text_area(&self, y: f32, scale: f32) -> TextArea {
        let extents = self.rendered_extents();
        let (width, height) = self.text.extents();

        let style = self.style();

        let color = match self.urgency() {
            crate::Urgency::Low => &style.urgency_low,
            crate::Urgency::Normal => &style.urgency_normal,
            crate::Urgency::Critical => &style.urgency_critical,
        };

        let color = color.foreground.rgba;

        let icon_width_positioning = self
            .icon
            .as_ref()
            .map(|i| i.width as f32 + style.padding.left)
            .unwrap_or(0.);

        TextArea {
            buffer: &self.text.0,
            left: extents.x + style.border.size + style.padding.left + icon_width_positioning,
            top: y + style.border.size + style.padding.top,
            scale,
            bounds: TextBounds {
                left: (extents.x + style.border.size + style.padding.left + icon_width_positioning)
                    as i32,
                top: (y + style.border.size + style.padding.top) as i32,
                right: (extents.x
                    + style.border.size
                    + width
                    + style.padding.left
                    + icon_width_positioning) as i32,
                bottom: (y + style.border.size + height.min(self.height()) + style.padding.top)
                    as i32,
            },
            default_color: glyphon::Color::rgba(color[0], color[1], color[2], color[3]),
            custom_glyphs: &[],
        }
    }
}
