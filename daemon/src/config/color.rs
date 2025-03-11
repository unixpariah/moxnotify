use serde::{Deserialize, Deserializer};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Color {
    rgba: [u8; 4],
}

impl Color {
    pub fn rgba(rgba: [u8; 4]) -> Self {
        Color { rgba }
    }
}

impl From<Color> for glyphon::Color {
    fn from(value: Color) -> Self {
        Self::rgba(value.rgba[0], value.rgba[1], value.rgba[2], value.rgba[3])
    }
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
        let test_cases = [
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
