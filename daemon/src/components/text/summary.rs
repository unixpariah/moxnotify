use super::Text;
use crate::{
    components::{Bounds, Component},
    config::Font,
};
use glyphon::{
    Attrs, Buffer, Cache, FontSystem, Shaping, SwashCache, TextArea, TextAtlas, TextRenderer,
    Viewport, Weight,
};
use wgpu::{MultisampleState, TextureFormat};

fn create_buffer(font: &Font, font_system: &mut FontSystem, max_width: Option<f32>) -> Buffer {
    let dpi = 96.0;
    let font_size = font.size * dpi / 72.0;
    let mut buffer = Buffer::new(
        font_system,
        glyphon::Metrics::new(font_size, font_size * 1.2),
    );
    buffer.shape_until_scroll(font_system, true);
    buffer.set_size(font_system, max_width, None);
    buffer
}

pub struct Summary {
    pub buffer: Buffer,
    x: f32,
    y: f32,
}

impl Text for Summary {}

impl Component for Summary {
    type Style = Font;

    fn get_config(&self) -> &crate::config::Config {
        todo!()
    }

    fn get_app_name(&self) -> &str {
        todo!()
    }

    fn get_id(&self) -> u32 {
        todo!()
    }

    fn get_ui_state(&self) -> std::cell::Ref<'_, crate::manager::UiState> {
        todo!()
    }

    fn get_style(&self) -> &Self::Style {
        todo!()
    }

    fn get_instances(&self, _: &crate::Urgency) -> Vec<crate::utils::buffers::Instance> {
        todo!()
    }

    fn get_text_areas(&self, _: &crate::Urgency) -> Vec<glyphon::TextArea> {
        todo!()
    }

    fn get_textures(&self) -> Vec<crate::rendering::texture_renderer::TextureArea> {
        todo!()
    }

    fn get_bounds(&self) -> Bounds {
        let (width, total_lines) = self
            .buffer
            .layout_runs()
            .fold((0.0, 0.0), |(width, total_lines), run| {
                (run.line_w.max(width), total_lines + 1.0)
            });

        Bounds {
            x: self.x,
            y: self.y,
            width,
            height: total_lines * self.buffer.metrics().line_height,
        }
    }

    fn get_render_bounds(&self) -> crate::components::Bounds {
        todo!()
    }

    fn set_position(&mut self, _: f32, _: f32) {
        todo!()
    }

    fn get_data(&self, _: &crate::Urgency) -> Vec<crate::components::Data> {
        todo!()
    }
}

impl Summary {
    pub fn new<T>(font: &Font, font_system: &mut FontSystem, body: T) -> Self
    where
        T: AsRef<str>,
    {
        let attrs = Attrs::new()
            .family(glyphon::Family::Name(&font.family))
            .weight(Weight::BOLD);
        let mut buffer = create_buffer(font, font_system, None);
        buffer.set_text(font_system, body.as_ref(), &attrs, Shaping::Advanced);

        Self {
            buffer,
            x: 0.,
            y: 0.,
        }
    }

    pub fn extents(&self) -> (f32, f32) {
        let (width, total_lines) = self
            .buffer
            .layout_runs()
            .fold((0.0, 0.0), |(width, total_lines), run| {
                (run.line_w.max(width), total_lines + 1.0)
            });

        (width, total_lines * self.buffer.metrics().line_height)
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
        text: Vec<TextArea>,
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
