use mlua::{Lua, LuaSerdeExt};
use serde::{Deserialize, Deserializer};
use std::{collections::HashMap, fs, path::PathBuf, str::FromStr};
use xkbcommon::xkb::Keysym;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Color {
    pub rgba: [u8; 4],
}

impl FromStr for Color {
    type Err = String;

    fn from_str(hex: &str) -> Result<Self, Self::Err> {
        if !hex.starts_with('#') {
            return Err("Hex string must start with '#'".to_string());
        }

        let hex_part = &hex[1..];
        let len = hex_part.len();

        if ![3, 4, 6, 8].contains(&len) {
            return Err(format!(
                "Invalid hex length: {} (expected 3, 4, 6, or 8 characters)",
                len
            ));
        }

        let expanded = match len {
            3 | 4 => {
                let mut expanded = String::with_capacity(8);
                for c in hex_part.chars() {
                    expanded.push(c);
                    expanded.push(c);
                }
                if len == 3 {
                    expanded.push_str("ff");
                }
                expanded
            }
            6 => format!("{}ff", hex_part),
            8 => hex_part.to_string(),
            _ => unreachable!(),
        };

        let parse_component = |start: usize| -> Result<u8, String> {
            u8::from_str_radix(&expanded[start..start + 2], 16)
                .map_err(|e| format!("Invalid hex component: {}", e))
        };

        let r = parse_component(0)?;
        let g = parse_component(2)?;
        let b = parse_component(4)?;
        let a = parse_component(6)?;

        Ok(Color { rgba: [r, g, b, a] })
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl From<Color> for [f32; 4] {
    fn from(color: Color) -> Self {
        let color = color.rgba;
        [
            color[0] as f32 / 255.0,
            color[1] as f32 / 255.0,
            color[2] as f32 / 255.0,
            color[3] as f32 / 255.0,
        ]
    }
}

#[derive(Deserialize, Default, Clone, Copy)]
pub struct BorderRadius {
    top_left: f32,
    top_right: f32,
    bottom_left: f32,
    bottom_right: f32,
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

#[derive(Deserialize, Default)]
pub struct Border {
    pub size: f32,
    pub radius: BorderRadius,
}

#[derive(Deserialize, Default)]
pub struct Font {
    pub size: f32,
    pub family: Box<str>,
}

#[derive(Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Queue {
    #[default]
    Ordered,
    Unordered,
}

#[derive(Deserialize, Default)]
pub struct Urgency {
    pub background: Color,
    pub foreground: Color,
    pub border: Color,
}

#[derive(Deserialize, Default)]
pub struct Icon {
    pub border: Border,
}

#[derive(Deserialize, Default)]
pub struct StyleState {
    pub font: Font,
    pub border: Border,
    #[serde(default)]
    pub margin: Insets,
    pub padding: Insets,
    pub urgency_low: Urgency,
    pub urgency_normal: Urgency,
    pub urgency_critical: Urgency,
    pub icon: Icon,
}

#[derive(Deserialize, Default)]
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

        for part in parts {
            match part {
                "ctrl" => modifiers.control = true,
                "shift" => modifiers.shift = true,
                "alt" => modifiers.alt = true,
                "meta" => modifiers.meta = true,
                _ => return Err(format!("Invalid modifier: {}", part)),
            }
        }

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
    InvokeAction,
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

#[derive(Deserialize, Default)]
pub struct Config {
    #[serde(default = "default_max_visible")]
    pub max_visible: u32,
    #[serde(default = "default_max_icon_size")]
    pub max_icon_size: u32,
    #[serde(default)]
    pub anchor: Anchor,
    #[serde(default)]
    pub layer: Layer,
    #[serde(default)]
    pub ignore_timeout: bool,
    #[serde(default)]
    pub queue: Queue,
    #[serde(default)]
    pub min_height: Option<f32>,
    #[serde(default)]
    pub max_height: Option<f32>,
    #[serde(default = "default_width")]
    pub width: f32,
    #[serde(default)]
    pub height: Option<f32>,
    #[serde(default)]
    pub output: Box<str>,
    #[serde(default)]
    pub default_timeout: i32,
    #[serde(default)]
    pub styles: Styles,
    #[serde(deserialize_with = "deserialize_keycombination_map")]
    pub keymaps: HashMap<KeyCombination, KeyAction>,
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
    insert_default(
        Key::SpecialKey(SpecialKeyCode::Enter),
        KeyAction::InvokeAction,
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
        let lua_code = fs::read_to_string(&config_path)?;
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

        Ok(config_dir.join("moxalert/config.lua"))
    }
}
