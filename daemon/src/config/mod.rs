pub mod border;
pub mod button;
pub mod color;
pub mod keymaps;
pub mod partial;

use border::{Border, BorderRadius};
use button::{Button, ButtonState, Buttons};
use color::Color;
use keymaps::Keymaps;
use mlua::{Lua, LuaSerdeExt};
use partial::{PartialFont, PartialInsets, PartialStyle};
use serde::{Deserialize, Deserializer};
use std::{fmt, fs, path::PathBuf};

#[derive(Deserialize)]
#[serde(default)]
pub struct Config {
    pub scroll_sensitivity: f64,
    pub max_visible: u32,
    pub icon_size: u32,
    pub app_icon_size: u32,
    pub anchor: Anchor,
    pub layer: Layer,
    pub queue: Queue,
    pub output: Option<Box<str>>,
    pub default_timeout: Timeout,
    pub ignore_timeout: bool,
    pub styles: Styles,
    pub keymaps: Keymaps,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            scroll_sensitivity: 20.,
            max_visible: 5,
            icon_size: 64,
            app_icon_size: 24,
            anchor: Anchor::default(),
            layer: Layer::default(),
            queue: Queue::default(),
            output: None,
            default_timeout: Timeout::default(),
            ignore_timeout: false,
            keymaps: Keymaps::default(),

            styles: Styles::default(),
        }
    }
}

#[derive(Deserialize)]
pub struct Style {
    #[serde(deserialize_with = "deserialize_selectors")]
    pub selector: Vec<Selector>,
    #[serde(default)]
    pub state: State,
    pub style: PartialStyle,
}

fn deserialize_selectors<'de, D>(deserializer: D) -> Result<Vec<Selector>, D::Error>
where
    D: Deserializer<'de>,
{
    struct SelectorsVisitor;

    impl<'de> serde::de::Visitor<'de> for SelectorsVisitor {
        type Value = Vec<Selector>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string or a list of strings")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            let selector = Selector::deserialize(serde::de::value::StrDeserializer::new(value))?;
            Ok(vec![selector])
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            let mut selectors = Vec::new();
            while let Some(s) = seq.next_element::<String>()? {
                let selector = Selector::deserialize(serde::de::value::StrDeserializer::new(&s))?;
                selectors.push(selector);
            }
            Ok(selectors)
        }
    }

    deserializer.deserialize_any(SelectorsVisitor)
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    #[default]
    Default,
    Hover,
    ContainerHover,
}

pub enum Selector {
    All,
    PrevCounter,
    NextCounter,
    AllNotifications,
    Notification(Box<str>),
    ActionButton,
    DismissButton,
    Progress,
    Icon,
}

impl<'de> Deserialize<'de> for Selector {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "*" => Ok(Selector::All),
            "prev_counter" => Ok(Selector::PrevCounter),
            "next_counter" => Ok(Selector::NextCounter),
            "notification" => Ok(Selector::AllNotifications),
            "action" => Ok(Selector::ActionButton),
            "dismiss" => Ok(Selector::DismissButton),
            "progress" => Ok(Selector::Progress),
            "icon" => Ok(Selector::Icon),
            _ => {
                if let Some(notification) = s.strip_prefix("notification:") {
                    Ok(Selector::Notification(notification.into()))
                } else {
                    Err(serde::de::Error::unknown_variant(
                        &s,
                        &[
                            "*",
                            "prev_counter",
                            "next_counter",
                            "notification",
                            "notification:...",
                            "action",
                            "dismiss",
                        ],
                    ))
                }
            }
        }
    }
}

#[derive(Default, Clone, Copy)]
pub struct Insets {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

impl Insets {
    pub fn size(value: f32) -> Self {
        Self {
            left: value,
            right: value,
            top: value,
            bottom: value,
        }
    }

    fn apply(&mut self, partial: &PartialInsets) {
        if let Some(left) = partial.left {
            self.left = left;
        }
        if let Some(right) = partial.right {
            self.right = right;
        }
        if let Some(top) = partial.top {
            self.top = top;
        }
        if let Some(bottom) = partial.bottom {
            self.bottom = bottom;
        }
    }
}

impl From<Insets> for [f32; 4] {
    fn from(value: Insets) -> Self {
        [value.left, value.right, value.top, value.bottom]
    }
}

pub struct Font {
    pub size: f32,
    pub family: Box<str>,
    pub color: Color,
}

impl Font {
    fn apply(&mut self, partial: &PartialFont) {
        if let Some(size) = partial.size {
            self.size = size;
        }
        if let Some(family) = partial.family.clone() {
            self.family = family;
        }
        if let Some(color) = partial.color.as_ref() {
            self.color.apply(color);
        }
    }
}

impl Default for Font {
    fn default() -> Self {
        Self {
            size: 10.,
            family: "DejaVu Sans".into(),
            color: Color::rgba([255, 255, 255, 255]),
        }
    }
}

#[derive(Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Queue {
    #[default]
    Unordered,
    Ordered,
}

pub struct Icon {
    pub border: Border,
}

impl Icon {
    pub fn apply(&mut self, partial: &PartialStyle) {
        if let Some(border) = partial.border.as_ref() {
            self.border.apply(border);
        }
    }
}

impl Default for Icon {
    fn default() -> Self {
        Self {
            border: Border {
                color: Color::default(),
                size: Insets::size(0.),
                radius: BorderRadius::circle(),
            },
        }
    }
}

#[derive(Deserialize, Default, Debug, Clone, Copy)]
pub enum Size {
    #[default]
    #[serde(rename = "auto")]
    Auto,
    #[serde(untagged)]
    Value(f32),
}

impl Size {
    pub fn resolve(&self, auto: f32) -> f32 {
        match self {
            Size::Auto => auto,
            Size::Value(v) => *v,
        }
    }
}

pub struct Progress {
    pub margin: Insets,
    pub height: Size,
    pub width: Size,
    pub border: Border,
    pub incomplete_color: Color,
    pub complete_color: Color,
}

impl Progress {
    pub fn apply(&mut self, partial: &PartialStyle) {
        if let Some(background) = partial.background.as_ref() {
            self.complete_color.apply(background);
        }
        if let Some(margin) = partial.margin.as_ref() {
            self.margin.apply(margin);
        }
        if let Some(height) = partial.height.as_ref() {
            self.height = *height;
        }
        if let Some(width) = partial.width.as_ref() {
            self.width = *width;
        }
        if let Some(border) = partial.border.as_ref() {
            self.border.apply(border);
        }
    }
}

impl Default for Progress {
    fn default() -> Self {
        Self {
            margin: Insets {
                left: 0.,
                right: 0.,
                top: 10.,
                bottom: 0.,
            },
            height: Size::Value(20.),
            width: Size::Auto,
            border: Border {
                radius: BorderRadius {
                    top_left: 5.,
                    top_right: 5.,
                    bottom_left: 5.,
                    bottom_right: 5.,
                },
                ..Default::default()
            },
            incomplete_color: Color::default(),
            complete_color: Color {
                urgency_low: [247, 118, 142, 255],
                urgency_normal: [247, 118, 142, 255],
                urgency_critical: [247, 118, 142, 255],
            },
        }
    }
}

pub struct StyleState {
    pub background: Color,
    pub width: Size,
    pub min_height: Size,
    pub max_height: Size,
    pub height: Size,
    pub font: Font,
    pub border: Border,
    pub margin: Insets,
    pub padding: Insets,
    pub icon: Icon,
    pub app_icon: Icon,
    pub progress: Progress,
    pub buttons: Buttons,
}

impl StyleState {
    fn default_hover() -> Self {
        Self {
            background: Color::rgba([47, 53, 73, 255]),
            ..Default::default()
        }
    }

    pub fn apply(&mut self, partial: &PartialStyle) {
        if let Some(background) = partial.background.as_ref() {
            self.background.apply(background);
        }
        if let Some(width) = partial.width {
            self.width = width;
        }
        if let Some(min_height) = partial.min_height {
            self.min_height = min_height;
        }
        if let Some(max_height) = partial.max_height {
            self.max_height = max_height;
        }
        if let Some(height) = partial.height {
            self.height = height;
        }
        if let Some(partial_font) = partial.font.as_ref() {
            self.font.apply(partial_font);
        }
        if let Some(partial_border) = partial.border.as_ref() {
            self.border.apply(partial_border);
        }
        if let Some(partial_margin) = partial.margin.as_ref() {
            self.margin.apply(partial_margin);
        }
        if let Some(partial_padding) = partial.padding.as_ref() {
            self.padding.apply(partial_padding);
        }
    }
}

impl Default for StyleState {
    fn default() -> Self {
        Self {
            background: Color {
                urgency_low: [26, 27, 38, 255],
                urgency_normal: [22, 22, 30, 255],
                urgency_critical: [22, 22, 30, 255],
            },
            width: Size::Value(300.),
            min_height: Size::Auto,
            max_height: Size::Auto,
            height: Size::Auto,
            font: Font::default(),
            border: Border::default(),
            margin: Insets::size(5.),
            padding: Insets::size(10.),
            icon: Icon::default(),
            app_icon: Icon::default(),
            progress: Progress::default(),
            buttons: Buttons::default(),
        }
    }
}

pub struct Styles {
    pub next: NotificationCounter,
    pub prev: NotificationCounter,
    pub notification: Vec<NotificationStyleEntry>,
    pub default: StyleState,
    pub hover: StyleState,
}

impl<'de> Deserialize<'de> for Styles {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct TempStyles(Vec<Style>);

        impl TempStyles {
            fn priority(style: &Style) -> u8 {
                match (&style.selector[0], &style.state) {
                    (Selector::All, _) => 1,
                    (Selector::AllNotifications, State::Default) => 2,
                    (Selector::AllNotifications, State::Hover) => 3,
                    (Selector::AllNotifications, State::ContainerHover) => 5,
                    (Selector::Notification(_), State::Default) => 6,
                    (Selector::Notification(_), State::Hover) => 7,
                    (Selector::Notification(_), State::ContainerHover) => 9,
                    (Selector::ActionButton, State::Default) => 10,
                    (Selector::ActionButton, State::Hover) => 11,
                    (Selector::ActionButton, State::ContainerHover) => 13,
                    (Selector::DismissButton, State::Default) => 14,
                    (Selector::DismissButton, State::Hover) => 15,
                    (Selector::DismissButton, State::ContainerHover) => 17,
                    (Selector::Icon, _) => 18,
                    (Selector::Progress, _) => 19,
                    (Selector::PrevCounter, _) => 20,
                    (Selector::NextCounter, _) => 21,
                }
            }

            fn sort(mut self) -> Self {
                self.0.sort_by_key(Self::priority);
                self
            }
        }

        let temp_styles = TempStyles::deserialize(deserializer)?.sort();
        let mut styles = Styles::default();

        temp_styles.0.iter().for_each(|style| {
            style
                .selector
                .iter()
                .for_each(|selector| match (selector, &style.state) {
                    (Selector::All, _) => {
                        styles.default.apply(&style.style);
                        styles.hover.apply(&style.style);

                        styles.prev.apply(&style.style);
                        styles.next.apply(&style.style);

                        styles.default.progress.apply(&style.style);
                        styles.hover.progress.apply(&style.style);

                        styles.default.icon.apply(&style.style);
                        styles.hover.icon.apply(&style.style);

                        styles.default.buttons.action.apply(&style.style);
                        styles.hover.buttons.action.apply(&style.style);

                        styles.default.buttons.dismiss.apply(&style.style);
                        styles.hover.buttons.dismiss.apply(&style.style);
                    }
                    (Selector::NextCounter, _) => styles.next.apply(&style.style),
                    (Selector::PrevCounter, _) => styles.prev.apply(&style.style),
                    (Selector::Progress, State::ContainerHover) => {
                        styles.hover.progress.apply(&style.style);
                    }
                    (Selector::Progress, _) => {
                        styles.default.progress.apply(&style.style);
                        styles.hover.progress.apply(&style.style);
                    }
                    (Selector::Icon, State::ContainerHover) => {
                        if let Some(border) = style.style.border.as_ref() {
                            styles.default.icon.border.apply(border);
                            styles.hover.icon.border.apply(border);
                        }
                    }
                    (Selector::Icon, _) => {
                        if let Some(border) = style.style.border.as_ref() {
                            styles.default.icon.border.apply(border);
                            styles.hover.icon.border.apply(border);
                        }
                    }

                    (Selector::AllNotifications, State::Default) => {
                        styles.default.apply(&style.style);
                        styles.hover.apply(&style.style);
                    }
                    (Selector::AllNotifications, State::Hover | State::ContainerHover) => {
                        styles.hover.apply(&style.style);
                    }
                    (Selector::Notification(_), State::Default) => {}
                    (Selector::Notification(_), State::Hover | State::ContainerHover) => {}
                    (Selector::ActionButton, State::Default) => {
                        styles.default.buttons.action.apply(&style.style);
                        styles.hover.buttons.action.apply(&style.style);
                    }
                    (Selector::ActionButton, State::Hover) => {
                        styles.default.buttons.action.apply_hover(&style.style);
                        styles.hover.buttons.action.apply_hover(&style.style);
                    }
                    (Selector::ActionButton, State::ContainerHover) => {
                        styles.hover.buttons.action.apply(&style.style);
                    }
                    (Selector::DismissButton, State::Default) => {
                        styles.default.buttons.dismiss.apply(&style.style);
                        styles.hover.buttons.dismiss.apply(&style.style);
                    }
                    (Selector::DismissButton, State::Hover) => {
                        styles.default.buttons.dismiss.apply_hover(&style.style);
                        styles.hover.buttons.dismiss.apply_hover(&style.style);
                    }
                    (Selector::DismissButton, State::ContainerHover) => {
                        styles.hover.buttons.dismiss.apply(&style.style);
                    }
                })
        });

        Ok(styles)
    }
}

impl Default for Styles {
    fn default() -> Self {
        Self {
            next: NotificationCounter::default(),
            prev: NotificationCounter::default(),
            notification: Vec::new(),
            default: StyleState {
                buttons: Buttons {
                    dismiss: Button {
                        default: ButtonState {
                            background: Color::rgba([0, 0, 0, 0]),
                            border: Border {
                                size: Insets {
                                    left: 0.,
                                    right: 0.,
                                    top: 0.,
                                    bottom: 0.,
                                },
                                radius: BorderRadius::circle(),
                                color: Color::rgba([0, 0, 0, 0]),
                            },
                            font: Font {
                                color: Color::rgba([0, 0, 0, 0]),
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ..Default::default()
            },
            hover: StyleState::default_hover(),
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    Background,
    Bottom,
    Top,
    #[default]
    Overlay,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Anchor {
    #[default]
    TopRight,
    TopCenter,
    TopLeft,
    BottomRight,
    BottomCenter,
    BottomLeft,
    CenterRight,
    CenterLeft,
    Center,
}

pub struct Timeout {
    urgency_low: i32,
    urgency_normal: i32,
    urgency_critical: i32,
}

impl Default for Timeout {
    fn default() -> Self {
        Self {
            urgency_low: 5,
            urgency_normal: 10,
            urgency_critical: 0,
        }
    }
}

impl Timeout {
    pub fn get(&self, urgency: &crate::Urgency) -> i32 {
        match urgency {
            crate::Urgency::Low => self.urgency_low,
            crate::Urgency::Normal => self.urgency_normal,
            crate::Urgency::Critical => self.urgency_critical,
        }
    }
}

impl<'de> Deserialize<'de> for Timeout {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct TimeoutVisitor;

        impl<'de> serde::de::Visitor<'de> for TimeoutVisitor {
            type Value = Timeout;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a number or a map")
            }

            fn visit_f32<E>(self, v: f32) -> Result<Self::Value, E> {
                let value = v as i32;
                Ok(Timeout {
                    urgency_low: value,
                    urgency_normal: value,
                    urgency_critical: value,
                })
            }

            fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E> {
                let value = v as i32;
                Ok(Timeout {
                    urgency_low: value,
                    urgency_normal: value,
                    urgency_critical: value,
                })
            }

            fn visit_i32<E>(self, value: i32) -> Result<Self::Value, E> {
                Ok(Timeout {
                    urgency_low: value,
                    urgency_normal: value,
                    urgency_critical: value,
                })
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> {
                let value = v as i32;
                Ok(Timeout {
                    urgency_low: value,
                    urgency_normal: value,
                    urgency_critical: value,
                })
            }

            fn visit_u32<E>(self, v: u32) -> Result<Self::Value, E> {
                let value = v as i32;
                Ok(Timeout {
                    urgency_low: value,
                    urgency_normal: value,
                    urgency_critical: value,
                })
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
                let value = v as i32;
                Ok(Timeout {
                    urgency_low: value,
                    urgency_normal: value,
                    urgency_critical: value,
                })
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: serde::de::MapAccess<'de>,
            {
                let mut urgency_low = None;
                let mut urgency_normal = None;
                let mut urgency_critical = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "urgency_low" => urgency_low = Some(map.next_value()?),
                        "urgency_normal" => urgency_normal = Some(map.next_value()?),
                        "urgency_critical" => urgency_critical = Some(map.next_value()?),
                        _ => {
                            return Err(serde::de::Error::unknown_field(
                                &key,
                                &["urgency_low", "urgency_normal", "urgency_critical"],
                            ))
                        }
                    }
                }

                Ok(Timeout {
                    urgency_low: urgency_low.unwrap_or_default(),
                    urgency_normal: urgency_normal.unwrap_or_default(),
                    urgency_critical: urgency_critical.unwrap_or_default(),
                })
            }
        }

        deserializer.deserialize_any(TimeoutVisitor)
    }
}

#[derive(Default)]
pub struct NotificationStyleEntry {
    pub app: Box<str>,
    pub styles: Styles,
    pub default_timeout: Option<Timeout>,
    pub ignore_timeout: Option<bool>,
}

pub struct NotificationCounter {
    pub format: Box<str>,
    pub border: Border,
    pub background: Color,
    pub margin: Insets,
    pub padding: Insets,
}

impl NotificationCounter {
    pub fn apply(&mut self, partial: &PartialStyle) {
        if let Some(background) = partial.background.as_ref() {
            self.background.apply(background);
        }
        if let Some(border) = partial.border.as_ref() {
            self.border.apply(border);
        }
        if let Some(margin) = partial.margin.as_ref() {
            self.margin.apply(margin);
        }
        if let Some(padding) = partial.padding.as_ref() {
            self.padding.apply(padding);
        }
    }
}

impl Default for NotificationCounter {
    fn default() -> Self {
        Self {
            format: "({} more)".into(),
            border: Border::default(),
            background: Color::rgba([26, 27, 38, 255]),
            margin: Insets::default(),
            padding: Insets::default(),
        }
    }
}

impl Config {
    pub fn load(path: Option<PathBuf>) -> anyhow::Result<Self> {
        let config_path = if let Some(path) = path {
            path
        } else {
            Self::path()?
        };

        let lua_code = fs::read_to_string(&config_path).unwrap_or_default();
        let lua = Lua::new();

        let lua_result = lua
            .load(lua_code)
            .eval()
            .map_err(|e| anyhow::anyhow!("Lua evaluation error: {}", e))?;

        lua.from_value(lua_result)
            .map_err(|e| anyhow::anyhow!("Config deserialization error: {}", e))
    }

    pub fn find_style(&self, app_name: &str, hovered: bool) -> &StyleState {
        let styles = &self
            .styles
            .notification
            .iter()
            .find(|n| &*n.app == app_name)
            .map(|c| &c.styles)
            .unwrap_or(&self.styles);

        if hovered {
            &styles.hover
        } else {
            &styles.default
        }
    }

    pub fn path() -> anyhow::Result<PathBuf> {
        let config_dir = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|_| std::env::var("HOME").map(|home| PathBuf::from(home).join(".config")))?;

        Ok(config_dir.join("moxnotify/config.lua"))
    }
}
