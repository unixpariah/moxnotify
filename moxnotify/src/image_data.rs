use fast_image_resize::{self as fr, ResizeOptions};
use image::DynamicImage;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use zbus::zvariant::{Signature, Structure};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub rowstride: i32,
    pub has_alpha: bool,
    pub bits_per_sample: i32,
    pub channels: i32,
    pub data: Vec<u8>,
}

impl ImageData {
    pub fn into_rgba(self, max_size: u32) -> Self {
        let rgba = if self.has_alpha {
            self
        } else {
            let mut data = self.data;
            let mut new_data = Vec::with_capacity(data.len() / self.channels as usize * 4);

            for chunk in data.chunks_exact_mut(self.channels as usize) {
                new_data.extend_from_slice(chunk);
                new_data.push(0xFF);
            }

            Self {
                has_alpha: true,
                data: new_data,
                channels: 4,
                rowstride: self.width as i32 * 4,
                ..self
            }
        };

        let mut src =
            fr::images::Image::from_vec_u8(rgba.width, rgba.height, rgba.data, fr::PixelType::U8x4)
                .unwrap();

        let alpha_mul_div = fr::MulDiv::default();
        alpha_mul_div.multiply_alpha_inplace(&mut src).unwrap();
        let mut dst = fr::images::Image::new(max_size, max_size, fr::PixelType::U8x4);
        let mut resizer = fr::Resizer::new();
        resizer
            .resize(&src, &mut dst, &ResizeOptions::default())
            .unwrap();
        alpha_mul_div.divide_alpha_inplace(&mut dst).unwrap();

        Self {
            width: dst.width(),
            height: dst.height(),
            data: dst.into_vec(),
            ..rgba
        }
    }
}

impl TryFrom<DynamicImage> for ImageData {
    type Error = anyhow::Error;

    fn try_from(value: DynamicImage) -> Result<Self, Self::Error> {
        let rgba_image = value.to_rgba8();

        let width = rgba_image.width();
        let height = rgba_image.height();
        let data = rgba_image.as_raw().to_vec();

        let channels = 4;
        let bits_per_sample = 8;
        let has_alpha = true;
        let rowstride = (width * channels as u32) as i32;

        Ok(Self {
            width,
            height,
            rowstride,
            has_alpha,
            bits_per_sample,
            channels,
            data,
        })
    }
}

impl<'a> TryFrom<Structure<'a>> for ImageData {
    type Error = zbus::Error;

    fn try_from(value: Structure<'a>) -> zbus::Result<Self> {
        if Ok(value.signature()) != Signature::from_str("(iiibiiay)").as_ref() {
            return Err(zbus::Error::Failure(format!(
                "Invalid ImageData: invalid signature {}",
                value.signature().to_string()
            )));
        }

        let mut fields = value.into_fields();

        if fields.len() != 7 {
            return Err(zbus::Error::Failure(
                "Invalid ImageData: missing fields".to_string(),
            ));
        }

        let data = Vec::<u8>::try_from(fields.remove(6))
            .map_err(|e| zbus::Error::Failure(format!("data: {}", e)))?;
        let channels = i32::try_from(fields.remove(5))
            .map_err(|e| zbus::Error::Failure(format!("channels: {}", e)))?;
        let bits_per_sample = i32::try_from(fields.remove(4))
            .map_err(|e| zbus::Error::Failure(format!("bits_per_sample: {}", e)))?;
        let has_alpha = bool::try_from(fields.remove(3))
            .map_err(|e| zbus::Error::Failure(format!("has_alpha: {}", e)))?;
        let rowstride = i32::try_from(fields.remove(2))
            .map_err(|e| zbus::Error::Failure(format!("rowstride: {}", e)))?;
        let height = i32::try_from(fields.remove(1))
            .map_err(|e| zbus::Error::Failure(format!("height: {}", e)))?;
        let width = i32::try_from(fields.remove(0))
            .map_err(|e| zbus::Error::Failure(format!("width: {}", e)))?;

        if width <= 0 {
            return Err(zbus::Error::Failure(
                "Invalid ImageData: width is not positive".to_string(),
            ));
        }

        if height <= 0 {
            return Err(zbus::Error::Failure(
                "Invalid ImageData: height is not positive".to_string(),
            ));
        }

        if bits_per_sample != 8 {
            return Err(zbus::Error::Failure(
                "Invalid ImageData: bits_per_sample is not 8".to_string(),
            ));
        }

        if has_alpha && channels != 4 {
            return Err(zbus::Error::Failure(
                "Invalid ImageData: has_alpha is true but channels is not 4".to_string(),
            ));
        }

        if (width * height * channels) as usize != data.len() {
            return Err(zbus::Error::Failure(
                "Invalid ImageData: data length does not match width * height * channels"
                    .to_string(),
            ));
        }

        if data.len() != (rowstride * height) as usize {
            return Err(zbus::Error::Failure(
                "Invalid ImageData: data length does not match rowstride * height".to_string(),
            ));
        }

        Ok(Self {
            width: width as u32,
            height: height as u32,
            rowstride,
            has_alpha,
            bits_per_sample,
            channels,
            data,
        })
    }
}
