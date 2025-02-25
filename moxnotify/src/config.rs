use mlua::{Lua, LuaSerdeExt};
use serde::{Deserialize, Deserializer};
use std::{collections::HashMap, ffi::OsStr, fs, path::PathBuf, str::FromStr};
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

impl Color {
    pub fn to_linear(self) -> [f32; 4] {
        let srgb_to_linear = |c: u8| {
            let normalized = c as f32 / 255.0;
            if normalized > 0.04045 {
                ((normalized + 0.055) / 1.055).powf(2.4)
            } else {
                normalized / 12.92
            }
        };

        let r = srgb_to_linear(self.rgba[0]);
        let g = srgb_to_linear(self.rgba[1]);
        let b = srgb_to_linear(self.rgba[2]);

        let a = self.rgba[3] as f32 / 255.0;

        [r * a, g * a, b * a, a]
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
    #[serde(default)]
    pub top_left: f32,
    #[serde(default)]
    pub top_right: f32,
    #[serde(default)]
    pub bottom_left: f32,
    #[serde(default)]
    pub bottom_right: f32,
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
pub struct Insets {
    #[serde(default)]
    pub left: f32,
    #[serde(default)]
    pub right: f32,
    #[serde(default)]
    pub top: f32,
    #[serde(default)]
    pub bottom: f32,
}

impl From<Insets> for [f32; 4] {
    fn from(value: Insets) -> Self {
        [value.left, value.right, value.top, value.bottom]
    }
}

#[derive(Deserialize)]
pub struct Border {
    #[serde(default)]
    pub size: f32,
    #[serde(default)]
    pub radius: BorderRadius,
}

impl Default for Border {
    fn default() -> Self {
        Self {
            size: 2.,
            radius: BorderRadius::default(),
        }
    }
}

#[derive(Deserialize)]
pub struct Font {
    #[serde(default = "default_font_size")]
    pub size: f32,
    #[serde(default = "default_font_family")]
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

fn default_font_size() -> f32 {
    10.
}

fn default_font_family() -> Box<str> {
    "DejaVu Sans".into()
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
    #[serde(default)]
    pub background: Color,
    #[serde(default)]
    pub foreground: Color,
    #[serde(default)]
    pub border: Color,
    #[serde(default)]
    pub icon_border: Color,
}

#[derive(Deserialize, Default)]
pub struct Icon {
    #[serde(default)]
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
pub struct Styles {
    #[serde(default)]
    pub default: StyleState,
    #[serde(default)]
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
            match part {
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
pub struct Button {
    pub width: f32,
    pub height: f32,
    pub border: Border,
}

#[derive(Deserialize, Default)]
pub struct Buttons {
    pub dismiss: Button,
    pub action: Button,
}

#[derive(Deserialize, Default)]
pub struct Config {
    #[serde(default = "default_icon_paths")]
    #[serde(deserialize_with = "deserialize_icon_paths")]
    pub icon_paths: Vec<Box<OsStr>>,
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
    #[serde(default = "default_buttons")]
    pub button: Buttons,
    #[serde(default = "default_keymaps")]
    #[serde(deserialize_with = "deserialize_keycombination_map")]
    pub keymaps: HashMap<KeyCombination, KeyAction>,
}

fn default_buttons() -> Buttons {
    Buttons {
        dismiss: Button {
            height: 20.,
            width: 20.,
            border: Border {
                size: 0.,
                radius: BorderRadius {
                    top_left: 50.,
                    top_right: 50.,
                    bottom_left: 50.,
                    bottom_right: 50.,
                },
            },
        },
        action: Button {
            height: 20.,
            width: 20.,
            border: Border {
                size: 0.,
                radius: BorderRadius {
                    top_left: 50.,
                    top_right: 50.,
                    bottom_left: 50.,
                    bottom_right: 50.,
                },
            },
        },
    }
}

fn default_icon_paths() -> Vec<Box<OsStr>> {
    vec![
        OsStr::new("/usr/share/icons/hicolor").into(),
        OsStr::new("/usr/share/pixmaps").into(),
    ]
}

fn deserialize_icon_paths<'de, D>(deserializer: D) -> Result<Vec<Box<OsStr>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let mut res: Vec<Box<OsStr>> = s.split(':').map(|part| OsStr::new(part).into()).collect();
    res.push(OsStr::new("/usr/share/icons/hicolor").into());
    res.push(OsStr::new("/usr/share/pixmaps").into());
    Ok(res)
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
                              icon_border = "#3C7B82",
                              foreground = "#3C7B82"
                            },
                            urgency_normal = {
                              background = "#1A1412",
                              border = "#567734",
                              icon_border = "#567734",
                              foreground = "#567734"
                            },
                            urgency_critical = {
                              background = "#1A1412",
                              border = "#B04027",
                              icon_border = "#B04027",
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
                        
                        styles.default = deep_merge(
                            user_config.styles.default or {{}},
                            styles.default or {{}}
                        )
                        
                        styles.hover = deep_merge(
                            user_config.styles.hover or {{}},
                            styles.hover or {{}}
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_3_char_hex() {
        let color = Color::from_str("#fff").unwrap();
        assert_eq!(color.rgba, [0xff, 0xff, 0xff, 0xff]);

        let color = Color::from_str("#000").unwrap();
        assert_eq!(color.rgba, [0x00, 0x00, 0x00, 0xff]);
    }

    #[test]
    fn valid_6_char_hex() {
        let color = Color::from_str("#ff0000").unwrap();
        assert_eq!(color.rgba, [0xff, 0x00, 0x00, 0xff]);

        let color = Color::from_str("#00ff00ff").unwrap();
        assert_eq!(color.rgba, [0x00, 0xff, 0x00, 0xff]);
    }

    #[test]
    fn valid_8_char_hex() {
        let color = Color::from_str("#12345678").unwrap();
        assert_eq!(color.rgba, [0x12, 0x34, 0x56, 0x78]);

        let color = Color::from_str("#abcdef42").unwrap();
        assert_eq!(color.rgba, [0xab, 0xcd, 0xef, 0x42]);
    }

    #[test]
    fn mixed_case_hex() {
        let color = Color::from_str("#FfEeDd").unwrap();
        assert_eq!(color.rgba, [0xff, 0xee, 0xdd, 0xff]);

        let color = Color::from_str("#AaBbCcDd").unwrap();
        assert_eq!(color.rgba, [0xaa, 0xbb, 0xcc, 0xdd]);
    }

    #[test]
    fn invalid_cases() {
        let test_cases = vec![
            ("missing hash", "fff", "Hex string must start with '#'"),
            ("invalid_char", "#ggg", "Invalid hex component"),
            ("wrong_length_5", "#12345", "Invalid hex length: 5"),
            ("wrong_length_7", "#1234567", "Invalid hex length: 7"),
        ];

        test_cases.iter().for_each(|(name, input, err_msg)| {
            let result = Color::from_str(input);
            assert!(result.is_err(), "{} should fail", name);
            let err = result.unwrap_err();
            assert!(
                err.contains(err_msg),
                "{}: Expected error containing '{}', got '{}'",
                name,
                err_msg,
                err
            );
        });
    }

    #[test]
    fn alpha_channel_handling() {
        let opaque = Color::from_str("#123456").unwrap();
        assert_eq!(opaque.rgba[3], 0xff, "Default alpha should be 0xff");

        let transparent = Color::from_str("#12345678").unwrap();
        assert_eq!(transparent.rgba[3], 0x78);

        let semi_transparent = Color::from_str("#a1b2c3d4").unwrap();
        assert_eq!(semi_transparent.rgba, [0xa1, 0xb2, 0xc3, 0xd4]);
    }

    #[test]
    fn expansion_of_3_char() {
        let color = Color::from_str("#f0c").unwrap();
        assert_eq!(color.rgba, [0xff, 0x00, 0xcc, 0xff]);

        let color = Color::from_str("#1a2").unwrap();
        assert_eq!(color.rgba, [0x11, 0xaa, 0x22, 0xff]);
    }
}
