use super::{progress::Progress, Extents};
use crate::{
    button::{ButtonManager, ButtonType},
    config::{Config, StyleState},
    image_data::ImageData,
    texture_renderer::{TextureArea, TextureBounds},
    Image,
};
use std::path::Path;

#[derive(Default)]
pub struct Icons {
    pub icon: Option<ImageData>,
    pub app_icon: Option<ImageData>,
    pub x: f32,
    pub y: f32,
}

impl Icons {
    pub fn new(image: Option<&Image>, app_icon: Option<&str>, config: &Config) -> Self {
        let icon = match image {
            Some(Image::Data(image_data)) => Some(image_data.clone().into_rgba(config.icon_size)),
            Some(Image::File(file)) => get_icon(file, config.icon_size as u16),
            Some(Image::Name(name)) => find_icon(name, config.icon_size as u16),
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

    pub fn set_position(
        &mut self,
        container_extents: &Extents,
        style: &StyleState,
        progress: &Option<Progress>,
        buttons: &ButtonManager,
        container_hovered: bool,
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
                .unwrap_or_default()
            - buttons
                .buttons()
                .iter()
                .filter_map(|button| {
                    if matches!(button.button_type, ButtonType::Action { .. }) {
                        Some(button.extents(container_hovered).height)
                    } else {
                        None
                    }
                })
                .max_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap_or_default();

        let vertical_offset = (available_height - icon_size) / 2.0;

        self.x = container_extents.x + style.border.size.left + style.padding.left;
        self.y = container_extents.y + style.border.size.top + style.padding.top + vertical_offset;
    }

    pub fn extents(&self, style: &StyleState) -> Extents {
        let (width, height) = self
            .icon
            .as_ref()
            .map(|i| (i.width as f32 + style.padding.right, i.height as f32))
            .unwrap_or((0., 0.));

        Extents {
            x: self.x,
            y: self.y,
            width,
            height,
        }
    }

    pub fn textures(
        &self,
        style: &StyleState,
        config: &Config,
        total_height: f32,
        scale: f32,
    ) -> Vec<TextureArea> {
        let mut texture_areas = Vec::new();

        let width = config.icon_size as f32;
        let height = config.icon_size as f32;

        let mut icon_extents = self.extents(style);

        if let Some(icon) = self.icon.as_ref() {
            let icon_size = config.icon_size as f32;
            let image_y = icon_extents.y + (height - icon_size) / 2.0;

            texture_areas.push(TextureArea {
                left: icon_extents.x,
                top: total_height - image_y - icon_size,
                width: icon_size,
                height: icon_size,
                scale,
                border_size: style.icon.border.size.into(),
                bounds: TextureBounds {
                    left: icon_extents.x as u32,
                    top: (total_height - icon_extents.y - height) as u32,
                    right: (icon_extents.x + width) as u32,
                    bottom: (total_height - icon_extents.y) as u32,
                },
                data: &icon.data,
                radius: style.icon.border.radius.into(),
            });

            icon_extents.x += (icon.height - config.app_icon_size) as f32;
            icon_extents.y += (icon.height as f32 / 2.) - config.app_icon_size as f32 / 2.;
        }

        if let Some(app_icon) = self.app_icon.as_ref() {
            let app_icon_size = config.app_icon_size as f32;
            let image_y = icon_extents.y + (height - app_icon_size) / 2.0;

            texture_areas.push(TextureArea {
                left: icon_extents.x,
                top: total_height - image_y - app_icon_size,
                width: app_icon_size,
                height: app_icon_size,
                scale,
                border_size: style.icon.border.size.into(),
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
}

fn find_icon(name: &str, icon_size: u16) -> Option<ImageData> {
    let icon_path = freedesktop_icons::lookup(name)
        .with_size(icon_size)
        .with_cache()
        .find()?;

    get_icon(&icon_path, icon_size)
}

pub fn get_icon(icon_path: &Path, icon_size: u16) -> Option<ImageData> {
    let image = image::open(icon_path).ok()?;
    let image_data = ImageData::try_from(image);
    image_data.ok().map(|i| i.into_rgba(icon_size as u32))
}
