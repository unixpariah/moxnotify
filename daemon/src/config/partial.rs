use super::Size;
use crate::config::color::{parse_hex, Color};
use serde::{
    de::{self, MapAccess, Visitor},
    Deserialize, Deserializer,
};
use std::{fmt, str::FromStr};

#[derive(Deserialize, Default)]
pub struct PartialStyle {
    pub background: Option<PartialColor>,
    pub min_width: Option<Size>,
    pub width: Option<Size>,
    pub max_width: Option<Size>,
    pub min_height: Option<Size>,
    pub height: Option<Size>,
    pub max_height: Option<Size>,
    pub font: Option<PartialFont>,
    pub border: Option<PartialBorder>,
    pub margin: Option<PartialInsets>,
    pub padding: Option<PartialInsets>,
}

#[derive(Debug, Clone, Default)]
pub struct PartialColor {
    pub urgency_low: Option<[u8; 4]>,
    pub urgency_normal: Option<[u8; 4]>,
    pub urgency_critical: Option<[u8; 4]>,
}

impl<'de> Deserialize<'de> for PartialColor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PartialColorVisitor;

        impl<'de> Visitor<'de> for PartialColorVisitor {
            type Value = PartialColor;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str(
                    "a hex color string, comma-separated hex colors, or a map with urgency keys",
                )
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let color = Color::from_str(v).map_err(de::Error::custom)?;
                Ok(PartialColor {
                    urgency_low: Some(color.urgency_low),
                    urgency_normal: Some(color.urgency_normal),
                    urgency_critical: Some(color.urgency_critical),
                })
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut partial_color = PartialColor::default();
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "urgency_low" => {
                            let hex = map.next_value::<String>()?;
                            let color = parse_hex(&hex).map_err(de::Error::custom)?;
                            partial_color.urgency_low = Some(color);
                        }
                        "urgency_normal" => {
                            let hex = map.next_value::<String>()?;
                            let color = parse_hex(&hex).map_err(de::Error::custom)?;
                            partial_color.urgency_normal = Some(color);
                        }
                        "urgency_critical" => {
                            let hex = map.next_value::<String>()?;
                            let color = parse_hex(&hex).map_err(de::Error::custom)?;
                            partial_color.urgency_critical = Some(color);
                        }
                        _ => {
                            return Err(de::Error::unknown_field(
                                &key,
                                &["urgency_low", "urgency_normal", "urgency_critical"],
                            ))
                        }
                    }
                }
                Ok(partial_color)
            }
        }

        deserializer.deserialize_any(PartialColorVisitor)
    }
}

#[derive(Deserialize)]
pub struct PartialFont {
    pub size: Option<f32>,
    pub family: Option<Box<str>>,
    pub color: Option<PartialColor>,
}

#[derive(Default, Clone, Copy, Deserialize)]
pub struct PartialInsets {
    pub left: Option<f32>,
    pub right: Option<f32>,
    pub top: Option<f32>,
    pub bottom: Option<f32>,
}

#[derive(Deserialize)]
pub struct PartialBorder {
    pub size: Option<PartialInsets>,
    pub radius: Option<PartialBorderRadius>,
    pub color: Option<PartialColor>,
}

#[derive(Deserialize, Default, Clone, Copy)]
pub struct PartialBorderRadius {
    pub top_left: Option<f32>,
    pub top_right: Option<f32>,
    pub bottom_left: Option<f32>,
    pub bottom_right: Option<f32>,
}

#[derive(Deserialize)]
pub struct PartialIcon {
    pub border: Option<PartialBorder>,
}
