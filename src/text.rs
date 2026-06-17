//! Native firmware text protocol.
//!
//! Unlike the DIY pixel path, the device firmware can render and *animate* a
//! short string on its own: you upload the glyph bitmaps and a style block
//! once, and the panel scrolls/blinks/breathes it without further BLE traffic.
//! This is the protocol the vendor app uses for its "Text" screen, recovered
//! from `TextAgreement.sendTextTo1616` and `Text1664` in the decompiled APK.
//!
//! The payload built here is the *inner* buffer; wrap it for transport with
//! [`crate::protocol::build_material_packets`] using [`MaterialOptions::text`],
//! which frames it as a packet-type-3 (text) material at the live display slot.
//!
//! [`MaterialOptions::text`]: crate::protocol::MaterialOptions::text

use crate::color::Color;
use crate::font::{self, GLYPH_HEIGHT, GLYPH_WIDTH};
use anyhow::{Result, anyhow};
use std::str::FromStr;

/// Panel height marker the firmware uses to pick its glyph layout. `1` selects
/// the 16-row layout used by 16x16 (and 16x16-derived) panels.
const PANEL_TYPE_16: u8 = 1;
/// Every glyph cell is 16 rows tall regardless of horizontal scale.
const CELL_HEIGHT: usize = 16;

/// How the firmware animates the uploaded text.
///
/// Values mirror the vendor app's effect menu order for 16x16 panels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextEffect {
    /// Static; long text is clipped rather than scrolled.
    Fixed,
    /// Scroll right-to-left (the classic marquee).
    ScrollLeft,
    /// Scroll left-to-right.
    ScrollRight,
    /// Scroll bottom-to-top.
    ScrollUp,
    /// Scroll top-to-bottom.
    ScrollDown,
    /// Blink on and off.
    Blink,
    /// Fade in and out.
    Breathe,
    /// Snowflake reveal.
    Snowflake,
    /// Laser draw-on.
    Laser,
    /// Any other effect byte the firmware understands.
    Raw(u8),
}

impl TextEffect {
    /// The protocol byte for this effect.
    pub const fn as_byte(self) -> u8 {
        match self {
            Self::Fixed => 0,
            Self::ScrollLeft => 1,
            Self::ScrollRight => 2,
            Self::ScrollUp => 3,
            Self::ScrollDown => 4,
            Self::Blink => 5,
            Self::Breathe => 6,
            Self::Snowflake => 7,
            Self::Laser => 8,
            Self::Raw(value) => value,
        }
    }
}

impl FromStr for TextEffect {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self> {
        Ok(match input.trim().to_ascii_lowercase().as_str() {
            "fixed" | "fixation" | "static" => Self::Fixed,
            "left" | "scroll-left" | "scrollleft" => Self::ScrollLeft,
            "right" | "scroll-right" | "scrollright" => Self::ScrollRight,
            "up" | "scroll-up" | "scrollup" => Self::ScrollUp,
            "down" | "scroll-down" | "scrolldown" => Self::ScrollDown,
            "blink" => Self::Blink,
            "breathe" | "breath" => Self::Breathe,
            "snowflake" | "snow" => Self::Snowflake,
            "laser" => Self::Laser,
            other => {
                let value: u8 = other
                    .parse()
                    .map_err(|_| anyhow!("unknown text effect '{other}'"))?;
                Self::Raw(value)
            }
        })
    }
}

/// Styling for a native text upload: effect, speed, and the text/background
/// colors. Defaults to a white right-to-left marquee on a black background.
#[derive(Debug, Clone, Copy)]
pub struct TextStyle {
    /// Firmware animation effect.
    pub effect: TextEffect,
    /// Animation speed, `0..=100`. Higher is faster. The app defaults to 85.
    pub speed: u8,
    /// Foreground color used when `color_mode` is solid (`1`).
    pub color: Color,
    /// Color mode byte. `1` paints every glyph in `color`; the firmware also
    /// supports gradient/palette modes at higher values.
    pub color_mode: u8,
    /// Background color used when `background_mode` is on (`1`).
    pub background: Color,
    /// Background mode byte. `0` leaves the background transparent (black); `1`
    /// fills it with `background`.
    pub background_mode: u8,
    /// Horizontal glyph scale: `1` packs glyphs into 8px-wide cells (ticker
    /// style, several characters visible), `2` into 16px-wide cells (one bold
    /// character that fills the panel height).
    pub scale: u8,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            effect: TextEffect::ScrollLeft,
            speed: 85,
            color: Color::WHITE,
            color_mode: 1,
            background: Color::BLACK,
            background_mode: 0,
            scale: 1,
        }
    }
}

impl TextStyle {
    /// Builder: set the animation effect.
    pub fn with_effect(mut self, effect: TextEffect) -> Self {
        self.effect = effect;
        self
    }

    /// Builder: set the foreground color (and solid color mode).
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self.color_mode = 1;
        self
    }

    /// Builder: set the animation speed, clamped to `0..=100`.
    pub fn with_speed(mut self, speed: u8) -> Self {
        self.speed = speed.min(100);
        self
    }

    /// Builder: fill the background with `color` instead of leaving it dark.
    pub fn with_background(mut self, color: Color) -> Self {
        self.background = color;
        self.background_mode = 1;
        self
    }

    /// Builder: set the horizontal glyph scale (clamped to `1..=2`).
    pub fn with_scale(mut self, scale: u8) -> Self {
        self.scale = scale.clamp(1, 2);
        self
    }
}

/// Builds the inner text payload the firmware parses: a 14-byte style header
/// followed by one entry per glyph (width marker, per-glyph RGB, then the
/// 16-row-tall column bitmap).
///
/// Pass the result to [`crate::protocol::build_material_packets`] with
/// [`MaterialOptions::text`](crate::protocol::MaterialOptions::text). Characters
/// without a glyph fall back to `?`.
pub fn build_text_payload(text: &str, style: &TextStyle) -> Vec<u8> {
    let scale = style.scale.clamp(1, 2) as usize;

    let mut glyphs = Vec::new();
    let mut count: u16 = 0;
    for ch in text.chars() {
        let bytes = render_glyph(ch, scale);
        // 16 bytes => 8px-wide cell (marker 2); 32 bytes => 16px-wide (marker 3).
        let marker = if bytes.len() == 16 { 2 } else { 3 };
        glyphs.push(marker);
        // Per-glyph color; the app sends white and lets the style header drive
        // the actual color via `color_mode`.
        glyphs.extend_from_slice(&[0xFF, 0xFF, 0xFF]);
        glyphs.extend_from_slice(&bytes);
        count = count.saturating_add(1);
    }

    // Guard against an all-black text color, which the firmware treats as unset.
    let mut blue = style.color.b;
    if style.color.r == 0 && style.color.g == 0 && style.color.b == 0 {
        blue = 1;
    }

    let mut payload = Vec::with_capacity(14 + glyphs.len());
    payload.extend_from_slice(&count.to_le_bytes());
    payload.push(PANEL_TYPE_16);
    payload.push(1);
    payload.push(style.effect.as_byte());
    payload.push(style.speed.min(100));
    payload.push(style.color_mode);
    payload.extend_from_slice(&[style.color.r, style.color.g, blue]);
    payload.push(style.background_mode);
    payload.extend_from_slice(&[style.background.r, style.background.g, style.background.b]);
    payload.extend_from_slice(&glyphs);
    payload
}

/// Renders one character into the device's glyph format: a `CELL_HEIGHT`-row,
/// row-major bitmap where the LSB of each byte is the leftmost pixel and each
/// row uses `cell_width / 8` bytes. The 5x7 font glyph is centered in the cell
/// at the requested integer `scale`.
fn render_glyph(ch: char, scale: usize) -> Vec<u8> {
    let cell_width = if scale <= 1 { 8 } else { 16 };
    let rows = font::glyph_rows(ch);
    let glyph_w = GLYPH_WIDTH * scale;
    let glyph_h = GLYPH_HEIGHT * scale;
    let ox = (cell_width - glyph_w) / 2;
    let oy = (CELL_HEIGHT - glyph_h) / 2;

    let lit = |x: usize, y: usize| -> bool {
        if x < ox || y < oy {
            return false;
        }
        let gx = (x - ox) / scale;
        let gy = (y - oy) / scale;
        if gx >= GLYPH_WIDTH || gy >= GLYPH_HEIGHT {
            return false;
        }
        let bit = GLYPH_WIDTH - 1 - gx;
        (rows[gy] >> bit) & 1 == 1
    };

    let bytes_per_row = cell_width / 8;
    let mut out = Vec::with_capacity(CELL_HEIGHT * bytes_per_row);
    for y in 0..CELL_HEIGHT {
        for bx in 0..bytes_per_row {
            let mut byte = 0u8;
            for bit in 0..8 {
                if lit(bx * 8 + bit, y) {
                    byte |= 1 << bit;
                }
            }
            out.push(byte);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{MaterialOptions, build_material_packets};

    #[test]
    fn effect_bytes_match_app_order() {
        assert_eq!(TextEffect::Fixed.as_byte(), 0);
        assert_eq!(TextEffect::ScrollLeft.as_byte(), 1);
        assert_eq!(TextEffect::Laser.as_byte(), 8);
        assert_eq!(TextEffect::Raw(42).as_byte(), 42);
    }

    #[test]
    fn glyph_is_16_bytes_at_scale_1_and_32_at_scale_2() {
        assert_eq!(render_glyph('A', 1).len(), 16);
        assert_eq!(render_glyph('A', 2).len(), 32);
    }

    #[test]
    fn payload_header_matches_protocol() {
        let style = TextStyle::default()
            .with_effect(TextEffect::ScrollLeft)
            .with_speed(50)
            .with_color(Color::new(10, 20, 30));
        let payload = build_text_payload("AB", &style);

        // Two glyphs, little-endian count.
        assert_eq!(&payload[0..2], &2u16.to_le_bytes());
        assert_eq!(payload[2], PANEL_TYPE_16);
        assert_eq!(payload[3], 1);
        assert_eq!(payload[4], 1); // ScrollLeft
        assert_eq!(payload[5], 50); // speed
        assert_eq!(payload[6], 1); // solid color mode
        assert_eq!(&payload[7..10], &[10, 20, 30]); // text color
        assert_eq!(payload[10], 0); // background mode off
        assert_eq!(&payload[11..14], &[0, 0, 0]); // background color

        // Each scale-1 glyph entry is marker(1) + rgb(3) + bytes(16) = 20 bytes.
        assert_eq!(payload.len(), 14 + 2 * 20);
        assert_eq!(payload[14], 2); // narrow glyph marker
        assert_eq!(&payload[15..18], &[0xFF, 0xFF, 0xFF]);
    }

    #[test]
    fn all_black_text_color_is_nudged_to_visible() {
        let style = TextStyle::default().with_color(Color::BLACK);
        let payload = build_text_payload("A", &style);
        assert_eq!(&payload[7..10], &[0, 0, 1]);
    }

    #[test]
    fn wraps_into_a_type_3_text_material_packet() {
        let payload = build_text_payload("HI", &TextStyle::default());
        let packets = build_material_packets(&payload, MaterialOptions::text());
        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0][2], 3); // packet type = text
        assert_eq!(packets[0][15], 12); // live-display slot index
        assert_eq!(&packets[0][16..], payload.as_slice());
    }
}
