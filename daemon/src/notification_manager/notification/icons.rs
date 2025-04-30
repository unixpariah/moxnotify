use super::{progress::Progress, Extents};
use crate::{
    button::{ButtonManager, ButtonType, Finished},
    component::Component,
    config::{Config, StyleState},
    image_data::ImageData,
    texture_renderer::{TextureArea, TextureBounds},
    Image,
};
use resvg::usvg;
use std::path::Path;
use tiny_skia::Size;

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
            Some(Image::Data(image_data)) => {
                Some(image_data.clone().into_rgba(config.general.icon_size))
            }
            Some(Image::File(file)) => get_icon(file, config.general.icon_size as u16),
            Some(Image::Name(name)) => find_icon(name, config.general.icon_size as u16),
            _ => None,
        };

        let app_icon = app_icon
            .as_ref()
            .and_then(|icon| find_icon(icon, config.general.icon_size as u16));

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
        buttons: &ButtonManager<Finished>,
    ) {
        let icon_size = 64.0;

        let available_height = container_extents.height
            - style.border.size.top
            - style.border.size.bottom
            - style.padding.top
            - style.padding.bottom
            - progress
                .as_ref()
                .map(|p| p.get_bounds().height)
                .unwrap_or_default()
            - buttons
                .buttons()
                .iter()
                .filter_map(|button| {
                    if button.button_type() == ButtonType::Action {
                        Some(button.get_bounds().height)
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

        let width = config.general.icon_size as f32;
        let height = config.general.icon_size as f32;

        let mut icon_extents = self.extents(style);

        if let Some(icon) = self.icon.as_ref() {
            let icon_size = config.general.icon_size as f32;
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

            icon_extents.x += (icon.height - config.general.app_icon_size) as f32;
            icon_extents.y += (icon.height as f32 / 2.) - config.general.app_icon_size as f32 / 2.;
        }

        if let Some(app_icon) = self.app_icon.as_ref() {
            let app_icon_size = config.general.app_icon_size as f32;
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

fn find_icon<T>(name: T, icon_size: u16) -> Option<ImageData>
where
    T: AsRef<str>,
{
    let icon_path = freedesktop_icons::lookup(name.as_ref())
        .with_size(icon_size)
        .with_theme(&freedesktop_icons::default_theme_gtk().unwrap_or("hicolor".to_string()))
        .with_cache()
        .find()?;

    get_icon(&icon_path, icon_size)
}

pub fn get_icon<T>(icon_path: T, icon_size: u16) -> Option<ImageData>
where
    T: AsRef<Path>,
{
    let image = if icon_path
        .as_ref()
        .extension()
        .is_some_and(|extension| extension == "svg")
    {
        let tree = {
            let mut opt = usvg::Options {
                resources_dir: Some(icon_path.as_ref().to_path_buf()),
                default_size: Size::from_wh(icon_size as f32, icon_size as f32)?,
                ..usvg::Options::default()
            };
            opt.fontdb_mut().load_system_fonts();

            let svg_data = std::fs::read(icon_path.as_ref()).unwrap();
            usvg::Tree::from_data(&svg_data, &opt).unwrap()
        };

        let pixmap_size = tree.size().to_int_size();
        let mut pixmap = tiny_skia::Pixmap::new(pixmap_size.width(), pixmap_size.height()).unwrap();
        resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

        image::load_from_memory(&pixmap.encode_png().ok()?)
    } else {
        image::open(icon_path)
    };

    let image_data = ImageData::try_from(image.ok()?);
    image_data.ok().map(|i| i.into_rgba(icon_size as u32))
}
