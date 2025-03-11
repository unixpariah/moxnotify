use super::config::Config;
use crate::button::{Action, Button, ButtonManager, ButtonType};
use crate::config::BorderRadius;
use crate::text;
use crate::{
    buffers,
    config::{Size, StyleState},
    image_data::ImageData,
    texture_renderer::{TextureArea, TextureBounds},
    Hint, Image, NotificationData, Urgency,
};
use calloop::RegistrationToken;
use glyphon::{FontSystem, TextArea, TextBounds};
use std::path::Path;
use std::sync::{Arc, LazyLock};

#[derive(Debug)]
pub struct Extents {
    pub x: f32,
    pub width: f32,
    pub height: f32,
}

pub type NotificationId = u32;

pub struct Notification {
    id: NotificationId,
    app_name: Box<str>,
    pub text: text::Text,
    timeout: Option<u64>,
    hovered: bool,
    config: Arc<Config>,
    actions: Box<[(Arc<str>, Arc<str>)]>,
    image: Option<ImageData>,
    app_icon: Option<ImageData>,
    urgency: Urgency,
    value: Option<i32>,
    pub registration_token: Option<RegistrationToken>,
    pub buttons: ButtonManager,
}

impl PartialEq for Notification {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

fn svg_to_rgba(file: &Path, max_icon_size: u32) -> Option<ImageData> {
    let svg_data = std::fs::read_to_string(file).ok()?;

    let mut options = usvg::Options::default();
    options.fontdb_mut().load_system_fonts();

    let tree = usvg::Tree::from_str(&svg_data, &options).ok()?;

    let (width, height) = {
        let size = tree.size();
        let ratio = size.width() / size.height();
        if size.width() > size.height() {
            (max_icon_size, (max_icon_size as f32 / ratio) as u32)
        } else {
            ((max_icon_size as f32 * ratio) as u32, max_icon_size)
        }
    };

    let mut pixmap = tiny_skia::Pixmap::new(width, height)?;
    resvg::render(
        &tree,
        tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );

    let rgba_image = image::RgbaImage::from_raw(width, height, pixmap.data().to_vec())?;

    ImageData::try_from(image::DynamicImage::ImageRgba8(rgba_image))
        .ok()
        .map(|d| d.into_rgba(max_icon_size))
}

fn find_icon(name: &str, icon_size: u16) -> Option<ImageData> {
    let icon_path = freedesktop_icons::lookup(name)
        .with_size(icon_size)
        .find()?;

    get_icon(&icon_path, icon_size)
}

pub fn get_icon(icon_path: &Path, icon_size: u16) -> Option<ImageData> {
    if icon_path.extension().and_then(|s| s.to_str()) == Some("svg") {
        svg_to_rgba(icon_path, icon_size as u32)
    } else {
        let image = image::open(icon_path).ok()?;
        let image_data = ImageData::try_from(image);
        image_data.ok().map(|i| i.into_rgba(icon_size as u32))
    }
}

impl Notification {
    pub fn new(config: Arc<Config>, font_system: &mut FontSystem, data: NotificationData) -> Self {
        let mut icon = None;
        let mut urgency = None;
        let mut value = None;

        data.hints.into_iter().for_each(|hint| match hint {
            Hint::Image(image) => {
                icon = match image {
                    Image::Data(image_data) => Some(image_data.into_rgba(config.icon_size)),
                    Image::File(file) => get_icon(&file, config.icon_size as u16),
                    Image::Name(name) => find_icon(&name, config.icon_size as u16),
                }
            }
            Hint::Urgency(level) if urgency.is_none() => urgency = Some(level),
            Hint::Value(val) => value = Some(val),
            _ => {}
        });

        let app_icon_option = find_icon(&data.app_icon, config.icon_size as u16);

        let final_app_icon = if icon.is_some() {
            app_icon_option
        } else {
            None
        };
        let final_image = if icon.is_some() {
            icon
        } else {
            find_icon(&data.app_icon, config.icon_size as u16)
        };

        let style = &config.styles.default;

        let mut buttons = ButtonManager::default();
        let dismiss_button = Button::new(
            style.border.size + style.width - style.padding.right - style.padding.left,
            style.border.size + style.padding.top,
            Action::DismissNotification,
            ButtonType::Dismiss,
            Arc::clone(&config),
            font_system,
        );

        let icon_width = final_image
            .as_ref()
            .map(|i| i.width as f32 + style.padding.right)
            .unwrap_or(0.);
        let text = text::Text::new_notification(
            &config.styles.default.font,
            font_system,
            &data.summary,
            &data.body,
            config.styles.default.width - icon_width - dismiss_button.extents().width,
            style.padding.left + style.border.size + style.margin.left + icon_width,
            style.margin.top + style.border.size,
        );

        buttons.push(dismiss_button.into());

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
            value,
            app_icon: final_app_icon,
            image: final_image,
            buttons,
            id: data.id,
            app_name: data.app_name,
            text,
            timeout,
            config,
            hovered: false,
            actions: data.actions,
            urgency: urgency.unwrap_or_default(),
            registration_token: None,
        }
    }

    pub fn timeout(&self) -> Option<u64> {
        self.timeout
    }

    pub fn set_text(&mut self, summary: &str, body: &str, font_system: &mut FontSystem) {
        let style = self.style();

        let icon_extents = self.icon_extents();

        self.text = text::Text::new_notification(
            &style.font,
            font_system,
            summary,
            body,
            style.width - icon_extents.0,
            style.padding.left + style.border.size + style.margin.left + icon_extents.0,
            style.margin.top + style.border.size,
        )
    }

    pub fn height(&self) -> f32 {
        let style = self.style();

        let dismiss_button = self
            .buttons
            .iter()
            .find(|button| button.borrow().button_type == ButtonType::Dismiss)
            .map(|b| b.borrow().extents().height)
            .unwrap_or(0.);

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
                .max(self.icon_extents().1)
                .max(dismiss_button)
                .clamp(min_height, max_height),
        }
    }

    pub fn width(&self) -> f32 {
        match self.hovered() {
            true => self.config.styles.hover.width,
            false => self.config.styles.default.width,
        }
    }

    pub fn image(&self) -> (Option<&ImageData>, Option<&ImageData>) {
        (self.image.as_ref(), self.app_icon.as_ref())
    }

    pub fn urgency(&self) -> &Urgency {
        &self.urgency
    }

    pub fn hovered(&self) -> bool {
        self.hovered
    }

    fn update_text_position(&mut self) {
        let style = self.style();
        let icon_width = self.icon_extents().0;
        self.text.set_buffer_position(
            style.padding.left + style.border.size + style.margin.left + icon_width,
            style.margin.top + style.border.size,
        );
    }

    pub fn hover(&mut self) {
        self.hovered = true;
        self.update_text_position();
    }

    pub fn unhover(&mut self) {
        self.hovered = false;
        self.update_text_position();
    }

    pub fn id(&self) -> NotificationId {
        self.id
    }

    pub fn get_instance(&self, y: f32, scale: f32) -> Vec<buffers::Instance> {
        let extents = self.rendered_extents();

        let style = self.style();

        let color = match self.urgency() {
            crate::Urgency::Low => &style.urgency_low,
            crate::Urgency::Normal => &style.urgency_normal,
            crate::Urgency::Critical => &style.urgency_critical,
        };

        let mut instances = vec![buffers::Instance {
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
        }];

        if let Some(value) = self.value {
            instances.push(buffers::Instance {
                rect_pos: [extents.x + style.border.size + style.padding.left, y],
                rect_size: [
                    (extents.width
                        - style.border.size * 2.0
                        - style.padding.left
                        - style.padding.right)
                        * (value as f32 / 100.).min(1.),
                    self.style().progress.height,
                ],
                rect_color: style.progress.complete_color.into(),
                border_radius: style.border.radius.into(),
                border_size: 0.,
                border_color: color.border.into(),
                scale,
            });
        }

        self.buttons.iter().for_each(|button| {
            instances.push(button.borrow().get_instance(
                extents.x + style.padding.left,
                y,
                self.hovered(),
                scale,
            ))
        });

        instances
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

    pub fn texture<'a>(
        &'a self,
        x: f32,
        y: f32,
        texture_width: f32,
        texture_height: f32,
        total_height: f32,
        scale: f32,
        image_data: &'a [u8],
        border_radius: BorderRadius,
    ) -> TextureArea<'a> {
        let extents = self.rendered_extents();
        let style = self.style();

        let x = extents.x + style.border.size + style.padding.left + x;
        let y = y + style.border.size + style.padding.top;
        let width =
            extents.width - 2.0 * style.border.size - style.padding.left - style.padding.right;
        let height =
            extents.height - 2.0 * style.border.size - style.padding.top - style.padding.bottom;

        let image_y = y + (height - texture_height) / 2.0;

        TextureArea {
            left: x,
            top: total_height - image_y - texture_height,
            width: texture_width,
            height: texture_height,
            scale,
            border_size: style.icon.border.size,
            bounds: TextureBounds {
                left: x as u32,
                top: (total_height - y - height) as u32,
                right: (x + width) as u32,
                bottom: (total_height - y) as u32,
            },
            data: image_data,
            radius: border_radius.into(),
        }
    }

    pub fn icon_extents(&self) -> (f32, f32) {
        let style = self.style();
        self.image
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

        let icon_width_positioning = self
            .image
            .as_ref()
            .map(|i| i.width as f32 + style.padding.left)
            .unwrap_or(0.);

        TextArea {
            buffer: &self.text.buffer,
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
            default_color: color.foreground.into(),
            custom_glyphs: &[],
        }
    }
}
