use crate::{
    config::Font,
    notification_manager::notification::{icons::get_icon, Extents},
};
use glyphon::{
    Attrs, Buffer, Cache, Color, FontSystem, Shaping, Style, SwashCache, TextArea, TextAtlas,
    TextRenderer, Viewport, Weight,
};
use regex::Regex;
use std::{
    path::Path,
    rc::Rc,
    sync::{Arc, LazyLock},
};
use wgpu::{MultisampleState, TextureFormat};

static REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<(/?)(b|i|a|u|img)\b[^>]*>").unwrap());
static HREF_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"href\s*=\s*["']([^"']*)["']"#).unwrap());
static ALT_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"alt\s*=\s*["']([^"']*)["']"#).unwrap());
static URL_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(https?://|ftp://|www\.)\S+\b").unwrap());
static SPLIT_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(<[^>]+>)|([^<]+)").unwrap());

#[derive(Debug)]
pub struct Anchor {
    text: Rc<str>,
    pub href: Arc<str>,
    pub line: usize,
    pub start: usize,
    pub end: usize,
    pub extents: Extents,
}

impl Anchor {
    pub fn extents(&self) -> Extents {
        Extents { ..self.extents }
    }
}

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

pub struct Text {
    pub buffer: Buffer,
    pub anchors: Vec<Rc<Anchor>>,
    x: f32,
    y: f32,
}

impl Text {
    pub fn new<T>(font: &Font, font_system: &mut FontSystem, body: T) -> Self
    where
        T: AsRef<str>,
    {
        let attrs = Attrs::new()
            .family(glyphon::Family::Name(&font.family))
            .weight(Weight::BOLD);
        let mut buffer = create_buffer(font, font_system, None);
        buffer.set_text(font_system, body.as_ref(), &attrs, Shaping::Basic);

        Self {
            buffer,
            anchors: Vec::new(),
            x: 0.,
            y: 0.,
        }
    }

    pub fn new_notification<T>(
        font: &Font,
        font_system: &mut FontSystem,
        summary: T,
        mut body: String,
        max_width: f32,
    ) -> Self
    where
        T: AsRef<str>,
    {
        let attrs = Attrs::new().family(glyphon::Family::Name(&font.family));
        let mut spans = vec![];
        let mut anchors = Vec::new();
        let mut anchor_stack: Vec<Anchor> = Vec::new();

        if !summary.as_ref().is_empty() {
            spans.push((summary.as_ref(), attrs.clone().weight(Weight::BOLD)));
        }

        if !summary.as_ref().is_empty() && !body.is_empty() {
            spans.push(("\n\n", attrs.clone()));
        }

        let mut start_pos = summary.as_ref().len();
        if !body.is_empty() {
            let mut style_stack = Vec::new();
            let mut current_attrs = attrs.clone();
            let mut last_pos = 0;

            body = SPLIT_REGEX
                .replace_all(&body, |caps: &regex::Captures| {
                    if let Some(tag) = caps.get(1) {
                        tag.as_str().to_string()
                    } else if let Some(text) = caps.get(2) {
                        URL_REGEX
                            .replace_all(text.as_str(), |url_caps: &regex::Captures| {
                                format!("<a href=\"{}\">{}</a>", &url_caps[0], &url_caps[0])
                            })
                            .to_string()
                    } else {
                        String::new()
                    }
                })
                .into_owned();

            REGEX.captures_iter(&body).for_each(|cap| {
                let full_match = cap.get(0).unwrap();
                let is_closing = !cap[1].is_empty();
                let tag: Box<str> = cap[2].into();

                if full_match.start() > last_pos {
                    let text = &body[last_pos..full_match.start()];
                    start_pos += text.chars().filter(|char| *char != '\n').count();
                    spans.push((text, current_attrs.clone()));
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
                    match tag.as_ref() {
                        "a" => {
                            if let Some(href_cap) = HREF_REGEX.captures(full_match.as_str()) {
                                let href = Arc::from(&href_cap[1]);
                                anchor_stack.push(Anchor {
                                    text: "".into(),
                                    href,
                                    line: 0,
                                    start: start_pos,
                                    end: 0,
                                    extents: Extents::default(),
                                });
                            }
                        }
                        "img" => {
                            if let Some(alt_cap) = ALT_REGEX.captures(full_match.as_str()) {
                                if let Some(href_cap) = HREF_REGEX.captures(full_match.as_str()) {
                                    let href = &href_cap[1];

                                    if let Some(image) = get_icon(Path::new(&href), 64) {
                                        _ = image;
                                    } else if let Some(alt) = alt_cap.get(1) {
                                        spans.push((alt.into(), current_attrs.clone()));
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                    style_stack.push(tag);
                }

                current_attrs = attrs.clone();
                style_stack.iter().for_each(|tag| {
                    current_attrs = match &**tag {
                        "b" => current_attrs.clone().weight(Weight::BOLD),
                        "i" => current_attrs.clone().style(Style::Italic),
                        "a" => current_attrs.clone().color(Color::rgb(0, 0, 255)),
                        "u" => current_attrs.clone(), // TODO: implement this once cosmic text implements
                        // underline
                        _ => current_attrs.clone(),
                    };
                });

                last_pos = full_match.end();
            });

            if last_pos < body.len() {
                let text = &body[last_pos..];
                spans.push((text, current_attrs));
            }
        }

        let mut buffer = create_buffer(font, font_system, Some(max_width));
        buffer.set_rich_text(font_system, spans, &attrs, Shaping::Basic, None);

        let mut total = 0;
        anchors.iter_mut().for_each(|anchor| {
            total = 0;
            buffer.lines.iter().enumerate().for_each(|(i, line)| {
                line.text()
                    .match_indices(&*anchor.text)
                    .for_each(|(start, _)| {
                        if total + start == anchor.start {
                            anchor.start = start;
                            anchor.end = start + anchor.text.len();
                            anchor.line = i;
                            anchor.extents = match buffer.layout_runs().nth(anchor.line) {
                                Some(line) => {
                                    let first = line.glyphs.get(anchor.start);
                                    let last = line.glyphs.get(anchor.end.saturating_sub(1));
                                    match (first, last) {
                                        (Some(first), Some(last)) => Extents {
                                            x: first.x + first.w,
                                            y: line.line_top + line.line_height,
                                            width: (last.x + last.w) - first.x,
                                            height: line.line_height,
                                        },
                                        _ => Extents::default(),
                                    }
                                }
                                None => Extents::default(),
                            };
                        }
                    });
                total += line.text().len();
            });
        });

        Self {
            buffer,
            anchors: anchors.into_iter().map(Rc::new).collect(),
            x: 0.,
            y: 0.,
        }
    }

    pub fn set_buffer_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
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

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        text: Vec<TextArea>,
    ) -> anyhow::Result<()> {
        if text.is_empty() {
            return Ok(());
        }

        self.renderer.prepare(
            device,
            queue,
            &mut self.font_system,
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
