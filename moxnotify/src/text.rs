use crate::config::Font;
use glyphon::{
    Attrs, Buffer, Cache, FontSystem, Shaping, SwashCache, TextArea, TextAtlas, TextRenderer,
    Viewport, Weight,
};
use wgpu::{MultisampleState, TextureFormat};

pub struct Text(pub Buffer);

impl Text {
    pub fn new(
        font: &Font,
        font_system: &mut FontSystem,
        summary: &str,
        body: &str,
        max_width: f32,
    ) -> Self {
        let attrs = Attrs::new();
        attrs.family(glyphon::Family::Name(&font.family));

        let spans: &[(&str, Attrs)] = &[
            (summary, attrs.weight(Weight::BOLD)),
            ("\n", attrs),
            (body, attrs),
        ];

        // Scale the text to match it more with other apps
        let dpi = 96.0;
        let font_size = font.size * dpi / 72.0;

        let mut buffer = glyphon::Buffer::new(
            font_system,
            glyphon::Metrics::new(font_size, font_size * 1.2),
        );

        buffer.set_rich_text(font_system, spans.iter().copied(), attrs, Shaping::Advanced);
        buffer.shape_until_scroll(font_system, true);
        buffer.set_size(font_system, Some(max_width), None);

        Self(buffer)
    }

    pub fn buffer(&self) -> &Buffer {
        &self.0
    }

    pub fn extents(&self) -> (f32, f32) {
        let (width, total_lines) = self
            .0
            .layout_runs()
            .fold((0.0, 0.0), |(width, total_lines), run| {
                (run.line_w.max(width), total_lines + 1.0)
            });

        (width, total_lines * self.0.metrics().line_height)
    }
}

pub struct TextContext {
    pub swash_cache: glyphon::SwashCache,
    pub viewport: glyphon::Viewport,
    pub atlas: glyphon::TextAtlas,
    pub renderer: glyphon::TextRenderer,
    pub font_system: FontSystem,
}

impl TextContext {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_format: TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let swash_cache = SwashCache::new();
        let cache = Cache::new(device);
        let mut viewport = Viewport::new(device, &cache);
        let mut atlas = TextAtlas::new(device, queue, &cache, texture_format);
        let renderer = TextRenderer::new(&mut atlas, device, MultisampleState::default(), None);

        viewport.update(queue, glyphon::Resolution { width, height });

        Self {
            font_system: FontSystem::new(),
            swash_cache,
            viewport,
            atlas,
            renderer,
        }
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        render_pass: &mut wgpu::RenderPass,
        text: Vec<TextArea>,
    ) -> anyhow::Result<()> {
        self.renderer.prepare(
            device,
            queue,
            &mut self.font_system,
            &mut self.atlas,
            &self.viewport,
            text,
            &mut self.swash_cache,
        )?;

        self.renderer
            .render(&self.atlas, &self.viewport, render_pass)?;

        Ok(())
    }
}
