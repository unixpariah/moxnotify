pub mod border;
pub mod button;
pub mod color;

use border::{Border, BorderRadius};
use button::Buttons;
use color::Color;
use mlua::{Lua, LuaSerdeExt};
use serde::{Deserialize, Deserializer};
use std::{collections::HashMap, fmt, fs, path::PathBuf, str::FromStr};
use xkbcommon::xkb::Keysym;

#[derive(Default, Clone, Copy)]
pub struct Insets {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
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
                Ok(Insets {
                    left: value,
                    right: value,
                    top: value,
                    bottom: value,
                })
            }

            fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E> {
                let value = v as f32;
                Ok(Insets {
                    left: value,
                    right: value,
                    top: value,
                    bottom: value,
                })
            }

            fn visit_i32<E>(self, v: i32) -> Result<Self::Value, E> {
                let value = v as f32;
                Ok(Insets {
                    left: value,
                    right: value,
                    top: value,
                    bottom: value,
                })
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> {
                let value = v as f32;
                Ok(Insets {
                    left: value,
                    right: value,
                    top: value,
                    bottom: value,
                })
            }

            fn visit_u32<E>(self, v: u32) -> Result<Self::Value, E> {
                let value = v as f32;
                Ok(Insets {
                    left: value,
                    right: value,
                    top: value,
                    bottom: value,
                })
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
                let value = v as f32;
                Ok(Insets {
                    left: value,
                    right: value,
                    top: value,
                    bottom: value,
                })
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
            color: Color::default(),
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

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Urgency {}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Icon {
    pub border: Border,
}

#[derive(Deserialize, Default, Debug)]
pub enum Size {
    #[default]
    #[serde(rename = "auto")]
    Auto,
    #[serde(untagged)]
    Value(f32),
}

#[derive(Deserialize)]
#[serde(default)]
pub struct Progress {
    pub height: f32,
    pub border: Border,
    pub incomplete_color: Color,
    pub complete_color: Color,
}

impl Default for Progress {
    fn default() -> Self {
        Self {
            height: 20.,
            border: Border {
                color: Color::default(),
                size: Insets {
                    left: 2.,
                    right: 2.,
                    top: 2.,
                    bottom: 2.,
                },
                radius: BorderRadius {
                    top_left: 5.,
                    top_right: 5.,
                    bottom_left: 5.,
                    bottom_right: 5.,
                },
            },
            incomplete_color: Color::rgba([255, 0, 0, 255]),
            complete_color: Color::rgba([0, 255, 0, 255]),
        }
    }
}

#[derive(Deserialize, Default)]
pub struct StyleState {
    #[serde(default)]
    pub background: Color,
    #[serde(default = "default_width")]
    pub width: f32,
    #[serde(default)]
    pub min_height: Size,
    #[serde(default)]
    pub max_height: Size,
    #[serde(default)]
    pub height: Size,
    #[serde(default)]
    pub font: Font,
    #[serde(default)]
    pub border: Border,
    #[serde(default = "default_margin")]
    pub margin: Insets,
    #[serde(default = "default_padding")]
    pub padding: Insets,
    #[serde(default = "default_icon")]
    pub icon: Icon,
    #[serde(default)]
    pub app_icon: Icon,
    #[serde(default)]
    pub progress: Progress,
    #[serde(default)]
    pub buttons: Buttons,
}

fn default_icon() -> Icon {
    Icon {
        border: Border {
            color: Color::default(),
            size: Insets {
                left: 0.,
                right: 0.,
                top: 0.,
                bottom: 0.,
            },
            radius: BorderRadius {
                top_left: 50.,
                top_right: 50.,
                bottom_left: 50.,
                bottom_right: 50.,
            },
        },
    }
}

fn default_margin() -> Insets {
    Insets {
        left: 5.,
        right: 5.,
        top: 5.,
        bottom: 5.,
    }
}

fn default_padding() -> Insets {
    Insets {
        left: 10.,
        right: 10.,
        top: 10.,
        bottom: 10.,
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Styles {
    pub default: StyleState,
    pub hover: StyleState,
}

#[derive(Deserialize, PartialEq, Eq, Hash, Debug, Default)]
pub struct KeyCombination {
    pub modifiers: Modifiers,
    pub key: Key,
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

        let key = match key_str.to_lowercase().as_str() {
            "enter" => Key::SpecialKey(SpecialKeyCode::Enter),
            "backspace" => Key::SpecialKey(SpecialKeyCode::Backspace),
            "tab" => Key::SpecialKey(SpecialKeyCode::Tab),
            "space" => Key::SpecialKey(SpecialKeyCode::Space),
            "escape" => Key::SpecialKey(SpecialKeyCode::Escape),
            key_str if key_str.len() == 1 => Key::Character(key_str.chars().next().unwrap()),
            _ => return Err(format!("Invalid key: {}", key_str)),
        };

        Ok(KeyCombination { modifiers, key })
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
                    Key::Character(character)
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
    Unfocus,
}

#[derive(Deserialize, PartialEq, Eq, Hash, Debug, Default)]
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

#[derive(Deserialize)]
pub struct NotificationStyleEntry {
    pub app: Box<str>,
    #[serde(default)]
    pub styles: Styles,
    #[serde(default)]
    pub default_timeout: Option<Timeout>,
    #[serde(default)]
    pub ignore_timeout: Option<bool>,
}

#[derive(Deserialize, Default)]
pub struct Config {
    #[serde(default = "default_scroll_sensitivity")]
    pub scroll_sensitivity: f64,
    #[serde(default = "default_max_visible")]
    pub max_visible: u32,
    #[serde(default = "default_icon_size")]
    pub icon_size: u32,
    #[serde(default = "default_app_icon_size")]
    pub app_icon_size: u32,
    #[serde(default)]
    pub anchor: Anchor,
    #[serde(default)]
    pub layer: Layer,
    #[serde(default)]
    pub queue: Queue,
    #[serde(default)]
    pub output: Box<str>,
    #[serde(default)]
    pub default_timeout: Timeout,
    #[serde(default)]
    pub ignore_timeout: bool,
    #[serde(default)]
    pub styles: Styles,
    #[serde(default)]
    pub notification: Vec<NotificationStyleEntry>,
    #[serde(default = "default_keymaps")]
    #[serde(deserialize_with = "deserialize_keycombination_map")]
    pub keymaps: HashMap<KeyCombination, KeyAction>,
    #[serde(default = "default_notification_counter")]
    pub prev: NotificationCounter,
    #[serde(default = "default_notification_counter")]
    pub next: NotificationCounter,
}

#[derive(Default, Deserialize)]
pub struct NotificationCounter {
    pub format: Box<str>,
    pub border: Border,
    pub border_color: Color,
    pub background_color: Color,
    pub margin: Insets,
    pub padding: Insets,
}

fn default_notification_counter() -> NotificationCounter {
    NotificationCounter {
        format: "({} more)".into(),
        border: Border {
            color: Color {
                urgency_low: [158, 206, 106, 255],
                urgency_normal: [187, 154, 247, 255],
                urgency_critical: [192, 202, 245, 255],
            },
            size: Insets {
                left: 2.,
                right: 2.,
                top: 2.,
                bottom: 2.,
            },
            radius: BorderRadius {
                top_left: 5.,
                top_right: 5.,
                bottom_left: 5.,
                bottom_right: 5.,
            },
        },
        border_color: Color::rgba([158, 206, 106, 255]),
        background_color: Color::rgba([26, 27, 38, 255]),
        margin: Insets::default(),
        padding: Insets::default(),
    }
}

fn default_keymaps() -> HashMap<KeyCombination, KeyAction> {
    let mut keymaps: HashMap<KeyCombination, KeyAction> = HashMap::new();

    let mut insert_default = |key: Key, default_action: KeyAction| {
        let key_combination = KeyCombination {
            modifiers: Modifiers {
                control: false,
                shift: false,
                alt: false,
                meta: false,
            },
            key,
        };

        if !keymaps.values().any(|action| *action == default_action) {
            keymaps.insert(key_combination, default_action);
        }
    };

    insert_default(Key::Character('j'), KeyAction::NextNotification);
    insert_default(Key::Character('k'), KeyAction::PreviousNotification);
    insert_default(Key::Character('x'), KeyAction::DismissNotification);
    insert_default(Key::SpecialKey(SpecialKeyCode::Escape), KeyAction::Unfocus);

    keymaps
}

fn default_scroll_sensitivity() -> f64 {
    20.
}

fn default_icon_size() -> u32 {
    64
}

fn default_app_icon_size() -> u32 {
    16
}

fn default_max_visible() -> u32 {
    5
}

fn default_width() -> f32 {
    300.0
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

    let mut insert_default = |key: Key, default_action: KeyAction| {
        let key_combination = KeyCombination {
            modifiers: Modifiers {
                control: false,
                shift: false,
                alt: false,
                meta: false,
            },
            key,
        };

        if !keymaps.values().any(|action| *action == default_action) {
            keymaps.insert(key_combination, default_action);
        }
    };

    insert_default(Key::Character('j'), KeyAction::NextNotification);
    insert_default(Key::Character('k'), KeyAction::PreviousNotification);
    insert_default(Key::Character('x'), KeyAction::DismissNotification);
    insert_default(Key::SpecialKey(SpecialKeyCode::Escape), KeyAction::Unfocus);

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
        let lua_code = match fs::read_to_string(&config_path) {
            Ok(content) => content,
            Err(_) => r##"
                      return {
                        styles = {
                          default = {
                            background = {
                                urgency_low = "#1a1b26FF",
                                urgency_normal = "#16161eFF",
                                urgency_critical = "#16161eFF",
                            },
                            font = { color = "#a9b1d6" },
                            border = { color = {
                                urgency_low = "#9ece6a",
                                urgency_normal = "#bb9af7",
                                urgency_critical = "#c0caf5",
                            }},
                          },
                          hover = { background = "#2f3549FF" }
                        }
                      }
                      "##
            .into(),
        };
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
