pub mod border;
pub mod button;
pub mod color;

use border::{Border, BorderRadius};
use button::{Button, ButtonState, Buttons};
use color::Color;
use mlua::{Lua, LuaSerdeExt};
use serde::{Deserialize, Deserializer};
use std::{collections::HashMap, fmt, fs, ops::Deref, path::PathBuf, str::FromStr};
use xkbcommon::xkb::Keysym;

#[derive(Deserialize, Debug)]
#[serde(default)]
pub struct Keymaps(
    #[serde(deserialize_with = "deserialize_keycombination_map")]
    HashMap<KeyCombination, KeyAction>,
);

impl Keymaps {
    pub fn matches(&self, sequence: &[Key]) -> bool {
        self.0.keys().any(|kc| kc.keys.starts_with(sequence))
    }
}

impl Default for Keymaps {
    fn default() -> Self {
        let mut keymaps: HashMap<KeyCombination, KeyAction> = HashMap::new();

        let mut insert_default =
            |keys: Vec<Key>, modifiers: Modifiers, default_action: KeyAction| {
                let key_combination = KeyCombination { modifiers, keys };

                if !keymaps.values().any(|action| *action == default_action) {
                    keymaps.insert(key_combination, default_action);
                }
            };

        insert_default(
            vec![Key::Character('j')],
            Modifiers::default(),
            KeyAction::NextNotification,
        );
        insert_default(
            vec![Key::Character('k')],
            Modifiers::default(),
            KeyAction::PreviousNotification,
        );
        insert_default(
            vec![Key::Character('x')],
            Modifiers::default(),
            KeyAction::DismissNotification,
        );
        insert_default(
            vec![Key::Character('d'), Key::Character('d')],
            Modifiers::default(),
            KeyAction::DismissNotification,
        );
        insert_default(
            vec![Key::Character('g')],
            Modifiers {
                shift: true,
                ..Default::default()
            },
            KeyAction::LastNotification,
        );
        insert_default(
            vec![Key::Character('g'), Key::Character('g')],
            Modifiers::default(),
            KeyAction::FirstNotification,
        );
        insert_default(
            vec![Key::SpecialKey(SpecialKeyCode::Escape)],
            Modifiers::default(),
            KeyAction::Unfocus,
        );

        Self(keymaps)
    }
}

impl Deref for Keymaps {
    type Target = HashMap<KeyCombination, KeyAction>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

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
    pub output: Box<str>,
    pub default_timeout: Timeout,
    pub ignore_timeout: bool,
    pub styles: Styles,
    pub notification: Vec<NotificationStyleEntry>,
    pub keymaps: Keymaps,
    pub prev: NotificationCounter,
    pub next: NotificationCounter,
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
            output: "".into(),
            default_timeout: Timeout::default(),
            ignore_timeout: false,
            styles: Styles::default(),
            notification: Vec::new(),
            keymaps: Keymaps::default(),
            prev: NotificationCounter::default(),
            next: NotificationCounter::default(),
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
}

impl<'de> Deserialize<'de> for Insets {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct InsetsVisitor;

        impl<'de> serde::de::Visitor<'de> for InsetsVisitor {
            type Value = Insets;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a number or a map with optional corner values")
            }

            fn visit_f32<E>(self, value: f32) -> Result<Self::Value, E> {
                Ok(Insets::size(value))
            }

            fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E> {
                Ok(Insets::size(value as f32))
            }

            fn visit_i32<E>(self, value: i32) -> Result<Self::Value, E> {
                Ok(Insets::size(value as f32))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
                Ok(Insets::size(value as f32))
            }

            fn visit_u32<E>(self, value: u32) -> Result<Self::Value, E> {
                Ok(Insets::size(value as f32))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
                Ok(Insets::size(value as f32))
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: serde::de::MapAccess<'de>,
            {
                let mut left = None;
                let mut right = None;
                let mut top = None;
                let mut bottom = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "left" => left = Some(map.next_value()?),
                        "right" => right = Some(map.next_value()?),
                        "top" => top = Some(map.next_value()?),
                        "bottom" => bottom = Some(map.next_value()?),
                        _ => {
                            return Err(serde::de::Error::unknown_field(
                                &key,
                                &["left", "right", "top", "bottom"],
                            ))
                        }
                    }
                }

                Ok(Insets {
                    left: left.unwrap_or(0.0),
                    right: right.unwrap_or(0.0),
                    top: top.unwrap_or(0.0),
                    bottom: bottom.unwrap_or(0.0),
                })
            }
        }

        deserializer.deserialize_any(InsetsVisitor)
    }
}

impl From<Insets> for [f32; 4] {
    fn from(value: Insets) -> Self {
        [value.left, value.right, value.top, value.bottom]
    }
}

#[derive(Deserialize)]
#[serde(default)]
pub struct Font {
    pub size: f32,
    pub family: Box<str>,
    pub color: Color,
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

#[derive(Deserialize)]
#[serde(default)]
pub struct Icon {
    pub border: Border,
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

#[derive(Deserialize, Default, Debug)]
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

#[derive(Deserialize)]
#[serde(default)]
pub struct Progress {
    pub margin: Insets,
    pub height: Size,
    pub width: Size,
    pub border: Border,
    pub incomplete_color: Color,
    pub complete_color: Color,
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

#[derive(Deserialize)]
#[serde(default)]
pub struct StyleState {
    pub background: Color,
    pub width: f32,
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
}

impl Default for StyleState {
    fn default() -> Self {
        Self {
            background: Color {
                urgency_low: [26, 27, 38, 255],
                urgency_normal: [22, 22, 30, 255],
                urgency_critical: [22, 22, 30, 255],
            },
            width: 300.,
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

#[derive(Deserialize)]
#[serde(default)]
pub struct Styles {
    #[serde(default)]
    pub default: StyleState,
    #[serde(default = "StyleState::default_hover")]
    pub hover: StyleState,
}

impl Default for Styles {
    fn default() -> Self {
        Self {
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

#[derive(Deserialize, PartialEq, Eq, Hash, Debug, Default)]
pub struct KeyCombination {
    pub keys: Vec<Key>,
    pub modifiers: Modifiers,
}

impl KeyCombination {
    pub fn clear(&mut self) {
        self.keys.clear();
        self.modifiers = Modifiers::default();
    }
}

impl FromStr for KeyCombination {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split('+');
        let mut modifiers = Modifiers::default();
        let key_str = parts.next_back().ok_or("Invalid key combination")?;

        parts.try_for_each(|part| {
            match part.to_lowercase().as_str() {
                "ctrl" => modifiers.control = true,
                "shift" => modifiers.shift = true,
                "alt" => modifiers.alt = true,
                "meta" => modifiers.meta = true,
                _ => return Err(format!("Invalid modifier: {}", part)),
            }

            Ok(())
        })?;

        let keys: Vec<Key> = match key_str.to_lowercase().as_str() {
            "enter" => vec![Key::SpecialKey(SpecialKeyCode::Enter)],
            "backspace" => vec![Key::SpecialKey(SpecialKeyCode::Backspace)],
            "tab" => vec![Key::SpecialKey(SpecialKeyCode::Tab)],
            "space" => vec![Key::SpecialKey(SpecialKeyCode::Space)],
            "escape" => vec![Key::SpecialKey(SpecialKeyCode::Escape)],
            _ => key_str.chars().map(Key::Character).collect(),
        };

        Ok(KeyCombination { modifiers, keys })
    }
}

#[derive(Deserialize, PartialEq, Eq, Hash, Debug, Clone, Copy)]
pub enum Key {
    Character(char),
    SpecialKey(SpecialKeyCode),
}

impl Key {
    pub fn from_keycode(
        xkb_state: &xkbcommon::xkb::State,
        keycode: xkbcommon::xkb::Keycode,
    ) -> Self {
        let key_name = xkb_state.key_get_one_sym(keycode);

        match key_name {
            Keysym::Return => Key::SpecialKey(SpecialKeyCode::Enter),
            Keysym::BackSpace => Key::SpecialKey(SpecialKeyCode::Backspace),
            Keysym::Tab => Key::SpecialKey(SpecialKeyCode::Tab),
            Keysym::Escape => Key::SpecialKey(SpecialKeyCode::Escape),
            _ => {
                let key_sym = xkb_state.key_get_one_sym(keycode);
                if u32::from(key_sym) == xkbcommon::xkb::keysyms::KEY_NoSymbol {
                    return Key::default();
                }
                let key_char_code = xkb_state.key_get_utf32(keycode);
                if let Some(character) = char::from_u32(key_char_code) {
                    Key::Character(character.to_ascii_lowercase())
                } else {
                    Key::default()
                }
            }
        }
    }
}

impl Default for Key {
    fn default() -> Self {
        Key::Character('\0')
    }
}

#[derive(Deserialize, PartialEq, Eq, Hash, Debug, Clone, Copy)]
pub enum SpecialKeyCode {
    Enter,
    Backspace,
    Tab,
    Space,
    Escape,
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(tag = "action")]
#[serde(rename_all = "snake_case")]
pub enum KeyAction {
    NextNotification,
    PreviousNotification,
    DismissNotification,
    FirstNotification,
    LastNotification,
    Unfocus,
    Noop,
}

#[derive(Deserialize, PartialEq, Eq, Hash, Debug, Default, Clone)]
pub struct Modifiers {
    pub control: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
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

#[derive(Deserialize, Default)]
pub struct NotificationStyleEntry {
    pub app: Box<str>,
    pub styles: Styles,
    pub default_timeout: Option<Timeout>,
    pub ignore_timeout: Option<bool>,
}

#[derive(Deserialize)]
pub struct NotificationCounter {
    pub format: Box<str>,
    pub border: Border,
    pub background: Color,
    pub margin: Insets,
    pub padding: Insets,
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

fn deserialize_keycombination_map<'de, D>(
    deserializer: D,
) -> Result<HashMap<KeyCombination, KeyAction>, D::Error>
where
    D: Deserializer<'de>,
{
    let map: HashMap<String, KeyAction> = HashMap::deserialize(deserializer)?;
    let mut keymaps: HashMap<KeyCombination, KeyAction> = HashMap::new();
    for (key_str, action) in map {
        let key_combination =
            deserialize_keycombination_inner(&key_str).map_err(serde::de::Error::custom)?;
        keymaps.insert(key_combination, action);
    }

    let mut insert_default = |keys: Vec<Key>, modifiers: Modifiers, default_action: KeyAction| {
        let key_combination = KeyCombination { modifiers, keys };

        if !keymaps.values().any(|action| *action == default_action) {
            keymaps.insert(key_combination, default_action);
        }
    };

    insert_default(
        vec![Key::Character('j')],
        Modifiers::default(),
        KeyAction::NextNotification,
    );
    insert_default(
        vec![Key::Character('k')],
        Modifiers::default(),
        KeyAction::PreviousNotification,
    );
    insert_default(
        vec![Key::Character('x')],
        Modifiers::default(),
        KeyAction::DismissNotification,
    );
    insert_default(
        vec![Key::Character('d'), Key::Character('d')],
        Modifiers::default(),
        KeyAction::DismissNotification,
    );
    insert_default(
        vec![Key::Character('g')],
        Modifiers {
            shift: true,
            ..Default::default()
        },
        KeyAction::LastNotification,
    );
    insert_default(
        vec![Key::Character('g'), Key::Character('g')],
        Modifiers::default(),
        KeyAction::FirstNotification,
    );
    insert_default(
        vec![Key::SpecialKey(SpecialKeyCode::Escape)],
        Modifiers::default(),
        KeyAction::Unfocus,
    );

    Ok(keymaps)
}

fn deserialize_keycombination_inner(value: &str) -> Result<KeyCombination, String> {
    KeyCombination::from_str(value)
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
            .load(format!(
                r#"
                local function deep_merge(base, override)
                    local function is_array(t)
                        if type(t) ~= 'table' then
                            return false
                        end
                        local i = 0
                        for _ in pairs(t) do
                            i = i + 1
                            if t[i] == nil then
                                return false
                            end
                        end
                        return true
                    end

                    local merged = {{}}
                    for k, v in pairs(base) do
                        if type(v) == 'table' and not is_array(v) then
                            merged[k] = deep_merge(v, {{}})
                        else
                            merged[k] = v
                        end
                    end
                    for k, v in pairs(override) do
                        if type(v) == 'table' and not is_array(v) and type(merged[k]) == 'table' and not is_array(merged[k]) then
                            merged[k] = deep_merge(merged[k], v)
                        else
                            merged[k] = v
                        end
                    end
                    return merged
                end

                local user_config = (function()
                    local config = {{}}
                    local env = {{
                        config = config,
                    }}
                    local user_return = (function()
                        local _ENV = env
                        {lua_code}
                    end)()

                    if type(user_return) == 'table' then
                        for k, v in pairs(user_return) do
                            config[k] = v
                        end
                    end
                    return config
                end)()

                if user_config.styles then
                    user_config.styles.default = user_config.styles.default or {{}}

                    user_config.styles.hover = deep_merge(
                        user_config.styles.default,
                        user_config.styles.hover or {{}}
                    )
                end

                if user_config.notification then
                    for _, entry in ipairs(user_config.notification) do
                        local styles = entry.styles or {{}}

                        styles.hover = deep_merge(
                            deep_merge(
                                user_config.styles.hover or {{}},
                                styles.default or {{}}
                            ),
                            styles.hover or {{}}
                        )
                        
                        styles.default = deep_merge(
                            user_config.styles.default or {{}},
                            styles.default or {{}}
                        )
                        
                    end
                end

                return user_config
                "#,
                lua_code = lua_code
            ))
            .eval()
            .map_err(|e| anyhow::anyhow!("Lua evaluation error: {}", e))?;

        let config: Config = lua
            .from_value(lua_result)
            .map_err(|e| anyhow::anyhow!("Config deserialization error: {}", e))?;

        Ok(config)
    }

    pub fn find_style(&self, app_name: &str, hovered: bool) -> &StyleState {
        let styles = self
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
