use super::{
    color::Color,
    partial::{PartialBorder, PartialBorderRadius},
    Insets,
};
use serde::{Deserialize, Deserializer};
use std::fmt;

#[derive(Deserialize)]
pub struct Border {
    pub size: Insets,
    pub radius: BorderRadius,
    pub color: Color,
}

impl Border {
    pub fn apply(&mut self, partial: &PartialBorder) {
        if let Some(color) = partial.color.as_ref() {
            self.color.apply(color);
        }
        if let Some(radius) = partial.radius.as_ref() {
            self.radius.apply(radius);
        }
        if let Some(size) = partial.size.as_ref() {
            self.size.apply(size);
        }
    }
}

impl Default for Border {
    fn default() -> Self {
        Self {
            size: Insets {
                left: 1.,
                right: 1.,
                top: 1.,
                bottom: 1.,
            },
            radius: BorderRadius::default(),
            color: Color {
                urgency_low: [158, 206, 106, 255],
                urgency_normal: [187, 154, 247, 255],
                urgency_critical: [192, 202, 245, 255],
            },
        }
    }
}

#[derive(Default, Clone, Copy)]
pub struct BorderRadius {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_left: f32,
    pub bottom_right: f32,
}

impl BorderRadius {
    pub fn apply(&mut self, partial: &PartialBorderRadius) {
        if let Some(top_left) = partial.top_left {
            self.top_left = top_left;
        }
        if let Some(top_right) = partial.top_right {
            self.top_right = top_right;
        }
        if let Some(bottom_left) = partial.bottom_left {
            self.bottom_left = bottom_left;
        }
        if let Some(bottom_right) = partial.bottom_right {
            self.bottom_right = bottom_right;
        }
    }
}
impl<'de> Deserialize<'de> for BorderRadius {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct BorderRadiusVisitor;

        impl<'de> serde::de::Visitor<'de> for BorderRadiusVisitor {
            type Value = BorderRadius;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a number or a map with optional corner values")
            }

            fn visit_f32<E>(self, value: f32) -> Result<Self::Value, E> {
                Ok(BorderRadius {
                    top_left: value,
                    top_right: value,
                    bottom_left: value,
                    bottom_right: value,
                })
            }

            fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E> {
                let value = v as f32;
                Ok(BorderRadius {
                    top_left: value,
                    top_right: value,
                    bottom_left: value,
                    bottom_right: value,
                })
            }

            fn visit_i32<E>(self, v: i32) -> Result<Self::Value, E> {
                let value = v as f32;
                Ok(BorderRadius {
                    top_left: value,
                    top_right: value,
                    bottom_left: value,
                    bottom_right: value,
                })
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> {
                let value = v as f32;
                Ok(BorderRadius {
                    top_left: value,
                    top_right: value,
                    bottom_left: value,
                    bottom_right: value,
                })
            }

            fn visit_u32<E>(self, v: u32) -> Result<Self::Value, E> {
                let value = v as f32;
                Ok(BorderRadius {
                    top_left: value,
                    top_right: value,
                    bottom_left: value,
                    bottom_right: value,
                })
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
                let value = v as f32;
                Ok(BorderRadius {
                    top_left: value,
                    top_right: value,
                    bottom_left: value,
                    bottom_right: value,
                })
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: serde::de::MapAccess<'de>,
            {
                let mut top_left = None;
                let mut top_right = None;
                let mut bottom_left = None;
                let mut bottom_right = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "top_left" => top_left = Some(map.next_value()?),
                        "top_right" => top_right = Some(map.next_value()?),
                        "bottom_left" => bottom_left = Some(map.next_value()?),
                        "bottom_right" => bottom_right = Some(map.next_value()?),
                        _ => {
                            return Err(serde::de::Error::unknown_field(
                                &key,
                                &["top_left", "top_right", "bottom_left", "bottom_right"],
                            ))
                        }
                    }
                }

                Ok(BorderRadius {
                    top_left: top_left.unwrap_or(0.0),
                    top_right: top_right.unwrap_or(0.0),
                    bottom_left: bottom_left.unwrap_or(0.0),
                    bottom_right: bottom_right.unwrap_or(0.0),
                })
            }
        }

        deserializer.deserialize_any(BorderRadiusVisitor)
    }
}

impl BorderRadius {
    pub fn circle() -> Self {
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
