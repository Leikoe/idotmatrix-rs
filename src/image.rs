use crate::{Color, Frame, MatrixSize};
use anyhow::{Context, Result};
use image::codecs::gif::GifDecoder;
use image::imageops::FilterType;
use image::{AnimationDecoder, DynamicImage};
use std::io::{BufRead, Seek};
use std::time::Duration;

pub fn frame_from_dynamic_image(
    image: &DynamicImage,
    size: MatrixSize,
    filter: FilterType,
) -> Frame {
    let resized = image
        .resize_exact(size.width as u32, size.height as u32, filter)
        .to_rgb8();
    Frame::from_fn(size, |x, y| {
        let pixel = resized.get_pixel(x as u32, y as u32);
        Color::new(pixel[0], pixel[1], pixel[2])
    })
}

/// How to map a non-square (or differently-sized) source image onto the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Fit {
    /// Scale each axis independently to fill the panel exactly (may distort).
    Stretch,
    /// Scale to fill the panel preserving aspect ratio, cropping the overflow.
    /// Best default for arbitrary internet GIFs on a square panel.
    #[default]
    Cover,
    /// Scale to fit inside the panel preserving aspect ratio, letterboxing the
    /// remainder with black. Shows the whole frame.
    Contain,
}

/// Maps `image` onto a `size` frame using the given `fit` strategy.
pub fn frame_with_fit(
    image: &DynamicImage,
    size: MatrixSize,
    filter: FilterType,
    fit: Fit,
) -> Frame {
    let (w, h) = (size.width as u32, size.height as u32);
    match fit {
        Fit::Stretch => frame_from_dynamic_image(image, size, filter),
        Fit::Cover => {
            let filled = image.resize_to_fill(w, h, filter).to_rgb8();
            Frame::from_fn(size, |x, y| {
                let p = filled.get_pixel(x as u32, y as u32);
                Color::new(p[0], p[1], p[2])
            })
        }
        Fit::Contain => {
            let resized = image.resize(w, h, filter).to_rgb8();
            let (rw, rh) = resized.dimensions();
            let ox = (w - rw) / 2;
            let oy = (h - rh) / 2;
            Frame::from_fn(size, |x, y| {
                let (sx, sy) = (x as u32, y as u32);
                if sx >= ox && sx < ox + rw && sy >= oy && sy < oy + rh {
                    let p = resized.get_pixel(sx - ox, sy - oy);
                    Color::new(p[0], p[1], p[2])
                } else {
                    Color::BLACK
                }
            })
        }
    }
}

/// A single decoded animation frame: the resized matrix frame plus the GIF's
/// requested on-screen duration for it.
#[derive(Debug, Clone)]
pub struct AnimationFrame {
    pub frame: Frame,
    pub delay: Duration,
}

/// Decodes an animated GIF from `reader`, compositing and mapping each frame to
/// `size` with the given `fit`. Frame disposal/blending is handled by the
/// decoder, so every returned frame is a complete picture. Per-frame delays
/// come straight from the GIF.
pub fn frames_from_gif<R: BufRead + Seek>(
    reader: R,
    size: MatrixSize,
    filter: FilterType,
    fit: Fit,
) -> Result<Vec<AnimationFrame>> {
    let decoder = GifDecoder::new(reader).context("not a readable GIF")?;
    let frames = decoder
        .into_frames()
        .collect_frames()
        .context("failed to decode GIF frames")?;

    let mut out = Vec::with_capacity(frames.len());
    for frame in frames {
        let (numer, denom) = frame.delay().numer_denom_ms();
        let millis = if denom == 0 { 0 } else { numer / denom };
        let image = DynamicImage::ImageRgba8(frame.into_buffer());
        out.push(AnimationFrame {
            frame: frame_with_fit(&image, size, filter, fit),
            delay: Duration::from_millis(millis as u64),
        });
    }
    Ok(out)
}
