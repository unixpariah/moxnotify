use serde::{Deserialize, Deserializer};
use std::{collections::HashMap, fmt, ops::Deref, str::FromStr};
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

fn default_key_combinations() -> Vec<(KeyCombination, KeyAction)> {
    vec![
        (
            KeyCombination {
                keys: vec![Key::Character('j')],
                ..Default::default()
            },
            KeyAction::NextNotification,
        ),
        (
            KeyCombination {
                keys: vec![Key::Character('k')],
                ..Default::default()
            },
            KeyAction::PreviousNotification,
        ),
        (
            KeyCombination {
                keys: vec![Key::Character('x')],
                ..Default::default()
            },
            KeyAction::DismissNotification,
        ),
        (
            KeyCombination {
                keys: vec![Key::Character('d'), Key::Character('d')],
                ..Default::default()
            },
            KeyAction::DismissNotification,
        ),
        (
            KeyCombination {
                keys: vec![Key::Character('G')],
                ..Default::default()
            },
            KeyAction::LastNotification,
        ),
        (
            KeyCombination {
                keys: vec![Key::Character('g'), Key::Character('g')],
                ..Default::default()
            },
            KeyAction::FirstNotification,
        ),
        (
            KeyCombination {
                mode: Mode::Hint,
                keys: vec![Key::SpecialKey(SpecialKeyCode::Escape)],
                ..Default::default()
            },
            KeyAction::NormalMode,
        ),
        (
            KeyCombination {
                keys: vec![Key::SpecialKey(SpecialKeyCode::Escape)],
                ..Default::default()
            },
            KeyAction::Unfocus,
        ),
        (
            KeyCombination {
                keys: vec![Key::Character('f')],
                ..Default::default()
            },
            KeyAction::HintMode,
        ),
    ]
}

impl Default for Keymaps {
    fn default() -> Self {
        let mut keymaps = HashMap::new();
        for (kc, action) in default_key_combinations() {
            keymaps.insert(kc, action);
        }
        Self(keymaps)
    }
}

impl Deref for Keymaps {
    type Target = HashMap<KeyCombination, KeyAction>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Deserialize, PartialEq, Eq, Hash, Debug, Default, Clone, Copy)]
pub enum Mode {
    #[default]
    Normal,
    Hint,
}

impl FromStr for Mode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "normal" => Ok(Mode::Normal),
            "hint" => Ok(Mode::Hint),
            _ => Err(format!("Invalid mode: {}", s)),
        }
    }
}

#[derive(Deserialize, PartialEq, Eq, Hash, Debug, Default)]
pub struct KeyCombination {
    #[serde(default)]
    pub mode: Mode,
    pub keys: Vec<Key>,
    pub modifiers: Modifiers,
}

impl fmt::Display for KeyCombination {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            self.keys
                .iter()
                .filter_map(|key| {
                    if let Key::Character(c) = key {
                        Some(*c)
                    } else {
                        None
                    }
                })
                .collect::<String>()
        )
    }
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
        let mut parts = s.split(':');
        let first_part = parts.next().ok_or("Invalid key combination")?;
        let key_comb_str;
        let mode;

        if let Ok(parsed_mode) = match s.to_lowercase().as_str() {
            "normal" => Ok(Mode::Normal),
            "hint" => Ok(Mode::Hint),
            _ => Err(format!("Invalid mode: {}", s)),
        } {
            mode = parsed_mode;
            key_comb_str = parts.next().ok_or("Missing key combination")?;
        } else {
            mode = Mode::Normal;
            key_comb_str = first_part;
        }

        let mut key_parts = key_comb_str.split('+');
        let mut modifiers = Modifiers::default();
        let key_str = key_parts.next_back().ok_or("Invalid key combination")?;

        key_parts.try_for_each(|part| {
            match part.to_lowercase().as_str() {
                "ctrl" => modifiers.control = true,
                "alt" => modifiers.alt = true,
                "meta" => modifiers.meta = true,
                _ => return Err(format!("Invalid modifier: {}", part)),
            }
            Ok(())
        })?;

        let keys: Vec<Key> = match key_str {
            "<CR>" => vec![Key::SpecialKey(SpecialKeyCode::Enter)],
            "<BS>" => vec![Key::SpecialKey(SpecialKeyCode::Backspace)],
            "<tab>" => vec![Key::SpecialKey(SpecialKeyCode::Tab)],
            "<leader>" => vec![Key::SpecialKey(SpecialKeyCode::Space)],
            "<Esc>" => vec![Key::SpecialKey(SpecialKeyCode::Escape)],
            "<Up>" => vec![Key::SpecialKey(SpecialKeyCode::Up)],
            "<Left>" => vec![Key::SpecialKey(SpecialKeyCode::Left)],
            "<Right>" => vec![Key::SpecialKey(SpecialKeyCode::Right)],
            "<Down>" => vec![Key::SpecialKey(SpecialKeyCode::Down)],
            _ => key_str.chars().map(Key::Character).collect(),
        };

        Ok(KeyCombination {
            mode,
            modifiers,
            keys,
        })
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
    ) -> Option<Self> {
        let key_name = xkb_state.key_get_one_sym(keycode);

        match key_name {
            Keysym::Return => Some(Key::SpecialKey(SpecialKeyCode::Enter)),
            Keysym::BackSpace => Some(Key::SpecialKey(SpecialKeyCode::Backspace)),
            Keysym::Tab => Some(Key::SpecialKey(SpecialKeyCode::Tab)),
            Keysym::Escape => Some(Key::SpecialKey(SpecialKeyCode::Escape)),
            Keysym::space => Some(Key::SpecialKey(SpecialKeyCode::Space)),
            Keysym::uparrow => Some(Key::SpecialKey(SpecialKeyCode::Up)),
            Keysym::downarrow => Some(Key::SpecialKey(SpecialKeyCode::Down)),
            Keysym::leftarrow => Some(Key::SpecialKey(SpecialKeyCode::Left)),
            Keysym::rightarrow => Some(Key::SpecialKey(SpecialKeyCode::Right)),
            _ => {
                let key_sym = xkb_state.key_get_one_sym(keycode);
                if u32::from(key_sym) == xkbcommon::xkb::keysyms::KEY_NoSymbol {
                    return None;
                }
                let key_char_code = xkb_state.key_get_utf32(keycode);
                char::from_u32(key_char_code).map(Key::Character)
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
    Up,
    Down,
    Left,
    Right,
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
    HintMode,
    NormalMode,
    Mute,
    Unmute,
    ToggleMute,
    Ihibit,
    Uninhibit,
    ToggleInhibit,
    ShowHistory,
    HideHistory,
    ToggleHistory,
}

#[derive(Deserialize, PartialEq, Eq, Hash, Debug, Default, Clone)]
pub struct Modifiers {
    pub control: bool,
    pub alt: bool,
    pub meta: bool,
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
            KeyCombination::from_str(&key_str).map_err(serde::de::Error::custom)?;
        keymaps.insert(key_combination, action);
    }

    for (kc, default_action) in default_key_combinations() {
        if !keymaps.values().any(|action| *action == default_action) {
            keymaps.insert(kc, default_action);
        }
    }

    Ok(keymaps)
}
