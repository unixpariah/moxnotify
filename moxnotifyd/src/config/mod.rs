pub mod button;
pub mod color;

use button::Buttons;
use color::Color;
use mlua::{Lua, LuaSerdeExt};
use serde::{Deserialize, Deserializer};
use std::{collections::HashMap, fs, path::PathBuf, str::FromStr};
use xkbcommon::xkb::Keysym;

#[derive(Deserialize, Default, Clone, Copy)]
pub struct BorderRadius {
    #[serde(default)]
    pub top_left: f32,
    #[serde(default)]
    pub top_right: f32,
    #[serde(default)]
    pub bottom_left: f32,
    #[serde(default)]
    pub bottom_right: f32,
}

impl BorderRadius {
    fn circle() -> Self {
        Self {
            top_right: 50.,
            top_left: 50.,
            bottom_left: 50.,
            bottom_right: 50.,
        }
    }
}

impl From<BorderRadius> for [f32; 4] {
    fn from(value: BorderRadius) -> Self {
        [
            value.bottom_right,
            value.top_right,
            value.bottom_left,
            value.top_left,
        ]
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Insets {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

impl From<Insets> for [f32; 4] {
    fn from(value: Insets) -> Self {
        [value.left, value.right, value.top, value.bottom]
    }
}

#[derive(Deserialize)]
#[serde(default)]
pub struct Border {
    pub size: f32,
    pub radius: BorderRadius,
}

impl Default for Border {
    fn default() -> Self {
        Self {
            size: 0.,
            radius: BorderRadius::default(),
        }
    }
}

#[derive(Deserialize)]
#[serde(default)]
pub struct Font {
    pub size: f32,
    pub family: Box<str>,
}

impl Default for Font {
    fn default() -> Self {
        Self {
            size: 10.,
            family: "DejaVu Sans".into(),
        }
    }
}

#[derive(Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Queue {
    #[default]
    Ordered,
    Unordered,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Urgency {
    pub background: Color,
    pub foreground: Color,
    pub border: Color,
}

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
pub struct Progress {
    height: f32,
    incomplete_color: Color,
    complete_color: Color,
}

impl Default for Progress {
    fn default() -> Self {
        Self {
            height: 20.,
            incomplete_color: Color::rgba([255, 0, 0, 255]),
            complete_color: Color::rgba([0, 255, 0, 255]),
        }
    }
}

#[derive(Deserialize, Default)]
pub struct StyleState {
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
    #[serde(default)]
    pub urgency_low: Urgency,
    #[serde(default)]
    pub urgency_normal: Urgency,
    #[serde(default)]
    pub urgency_critical: Urgency,
    #[serde(default)]
    pub icon: Icon,
    #[serde(default)]
    pub progress: Progress,
    #[serde(default)]
    pub buttons: Buttons,
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

#[derive(Deserialize)]
pub struct NotificationStyleEntry {
    pub app: Box<str>,
    #[serde(default)]
    pub styles: Styles,
    #[serde(default)]
    pub default_timeout: Option<i32>,
    #[serde(default)]
    pub ignore_timeout: Option<bool>,
}

#[derive(Deserialize, Default)]
pub struct Config {
    #[serde(default = "default_scroll_sensitivity")]
    pub scroll_sensitivity: f64,
    #[serde(default = "default_max_visible")]
    pub max_visible: u32,
    #[serde(default = "default_max_icon_size")]
    pub max_icon_size: u32,
    #[serde(default)]
    pub anchor: Anchor,
    #[serde(default)]
    pub layer: Layer,
    #[serde(default)]
    pub queue: Queue,
    #[serde(default)]
    pub output: Box<str>,
    #[serde(default)]
    pub default_timeout: i32,
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
            size: 2.,
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

fn default_max_icon_size() -> u32 {
    64
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
                            urgency_low = {
                              background = "#1A1412",
                              border = "#3C7B82",
                              foreground = "#3C7B82"
                            },
                            urgency_normal = {
                              background = "#1A1412",
                              border = "#567734",
                              foreground = "#567734"
                            },
                            urgency_critical = {
                              background = "#1A1412",
                              border = "#B04027",
                              foreground = "#B04027"
                            },
                          },
                          hover = {
                            urgency_low = { background = "#2f3549FF" },
                            urgency_normal = { background = "#2f3549FF" },
                            urgency_critical = { background = "#2f3549FF" },
                          }
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

    pub fn path() -> anyhow::Result<PathBuf> {
        let config_dir = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|_| std::env::var("HOME").map(|home| PathBuf::from(home).join(".config")))?;

        Ok(config_dir.join("moxnotify/config.lua"))
    }
}
