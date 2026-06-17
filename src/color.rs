use anyhow::{Result, anyhow};
use std::fmt;
use std::str::FromStr;

/// 8-bit RGB color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };
    pub const RED: Self = Self { r: 255, g: 0, b: 0 };
    pub const GREEN: Self = Self { r: 0, g: 255, b: 0 };
    pub const BLUE: Self = Self { r: 0, g: 0, b: 255 };
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
    };

    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const fn from_rgb(rgb: [u8; 3]) -> Self {
        Self {
            r: rgb[0],
            g: rgb[1],
            b: rgb[2],
        }
    }

    pub fn bytes(self, order: PixelOrder) -> [u8; 3] {
        match order {
            PixelOrder::Rgb => [self.r, self.g, self.b],
            PixelOrder::Rbg => [self.r, self.b, self.g],
            PixelOrder::Grb => [self.g, self.r, self.b],
            PixelOrder::Gbr => [self.g, self.b, self.r],
            PixelOrder::Brg => [self.b, self.r, self.g],
            PixelOrder::Bgr => [self.b, self.g, self.r],
        }
    }

    pub fn scale(self, brightness: f32) -> Self {
        let brightness = brightness.clamp(0.0, 1.0);
        Self {
            r: (self.r as f32 * brightness) as u8,
            g: (self.g as f32 * brightness) as u8,
            b: (self.b as f32 * brightness) as u8,
        }
    }

    pub fn lerp(self, other: Color, alpha: f32) -> Color {
        let alpha = alpha.clamp(0.0, 1.0);
        Color {
            r: (self.r as f32 + (other.r as f32 - self.r as f32) * alpha) as u8,
            g: (self.g as f32 + (other.g as f32 - self.g as f32) * alpha) as u8,
            b: (self.b as f32 + (other.b as f32 - self.b as f32) * alpha) as u8,
        }
    }
}

impl From<[u8; 3]> for Color {
    fn from(rgb: [u8; 3]) -> Self {
        Self::from_rgb(rgb)
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

impl FromStr for Color {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self> {
        let value = match input.trim().to_ascii_lowercase().as_str() {
            "black" => "000000".to_string(),
            "white" => "ffffff".to_string(),
            "red" => "ff0000".to_string(),
            "green" => "00ff00".to_string(),
            "blue" => "0000ff".to_string(),
            "yellow" => "ffff00".to_string(),
            "cyan" => "00ffff".to_string(),
            "magenta" => "ff00ff".to_string(),
            other => other.trim_start_matches('#').to_string(),
        };
        if value.len() != 6 || !value.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(anyhow!("color must be #RRGGBB or a basic color name"));
        }
        let raw = u32::from_str_radix(&value, 16)?;
        Ok(Color {
            r: ((raw >> 16) & 0xff) as u8,
            g: ((raw >> 8) & 0xff) as u8,
            b: (raw & 0xff) as u8,
        })
    }
}

/// Byte order used when serializing pixels for a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PixelOrder {
    #[default]
    Rgb,
    Rbg,
    Grb,
    Gbr,
    Brg,
    Bgr,
}

impl FromStr for PixelOrder {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self> {
        match input.to_ascii_lowercase().as_str() {
            "rgb" => Ok(PixelOrder::Rgb),
            "rbg" => Ok(PixelOrder::Rbg),
            "grb" => Ok(PixelOrder::Grb),
            "gbr" => Ok(PixelOrder::Gbr),
            "brg" => Ok(PixelOrder::Brg),
            "bgr" => Ok(PixelOrder::Bgr),
            _ => Err(anyhow!(
                "pixel order must be one of rgb/rbg/grb/gbr/brg/bgr"
            )),
        }
    }
}
