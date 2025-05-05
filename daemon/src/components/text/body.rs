use super::{
    markup::{Parser, Tag},
    Text,
};
use crate::{
    components::{notification::NotificationId, Bounds, Component, Data},
    config::{self, Config},
    manager::UiState,
    utils::buffers,
    Urgency,
};
use glyphon::{Attrs, Buffer, Color, FontSystem, Shaping, Style, Weight};
use std::{cell::RefCell, rc::Rc, sync::Arc};

#[derive(Debug)]
pub struct Anchor {
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
    pub buffer: Buffer,
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

        let mut anchors = Vec::new();

        let mut parser = Parser::new(text.as_ref().to_string());
        let body = parser.parse();
        let spans = body
            .iter()
            .map(|tag| match tag {
                Tag::Bold(text) => (text.as_str(), attrs.clone().weight(Weight::BOLD)),
                Tag::Italic(text) => (text.as_str(), attrs.clone().style(Style::Italic)),
                Tag::Underline(text) => (text.as_str(), attrs.clone()),
                Tag::Image { alt, src: _ } => (alt.as_str(), attrs.clone()),
                Tag::Anchor {
                    href,
                    text,
                    position,
                } => {
                    let anchor = Anchor {
                        href: href.as_str().into(),
                        line: position.line,
                        start: position.column,
                        end: position.column + text.len() - 1,
                        bounds: Bounds::default(),
                    };
                    anchors.push(anchor);
                    (text.as_str(), attrs.clone().color(Color::rgb(0, 0, 255)))
                }
                Tag::Text(text) => (text.as_str(), attrs.clone()),
            })
            .collect::<Vec<_>>();

        self.buffer
            .set_rich_text(font_system, spans, &attrs, Shaping::Advanced, None);

        anchors.iter_mut().for_each(|anchor| {
            if let Some(line) = self.buffer.layout_runs().nth(anchor.line) {
                let first = line.glyphs.get(anchor.start);
                let last = line.glyphs.get(anchor.end);

                if let (Some(first), Some(last)) = (first, last) {
                    anchor.bounds = Bounds {
                        x: first.x,
                        y: line.line_top,
                        width: last.x + last.w,
                        height: line.line_height,
                    };
                }
            };
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
