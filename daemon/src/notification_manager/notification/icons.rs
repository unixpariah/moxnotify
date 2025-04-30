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
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{LazyLock, Mutex},
};

static ICON_CACHE: LazyLock<Cache> = LazyLock::new(Cache::default);
type IconMap = BTreeMap<Box<Path>, ImageData>;
type ThemeMap = BTreeMap<Box<str>, IconMap>;

#[derive(Default)]
pub struct Cache(Mutex<ThemeMap>);

impl Cache {
    pub fn insert<P>(&self, theme: &str, icon_path: &P, data: ImageData)
    where
        P: AsRef<Path>,
    {
        let mut theme_map = self.0.lock().unwrap();
        let entry = icon_path.as_ref();

        match theme_map.get_mut(theme) {
            Some(theme_map) => {
                theme_map.insert(entry.into(), data);
            }
            None => {
                let mut icon_map = BTreeMap::new();
                icon_map.insert(entry.into(), data);
                theme_map.insert(theme.into(), icon_map);
            }
        }
    }

    pub fn get<P>(&self, theme: &str, icon_path: P) -> Option<ImageData>
    where
        P: AsRef<Path>,
    {
        let theme_map = self.0.lock().unwrap();

        theme_map
            .get(theme)
            .map(|icon_map| icon_map.get(icon_path.as_ref()))
            .and_then(|image_data| image_data.cloned())
    }
}

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
            Some(Image::Name(name)) => find_icon(
                name,
                config.general.icon_size as u16,
                config.general.theme.as_ref(),
            ),
            _ => None,
        };

        let app_icon = app_icon.as_ref().and_then(|icon| {
            find_icon(
                icon,
                config.general.icon_size as u16,
                config.general.theme.as_deref().as_ref(),
            )
        });

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

fn find_icon<T>(name: T, icon_size: u16, theme: Option<T>) -> Option<ImageData>
where
    T: AsRef<str>,
{
    let icon_path = freedesktop_icons::lookup(name.as_ref())
        .with_size(icon_size)
        .with_theme(theme.as_ref().map(AsRef::as_ref).unwrap_or("hicolor"))
        .with_cache()
        .find()?;

    get_icon(&icon_path, icon_size)
}

pub fn get_icon<T>(icon_path: T, icon_size: u16) -> Option<ImageData>
where
    T: AsRef<Path>,
{
    if let Some(icon) = ICON_CACHE.get("Cosmic", icon_path.as_ref()) {
        return Some(icon);
    }

    let image = if icon_path
        .as_ref()
        .extension()
        .is_some_and(|extension| extension == "svg")
    {
        let tree = {
            let opt = usvg::Options {
                resources_dir: Some(icon_path.as_ref().to_path_buf()),
                ..usvg::Options::default()
            };

            let svg_data = std::fs::read(icon_path.as_ref()).ok()?;
            usvg::Tree::from_data(&svg_data, &opt).ok()?
        };

        let mut pixmap = tiny_skia::Pixmap::new(icon_size as u32, icon_size as u32)?;

        let scale_x = icon_size as f32 / tree.size().width();
        let scale_y = icon_size as f32 / tree.size().height();

        resvg::render(
            &tree,
            tiny_skia::Transform::from_scale(scale_x, scale_y),
            &mut pixmap.as_mut(),
        );

        image::load_from_memory(&pixmap.encode_png().ok()?)
    } else {
        image::open(icon_path.as_ref())
    };

    let image_data = ImageData::try_from(image.ok()?);
    let image_data = image_data.ok().map(|i| i.into_rgba(icon_size as u32))?;
    ICON_CACHE.insert("Cosmic", &icon_path, image_data.clone());
    Some(image_data)
}
