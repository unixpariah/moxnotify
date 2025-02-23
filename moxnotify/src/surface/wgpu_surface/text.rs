use crate::config::Font;
use glyphon::{
    Attrs, Buffer, Cache, Color, FontSystem, Shaping, Style, SwashCache, TextArea, TextAtlas,
    TextRenderer, Viewport, Weight,
};
use regex::Regex;
use std::sync::{Arc, LazyLock};
use wgpu::{MultisampleState, TextureFormat};

static REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<(/?)(b|i|a)\b[^>]*>").unwrap());
static HREF_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"href\s*=\s*["']([^"']*)["']"#).unwrap());

#[derive(Debug)]
pub struct Anchor {
    text: Arc<str>,
    pub href: Arc<str>,
    pub line: usize,
    pub start: usize,
    pub end: usize,
}

pub struct Text {
    pub buffer: Buffer,
    pub anchors: Vec<Anchor>,
}

impl Text {
    pub fn new(
        font: &Font,
        font_system: &mut FontSystem,
        summary: &str,
        body: &str,
        width: f32,
    ) -> Self {
        let attrs = Attrs::new().family(glyphon::Family::Name(&font.family));
        let mut spans = vec![];
        let mut anchors = Vec::new();
        let mut anchor_stack: Vec<Anchor> = Vec::new();

        if !summary.is_empty() {
            spans.push((summary, attrs.weight(Weight::BOLD)));
        }

        if !summary.is_empty() && !body.is_empty() {
            spans.push(("\n\n", attrs));
        }

        let mut start_pos = summary.len();
        if !body.is_empty() {
            let mut style_stack = Vec::new();
            let mut current_attrs = attrs;
            let mut last_pos = 0;

            REGEX.captures_iter(body).for_each(|cap| {
                let full_match = cap.get(0).unwrap();
                let is_closing = !cap[1].is_empty();
                let tag: Box<str> = cap[2].into();

                if full_match.start() > last_pos {
                    let text = &body[last_pos..full_match.start()];
                    start_pos += text.trim().chars().filter(|char| *char != '\n').count();
                    spans.push((text, current_attrs));
                }

                if is_closing {
                    if let Some(pos) = style_stack.iter().rposition(|t| *t == tag) {
                        style_stack.remove(pos);
                    }
                    if tag.as_ref() == "a" {
                        if let Some(mut anchor) = anchor_stack.pop() {
                            anchor.text = (&body[last_pos..full_match.start()]).into();
                            anchors.push(anchor);
                        }
                    }
                } else {
                    if tag.as_ref() == "a" {
                        if let Some(href_cap) = HREF_REGEX.captures(full_match.as_str()) {
                            let href = Arc::from(&href_cap[1]);
                            anchor_stack.push(Anchor {
                                text: "".into(),
                                href,
                                line: 0,
                                start: 0,
                                end: 0,
                            });
                        }
                    }
                    style_stack.push(tag);
                }

                current_attrs = attrs;
                style_stack.iter().for_each(|tag| {
                    current_attrs = match &**tag {
                        "b" => current_attrs.weight(Weight::BOLD),
                        "i" => current_attrs.style(Style::Italic),
                        "a" => current_attrs.color(Color::rgb(0, 0, 255)),
                        _ => current_attrs,
                    };
                });

                last_pos = full_match.end();
            });

            if last_pos < body.len() {
                let text = &body[last_pos..];
                spans.push((text, current_attrs));
            }
        }

        let dpi = 96.0;
        let font_size = font.size * dpi / 72.0;

        let mut buffer = Buffer::new(
            font_system,
            glyphon::Metrics::new(font_size, font_size * 1.2),
        );
        buffer.set_rich_text(font_system, spans.iter().copied(), attrs, Shaping::Advanced);
        buffer.shape_until_scroll(font_system, true);
        buffer.set_size(font_system, Some(width), None);

        let mut total = 0;
        anchors.iter_mut().for_each(|anchor| {
            buffer.lines.iter().enumerate().for_each(|(i, line)| {
                line.text()
                    .match_indices(&*anchor.text)
                    .for_each(|(start, text)| {
                        if total + start - text.len() == start_pos - 1 {
                            anchor.start = start - text.len() - 1;
                            anchor.end = start - text.len() + anchor.text.len();
                            anchor.line = i;
                        }
                    });
                total += line.text().len();
            });
        });

        Self { buffer, anchors }
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
    pub font_system: FontSystem,
}

impl TextContext {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, texture_format: TextureFormat) -> Self {
        let swash_cache = SwashCache::new();
        let cache = Cache::new(device);
        let mut atlas = TextAtlas::new(device, queue, &cache, texture_format);
        let renderer = TextRenderer::new(&mut atlas, device, MultisampleState::default(), None);

        Self {
            font_system: FontSystem::new(),
            swash_cache,
            viewport: Viewport::new(device, &cache),
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
