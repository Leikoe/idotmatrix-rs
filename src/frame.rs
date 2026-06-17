use crate::color::{Color, PixelOrder};
use anyhow::{Result, anyhow};

/// Matrix dimensions in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatrixSize {
    pub width: usize,
    pub height: usize,
}

impl MatrixSize {
    pub const IDOTMATRIX_16X16: Self = Self {
        width: 16,
        height: 16,
    };

    pub const fn new(width: usize, height: usize) -> Self {
        Self { width, height }
    }

    pub const fn pixels(self) -> usize {
        self.width * self.height
    }
}

impl Default for MatrixSize {
    fn default() -> Self {
        Self::IDOTMATRIX_16X16
    }
}

/// Row-major RGB frame buffer.
#[derive(Debug, Clone)]
pub struct Frame {
    size: MatrixSize,
    pixels: Vec<Color>,
}

impl Frame {
    pub fn new(size: MatrixSize, pixels: Vec<Color>) -> Result<Self> {
        if pixels.len() != size.pixels() {
            return Err(anyhow!(
                "frame has {} pixels, expected {}",
                pixels.len(),
                size.pixels()
            ));
        }
        Ok(Self { size, pixels })
    }

    pub fn solid(size: MatrixSize, color: Color) -> Self {
        Self {
            size,
            pixels: vec![color; size.pixels()],
        }
    }

    pub fn from_fn(size: MatrixSize, mut f: impl FnMut(usize, usize) -> Color) -> Self {
        let mut pixels = Vec::with_capacity(size.pixels());
        for y in 0..size.height {
            for x in 0..size.width {
                pixels.push(f(x, y));
            }
        }
        Self { size, pixels }
    }

    pub fn clear(&mut self, color: Color) {
        self.pixels.fill(color);
    }

    pub fn get(&self, x: usize, y: usize) -> Option<Color> {
        self.index(x, y).map(|idx| self.pixels[idx])
    }

    pub fn set(&mut self, x: usize, y: usize, color: Color) -> bool {
        if let Some(idx) = self.index(x, y) {
            self.pixels[idx] = color;
            true
        } else {
            false
        }
    }

    pub fn map(&self, mut f: impl FnMut(usize, usize, Color) -> Color) -> Self {
        Self::from_fn(self.size, |x, y| {
            let idx = y * self.size.width + x;
            f(x, y, self.pixels[idx])
        })
    }

    pub fn size(&self) -> MatrixSize {
        self.size
    }

    pub fn pixels(&self) -> &[Color] {
        &self.pixels
    }

    pub fn pixels_mut(&mut self) -> &mut [Color] {
        &mut self.pixels
    }

    pub fn to_bytes(&self, order: PixelOrder) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.pixels.len() * 3);
        for pixel in &self.pixels {
            out.extend_from_slice(&pixel.bytes(order));
        }
        out
    }

    fn index(&self, x: usize, y: usize) -> Option<usize> {
        (x < self.size.width && y < self.size.height).then_some(y * self.size.width + x)
    }
}
