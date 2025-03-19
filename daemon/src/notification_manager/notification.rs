use super::config::Config;
use crate::{
    buffers,
    button::{Action, Button, ButtonManager, ButtonType},
    config::{border::BorderRadius, Insets, Size, StyleState},
    image_data::ImageData,
    text,
    texture_renderer::{TextureArea, TextureBounds},
    Hint, Image, NotificationData, Urgency,
};
use calloop::RegistrationToken;
use glyphon::{FontSystem, TextArea, TextBounds};
use std::path::Path;
use std::sync::Arc;

#[derive(Debug)]
pub struct Extents {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy)]
pub struct Progress {
    value: i32,
    x: f32,
    y: f32,
}

impl Progress {
    fn new(value: i32) -> Self {
        Self {
            value,
            x: 0.,
            y: 0.,
        }
    }

    pub fn set_position(&mut self, container_extents: &Extents, style: &StyleState) {
        self.x = container_extents.x + style.border.size.left + style.padding.left;
        self.y = container_extents.y + container_extents.height
            - style.border.size.bottom
            - style.padding.bottom
            - style.progress.height;
    }

    fn extents(&self, container_extents: &Extents, style: &StyleState) -> Extents {
        let width = container_extents.width
            - style.border.size.left
            - style.border.size.right
            - style.padding.left
            - style.padding.right;

        Extents {
            x: self.x,
            y: self.y,
            width,
            height: style.progress.height,
        }
    }

    fn instances(
        &self,
        urgency: &Urgency,
        notification_extents: &Extents,
        style: &StyleState,
        scale: f32,
    ) -> Vec<buffers::Instance> {
        let extents = self.extents(notification_extents, style);

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

pub type NotificationId = u32;

pub struct Notification {
    id: NotificationId,
    y: f32,
    app_name: Box<str>,
    pub text: text::Text,
    timeout: Option<u64>,
    hovered: bool,
    config: Arc<Config>,
    actions: Box<[(Arc<str>, Arc<str>)]>,
    icons: Icons,
    urgency: Urgency,
    progress: Option<Progress>,
    pub registration_token: Option<RegistrationToken>,
    pub buttons: ButtonManager,
}

impl PartialEq for Notification {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

pub struct Icons {
    icon: Option<ImageData>,
    app_icon: Option<ImageData>,
    x: f32,
    y: f32,
}

impl Icons {
    fn new(image: Option<Image>, app_icon: Option<Box<str>>, config: &Config) -> Self {
        let icon = match image {
            Some(Image::Data(image_data)) => Some(image_data.into_rgba(config.icon_size)),
            Some(Image::File(file)) => get_icon(&file, config.icon_size as u16),
            Some(Image::Name(name)) => find_icon(&name, config.icon_size as u16),
            _ => None,
        };

        let app_icon = app_icon
            .as_ref()
            .and_then(|icon| find_icon(icon, config.icon_size as u16));

        let (final_app_icon, final_icon) = match icon.is_some() {
            true => (app_icon, icon),
            false => (None, app_icon),
        };

        Self {
            icon: final_icon,
            app_icon: final_app_icon,
            x: 0.,
            y: 0.,
        }
    }

    fn set_position(
        &mut self,
        container_extents: &Extents,
        style: &StyleState,
        progress: &Option<Progress>,
    ) {
        let icon_size = 64.0;

        let available_height = container_extents.height
            - style.border.size.top
            - style.border.size.bottom
            - style.padding.top
            - style.padding.bottom
            - progress
                .as_ref()
                .map(|p| p.extents(container_extents, style).height)
                .unwrap_or_default();

        let vertical_offset = (available_height - icon_size) / 2.0;

        self.x = container_extents.x + style.border.size.left + style.padding.left;
        self.y = container_extents.y + style.border.size.top + style.padding.top + vertical_offset;
    }

    fn extents(&self) -> Extents {
        Extents {
            x: self.x,
            y: self.y,
            width: 0.,
            height: 0.,
        }
    }
}

impl Notification {
    pub fn new(config: Arc<Config>, font_system: &mut FontSystem, data: NotificationData) -> Self {
        let mut urgency = None;
        let mut progress = None;

        data.hints.iter().for_each(|hint| match hint {
            Hint::Urgency(level) if urgency.is_none() => urgency = Some(*level),
            Hint::Value(value) => progress = Some(Progress::new(*value)),
            _ => {}
        });

        let icon = data.hints.into_iter().find_map(|hint| {
            if let Hint::Image(image) = hint {
                Some(image)
            } else {
                None
            }
        });

        let icons = Icons::new(icon, data.app_icon, &config);

        let style = &config.styles.default;
        let mut buttons = ButtonManager::default();
        let dismiss_button = Button::new(
            Action::DismissNotification,
            ButtonType::Dismiss,
            Arc::clone(&config),
            font_system,
        );

        let icon_width = icons
            .icon
            .as_ref()
            .map(|i| i.width as f32 + style.padding.right)
            .unwrap_or(0.);
        let text = text::Text::new_notification(
            &config.styles.default.font,
            font_system,
            &data.summary,
            &data.body,
            config.styles.default.width - icon_width - dismiss_button.extents(false).width,
        );

        buttons.push(dismiss_button);

        let notification_style_entry = config
            .notification
            .iter()
            .find(|entry| entry.app == data.app_name);

        let ignore_timeout = notification_style_entry
            .and_then(|entry| entry.ignore_timeout)
            .unwrap_or(config.ignore_timeout);

        let default_timeout = notification_style_entry
            .and_then(|entry| entry.default_timeout.as_ref())
            .unwrap_or(&config.default_timeout);

        let urgency = urgency.unwrap_or_default();

        let timeout = if ignore_timeout {
            (default_timeout.get(&urgency) > 0)
                .then(|| (default_timeout.get(&urgency) as u64) * 1000)
        } else {
            match data.timeout {
                0 => None,
                -1 => (default_timeout.get(&urgency) > 0)
                    .then(|| (default_timeout.get(&urgency) as u64) * 1000),
                t if t > 0 => Some(t as u64),
                _ => None,
            }
        };

        Self {
            progress,
            y: 0.,
            icons,
            buttons,
            id: data.id,
            app_name: data.app_name,
            text,
            timeout,
            config,
            hovered: false,
            actions: data.actions,
            urgency,
            registration_token: None,
        }
    }

    pub fn set_y(&mut self, y: f32) {
        self.y = y;
        let extents = self.rendered_extents();
        let style = self.config.find_style(&self.app_name, self.hovered());

        if let Some(progress) = self.progress.as_mut() {
            progress.set_position(&extents, style);
        }

        self.icons.set_position(&extents, style, &self.progress);

        let extents = self.rendered_extents();
        let hovered = self.hovered();
        self.buttons.iter_mut().for_each(|button| {
            let (x, y) = match button.button_type {
                ButtonType::Action => todo!(),
                ButtonType::Dismiss => (
                    extents.x + extents.width
                        - style.border.size.right
                        - style.padding.right
                        - button.extents(hovered).width,
                    y + style.margin.top + style.border.size.top + style.padding.top,
                ),
            };

            button.set_position(x, y);
        });
    }

    pub fn timeout(&self) -> Option<u64> {
        self.timeout
    }

    fn text_extents(&self) -> Extents {
        let style = self.style();
        let icon_extents = self.icon_extents();

        let dismiss_button = self
            .buttons
            .iter()
            .find(|button| button.button_type == ButtonType::Dismiss);

        Extents {
            x: style.padding.left + style.border.size.left + style.margin.left + icon_extents.0,
            y: style.margin.top + style.border.size.top,
            width: style.width
                - icon_extents.0
                - dismiss_button
                    .map(|b| b.extents(self.hovered()).width)
                    .unwrap_or_default(),
            height: 0.,
        }
    }

    pub fn set_text(&mut self, summary: &str, body: &str, font_system: &mut FontSystem) {
        let style = self.style();
        let text_extents = self.text_extents();

        self.text = text::Text::new_notification(
            &style.font,
            font_system,
            summary,
            body,
            text_extents.width,
        );
    }

    pub fn height(&self) -> f32 {
        let style = self.style();

        let dismiss_button = self
            .buttons
            .iter()
            .find(|button| button.button_type == ButtonType::Dismiss)
            .map(|b| b.extents(self.hovered()).height)
            .unwrap_or(0.0);

        let progress = if self.progress.is_some() {
            style.progress.height + style.padding.top + style.padding.bottom
        } else {
            0.0
        };

        let min_height = match style.min_height {
            Size::Auto => 0.0,
            Size::Value(value) => value,
        };

        let max_height = match style.max_height {
            Size::Auto => f32::INFINITY,
            Size::Value(value) => value,
        };

        match style.height {
            Size::Value(height) => height.clamp(min_height, max_height),
            Size::Auto => {
                let text_height = self.text.extents().1 + progress;
                let icon_height = self.icon_extents().1 + progress;
                let base_height = text_height.max(icon_height).max(dismiss_button);
                base_height.clamp(min_height, max_height)
            }
        }
    }

    pub fn width(&self) -> f32 {
        match self.hovered() {
            true => self.config.styles.hover.width,
            false => self.config.styles.default.width,
        }
    }

    pub fn image(&self) -> (Option<&ImageData>, Option<&ImageData>) {
        (self.icons.icon.as_ref(), self.icons.app_icon.as_ref())
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
            style.padding.left + style.border.size.left + style.margin.left + icon_width,
            style.margin.top + style.border.size.top,
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

    pub fn background_instance(&self, scale: f32) -> buffers::Instance {
        let extents = self.rendered_extents();
        let style = self.style();

        buffers::Instance {
            rect_pos: [extents.x, extents.y],
            rect_size: [
                extents.width - style.border.size.left - style.border.size.right,
                extents.height - style.border.size.top - style.border.size.bottom,
            ],
            rect_color: style.background.to_linear(self.urgency()),
            border_radius: style.border.radius.into(),
            border_size: style.border.size.into(),
            border_color: style.border.color.to_linear(self.urgency()),
            scale,
        }
    }

    pub fn get_instance(&self, scale: f32) -> Vec<buffers::Instance> {
        let mut instances = vec![self.background_instance(scale)];
        if let Some(progress) = self.progress.as_ref() {
            instances.extend_from_slice(&progress.instances(
                self.urgency(),
                &self.rendered_extents(),
                self.style(),
                scale,
            ));
        }

        let button_instances: Box<[buffers::Instance]> = self
            .buttons
            .iter()
            .map(|button| button.get_instance(self.hovered(), scale, &self.urgency))
            .collect();

        instances.extend_from_slice(&button_instances);

        instances
    }

    pub fn extents(&self) -> Extents {
        let style = self.style();

        Extents {
            x: 0.,
            y: self.y,
            width: self.width()
                + style.border.size.left
                + style.border.size.right
                + style.padding.left
                + style.padding.right
                + style.margin.left
                + style.margin.right,
            height: self.height()
                + style.border.size.top
                + style.border.size.bottom
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
            y: extents.y + style.margin.top,
            width: extents.width - style.margin.left - style.margin.right,
            height: extents.height - style.margin.top - style.margin.bottom,
        }
    }

    pub fn textures(&self, total_height: f32, scale: f32) -> Vec<TextureArea> {
        let mut texture_areas = Vec::new();
        let (icon, app_icon) = self.image();

        let style = self.style();

        let width = self.config.icon_size as f32;
        let height = self.config.icon_size as f32;

        let mut icon_extents = self.icons.extents();

        if let Some(icon) = icon.as_ref() {
            let icon_size = self.config.icon_size as f32;
            let image_y = icon_extents.y + (height - icon_size) / 2.0;

            texture_areas.push(TextureArea {
                left: icon_extents.x,
                top: total_height - image_y - icon_size,
                width: icon_size,
                height: icon_size,
                scale,
                border_size: style.icon.border.size.top, // TODO: make it use each of the edges
                bounds: TextureBounds {
                    left: icon_extents.x as u32,
                    top: (total_height - icon_extents.y - height) as u32,
                    right: (icon_extents.x + width) as u32,
                    bottom: (total_height - icon_extents.y) as u32,
                },
                data: &icon.data,
                radius: style.icon.border.radius.into(),
            });

            icon_extents.x += (icon.height - self.config.app_icon_size) as f32;
            icon_extents.y += (icon.height as f32 / 2.) - self.config.app_icon_size as f32 / 2.;
        }

        if let Some(app_icon) = app_icon.as_ref() {
            let app_icon_size = self.config.app_icon_size as f32;
            let image_y = icon_extents.y + (height - app_icon_size) / 2.0;

            texture_areas.push(TextureArea {
                left: icon_extents.x,
                top: total_height - image_y - app_icon_size,
                width: app_icon_size,
                height: app_icon_size,
                scale,
                border_size: style.icon.border.size.top, // TODO: make it use each of the edges
                bounds: TextureBounds {
                    left: icon_extents.x as u32,
                    top: (total_height - icon_extents.y - height) as u32,
                    right: (icon_extents.x + width) as u32,
                    bottom: (total_height - icon_extents.y) as u32,
                },
                data: &app_icon.data,
                radius: style.app_icon.border.radius.into(),
            });
        }

        texture_areas
    }

    pub fn icon_extents(&self) -> (f32, f32) {
        let style = self.style();
        self.icons
            .icon
            .as_ref()
            .map(|i| (i.width as f32 + style.padding.right, i.height as f32))
            .unwrap_or((0., 0.))
    }

    pub fn text_area(&self, scale: f32) -> Vec<TextArea> {
        let extents = self.rendered_extents();
        let (width, height) = self.text.extents();

        let style = self.style();

        let icon_width_positioning = self
            .icons
            .icon
            .as_ref()
            .map(|i| i.width as f32 + style.padding.left)
            .unwrap_or(0.);

        let mut res = vec![TextArea {
            buffer: &self.text.buffer,
            left: extents.x + style.border.size.left + style.padding.left + icon_width_positioning,
            top: extents.y + style.border.size.top + style.padding.top,
            scale,
            bounds: TextBounds {
                left: (extents.x
                    + style.border.size.left
                    + style.padding.left
                    + icon_width_positioning) as i32,
                top: (extents.y + style.border.size.top + style.padding.top) as i32,
                right: (extents.x
                    + style.border.size.left
                    + width
                    + style.padding.left
                    + icon_width_positioning) as i32,
                bottom: (extents.y
                    + style.border.size.top
                    + height.min(self.height())
                    + style.padding.top) as i32,
            },
            default_color: style.font.color.into_glyphon(self.urgency()),
            custom_glyphs: &[],
        }];

        let button_areas: Box<[TextArea]> = self
            .buttons
            .iter()
            .map(|button| button.text_area(self.urgency(), self.hovered(), scale))
            .collect();

        res.extend_from_slice(&button_areas);
        res
    }
}
