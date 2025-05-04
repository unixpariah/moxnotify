use super::Text;
use crate::{
    components::{notification::NotificationId, Bounds, Component, Data},
    config::{self, Config},
    manager::UiState,
    utils::buffers,
    Urgency,
};
use glyphon::{Attrs, Buffer, Color, FontSystem, Shaping, Style, Weight};
use regex::Regex;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, LazyLock},
};

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
    pub bounds: Bounds,
}

impl Anchor {
    pub fn bounds(&self) -> Bounds {
        Bounds { ..self.bounds }
    }
}

pub struct Body {
    id: NotificationId,
    app_name: Arc<str>,
    ui_state: Rc<RefCell<UiState>>,
    pub anchors: Vec<Rc<Anchor>>,
    config: Rc<Config>,
    buffer: Buffer,
    x: f32,
    y: f32,
}

impl Text for Body {
    fn set_size(&mut self, font_system: &mut FontSystem, width: Option<f32>, height: Option<f32>) {
        self.buffer.set_size(font_system, width, height);
    }

    fn set_text<T>(&mut self, font_system: &mut FontSystem, text: T)
    where
        T: AsRef<str>,
    {
        let family = Rc::clone(&self.get_style().family);

        let attrs = Attrs::new()
            .metadata(0.7_f32.to_bits() as usize)
            .family(glyphon::Family::Name(&family));
        let mut spans = Vec::new();
        let mut anchors = Vec::new();
        let mut anchor_stack: Vec<Anchor> = Vec::new();

        let mut start_pos = 0;
        let mut style_stack = Vec::new();
        let mut current_attrs = attrs.clone();
        let mut last_pos = 0;

        let body = SPLIT_REGEX
            .replace_all(text.as_ref(), |caps: &regex::Captures| {
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
                start_pos += text.len();
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
                                bounds: Bounds::default(),
                            });
                        }
                    }
                    "img" => {
                        if let Some(alt_cap) = ALT_REGEX.captures(full_match.as_str()) {
                            if HREF_REGEX.captures(full_match.as_str()).is_some() {
                                if let Some(alt) = alt_cap.get(1) {
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

        self.buffer
            .set_rich_text(font_system, spans, &attrs, Shaping::Advanced, None);

        anchors.iter_mut().for_each(|anchor| {
            let mut total_bytes = 0;

            for (line_idx, layout_run) in self.buffer.layout_runs().enumerate() {
                let line_text = &self.buffer.lines[line_idx].text();
                let line_start = total_bytes;
                let line_end = line_start + line_text.len();

                if anchor.start >= line_start && anchor.start < line_end {
                    let local_start = anchor.start - line_start;
                    let local_end = local_start + anchor.text.len();

                    if line_text.get(local_start..local_end) == Some(&*anchor.text) {
                        anchor.line = line_idx;

                        let mut first_glyph = None;
                        let mut last_glyph = None;

                        for glyph in layout_run.glyphs.iter() {
                            if glyph.start <= local_start && glyph.end > local_start {
                                first_glyph.get_or_insert(glyph);
                            }
                            if glyph.start < local_end && glyph.end >= local_end {
                                last_glyph = Some(glyph);
                                break;
                            }
                        }

                        if let (Some(first), Some(last)) = (first_glyph, last_glyph) {
                            anchor.bounds = Bounds {
                                x: first.x,
                                y: layout_run.line_top,
                                width: last.x + last.w - first.x,
                                height: layout_run.line_height,
                            };
                        }
                    }
                }
                total_bytes = line_end;
            }
        });

        self.anchors = anchors.into_iter().map(Rc::new).collect();
    }
}

impl Component for Body {
    type Style = config::text::Body;

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
        &self.get_notification_style().body
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
            depth: 0.8,
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

impl Body {
    pub fn new(
        id: NotificationId,
        config: Rc<Config>,
        app_name: Arc<str>,
        ui_state: Rc<RefCell<UiState>>,
        font_system: &mut FontSystem,
    ) -> Self {
        let dpi = 96.0;
        let font_size = config.styles.default.font.size * dpi / 72.0;
        let mut buffer = Buffer::new(
            font_system,
            glyphon::Metrics::new(font_size, font_size * 1.2),
        );
        buffer.shape_until_scroll(font_system, true);

        Self {
            id,
            buffer,
            x: 0.,
            y: 0.,
            config,
            ui_state,
            app_name,
            anchors: Vec::new(),
        }
    }
}
