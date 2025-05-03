use std::{cell::RefCell, rc::Rc, sync::Arc};

use super::Text;
use crate::{
    components::{notification::NotificationId, Bounds, Component, Data},
    config::{self, Config},
    manager::UiState,
    utils::buffers,
    Urgency,
};
use glyphon::{
    Attrs, Buffer, Cache, FontSystem, Shaping, SwashCache, TextAtlas, TextRenderer, Viewport,
    Weight,
};
use wgpu::{MultisampleState, TextureFormat};

pub struct Summary {
    id: NotificationId,
    app_name: Arc<str>,
    ui_state: Rc<RefCell<UiState>>,
    config: Rc<Config>,
    pub buffer: Buffer,
    x: f32,
    y: f32,
}

impl Text for Summary {
    fn set_text<T>(&mut self, font_system: &mut FontSystem, text: T)
    where
        T: AsRef<str>,
    {
        let style = &self.get_style();
        let family = Rc::clone(&style.family);

        let attrs = Attrs::new()
            .family(glyphon::Family::Name(&family))
            .weight(Weight::BOLD);

        self.buffer.set_text(
            font_system,
            text.as_ref(),
            &attrs,
            glyphon::Shaping::Advanced,
        );
    }
}

impl Component for Summary {
    type Style = config::text::Summary;

    fn get_config(&self) -> &crate::config::Config {
        &self.config
    }

    fn get_app_name(&self) -> &str {
        &self.app_name
    }

    fn get_id(&self) -> u32 {
        self.id
    }

    fn get_ui_state(&self) -> std::cell::Ref<'_, crate::manager::UiState> {
        self.ui_state.borrow()
    }

    fn get_style(&self) -> &Self::Style {
        &self.get_notification_style().summary
    }

    fn get_instances(&self, urgency: &Urgency) -> Vec<buffers::Instance> {
        let style = self.get_style();
        let bounds = self.get_render_bounds();

        vec![buffers::Instance {
            rect_pos: [bounds.x, bounds.y],
            rect_size: [bounds.width, bounds.height],
            rect_color: style.background.to_linear(urgency),
            border_radius: style.border.radius.into(),
            border_size: style.border.size.into(),
            border_color: style.border.color.to_linear(urgency),
            scale: self.ui_state.borrow().scale,
        }]
    }

    fn get_text_areas(&self, urgency: &crate::Urgency) -> Vec<glyphon::TextArea> {
        let style = self.get_style();
        let render_bounds = self.get_render_bounds();

        let content_width = render_bounds.width
            - style.border.size.left
            - style.border.size.right
            - style.padding.left
            - style.padding.right;

        let content_height = render_bounds.height
            - style.border.size.top
            - style.border.size.bottom
            - style.padding.top
            - style.padding.bottom;

        let left = render_bounds.x + style.border.size.left + style.padding.left;
        let top = render_bounds.y + style.border.size.top + style.padding.top;

        vec![glyphon::TextArea {
            buffer: &self.buffer,
            left,
            top,
            scale: self.ui_state.borrow().scale,
            bounds: glyphon::TextBounds {
                left: left as i32,
                top: top as i32,
                right: (left + content_width) as i32,
                bottom: (top + content_height) as i32,
            },
            default_color: style.color.into_glyphon(urgency),
            custom_glyphs: &[],
        }]
    }

    fn get_textures(&self) -> Vec<crate::rendering::texture_renderer::TextureArea> {
        Vec::new()
    }

    fn get_bounds(&self) -> Bounds {
        let style = self.get_style();
        let (width, total_lines) = self
            .buffer
            .layout_runs()
            .fold((0.0, 0.0), |(width, total_lines), run| {
                (run.line_w.max(width), total_lines + 1.0)
            });

        Bounds {
            x: self.x,
            y: self.y,
            width: width
                + style.margin.left
                + style.margin.right
                + style.padding.left
                + style.padding.right
                + style.border.size.left
                + style.border.size.right,
            height: total_lines * self.buffer.metrics().line_height
                + style.margin.top
                + style.margin.bottom
                + style.padding.top
                + style.padding.bottom
                + style.border.size.top
                + style.border.size.bottom,
        }
    }

    fn get_render_bounds(&self) -> Bounds {
        let style = self.get_style();
        let bounds = self.get_bounds();
        Bounds {
            x: bounds.x + style.margin.left,
            y: bounds.y + style.margin.top,
            width: bounds.width - style.margin.left - style.margin.right,
            height: bounds.height - style.margin.top - style.margin.bottom,
        }
    }

    fn set_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
    }

    fn get_data(&self, urgency: &Urgency) -> Vec<Data> {
        self.get_instances(urgency)
            .into_iter()
            .map(Data::Instance)
            .chain(self.get_text_areas(urgency).into_iter().map(Data::TextArea))
            .collect()
    }
}

impl Summary {
    pub fn new<T>(
        id: NotificationId,
        config: Rc<Config>,
        app_name: Arc<str>,
        ui_state: Rc<RefCell<UiState>>,
        font_system: &mut FontSystem,
        body: T,
    ) -> Self
    where
        T: AsRef<str>,
    {
        let attrs = Attrs::new()
            .family(glyphon::Family::Name(&config.styles.default.font.family))
            .weight(Weight::BOLD);

        let dpi = 96.0;
        let font_size = config.styles.default.font.size * dpi / 72.0;
        let mut buffer = Buffer::new(
            font_system,
            glyphon::Metrics::new(font_size, font_size * 1.2),
        );
        buffer.shape_until_scroll(font_system, true);
        buffer.set_size(font_system, None, None);
        buffer.set_text(font_system, body.as_ref(), &attrs, Shaping::Advanced);

        Self {
            id,
            buffer,
            x: 0.,
            y: 0.,
            config,
            ui_state,
            app_name,
        }
    }
}

pub struct TextContext {
    pub swash_cache: glyphon::SwashCache,
    pub viewport: glyphon::Viewport,
    pub atlas: glyphon::TextAtlas,
    pub renderer: glyphon::TextRenderer,
}

impl TextContext {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, texture_format: TextureFormat) -> Self {
        let swash_cache = SwashCache::new();
        let cache = Cache::new(device);
        let mut atlas = TextAtlas::new(device, queue, &cache, texture_format);
        let renderer = TextRenderer::new(&mut atlas, device, MultisampleState::default(), None);

        Self {
            swash_cache,
            viewport: Viewport::new(device, &cache),
            atlas,
            renderer,
        }
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        text: Vec<glyphon::TextArea>,
        font_system: &mut FontSystem,
    ) -> anyhow::Result<()> {
        if text.is_empty() {
            return Ok(());
        }

        self.renderer.prepare(
            device,
            queue,
            font_system,
            &mut self.atlas,
            &self.viewport,
            text,
            &mut self.swash_cache,
        )?;

        Ok(())
    }

    pub fn render(&mut self, render_pass: &mut wgpu::RenderPass) -> anyhow::Result<()> {
        self.renderer
            .render(&self.atlas, &self.viewport, render_pass)?;

        Ok(())
    }
}
