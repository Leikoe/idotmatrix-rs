//! A compact 5x7 bitmap font and text rasterizer.
//!
//! The matrix has no built-in glyphs, so this module ships a hand-drawn 5x7
//! font covering the printable ASCII range that matters for short messages:
//! digits, uppercase letters (lowercase is folded to uppercase), and common
//! punctuation. [`TextMask`] rasterizes a string into a 1-bit-per-pixel mask
//! that callers scroll across the display to build a marquee.

/// Width of a single glyph cell, in pixels.
pub const GLYPH_WIDTH: usize = 5;
/// Height of a single glyph cell, in pixels.
pub const GLYPH_HEIGHT: usize = 7;

/// Returns the 7 row bitmaps for `c`, MSB-of-low-5-bits is the leftmost column.
///
/// Lowercase letters are folded to uppercase. Unknown characters render as `?`.
pub fn glyph_rows(c: char) -> [u8; GLYPH_HEIGHT] {
    let c = c.to_ascii_uppercase();
    GLYPHS
        .iter()
        .find(|(glyph, _)| *glyph == c)
        .or_else(|| GLYPHS.iter().find(|(glyph, _)| *glyph == '?'))
        .map(|(_, rows)| *rows)
        .unwrap_or([0; GLYPH_HEIGHT])
}

/// Returns `true` when this font has a dedicated glyph for `c` (after
/// uppercase folding), so callers can skip characters that would render as `?`.
pub fn has_glyph(c: char) -> bool {
    let c = c.to_ascii_uppercase();
    GLYPHS.iter().any(|(glyph, _)| *glyph == c)
}

/// A rasterized string: a 1-bit-per-pixel mask `GLYPH_HEIGHT` rows tall and as
/// wide as the text requires, including inter-letter spacing.
#[derive(Debug, Clone)]
pub struct TextMask {
    width: usize,
    cells: Vec<bool>,
}

impl TextMask {
    /// Rasterizes `text` with `letter_spacing` blank columns between glyphs.
    ///
    /// A space character renders as a blank glyph-width gap. The returned mask
    /// has no leading or trailing padding; add display-width padding while
    /// scrolling if you want the text to enter and exit cleanly.
    pub fn render(text: &str, letter_spacing: usize) -> Self {
        let mut columns: Vec<u8> = Vec::new();
        for (i, c) in text.chars().enumerate() {
            if i > 0 {
                columns.extend(std::iter::repeat(0).take(letter_spacing));
            }
            if c == ' ' {
                columns.extend(std::iter::repeat(0).take(GLYPH_WIDTH));
                continue;
            }
            let rows = glyph_rows(c);
            for col in 0..GLYPH_WIDTH {
                let bit = GLYPH_WIDTH - 1 - col;
                let mut packed = 0u8;
                for (row, row_bits) in rows.iter().enumerate() {
                    if (row_bits >> bit) & 1 == 1 {
                        packed |= 1 << row;
                    }
                }
                columns.push(packed);
            }
        }

        let width = columns.len();
        let mut cells = vec![false; width * GLYPH_HEIGHT];
        for (x, packed) in columns.iter().enumerate() {
            for y in 0..GLYPH_HEIGHT {
                cells[y * width + x] = (packed >> y) & 1 == 1;
            }
        }
        Self { width, cells }
    }

    /// Total width of the rasterized text in pixels.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Height of the mask, always [`GLYPH_HEIGHT`].
    pub fn height(&self) -> usize {
        GLYPH_HEIGHT
    }

    /// Returns whether the pixel at `(x, y)` is lit. Out-of-range is `false`,
    /// which makes scrolling past either edge render as blank columns.
    pub fn get(&self, x: isize, y: isize) -> bool {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= GLYPH_HEIGHT {
            return false;
        }
        self.cells[y as usize * self.width + x as usize]
    }
}

/// Glyph table: each entry is `(char, rows)` where `rows` lists the 7 scanlines
/// top-to-bottom and the low 5 bits of each select the lit columns left-to-right.
#[rustfmt::skip]
const GLYPHS: &[(char, [u8; GLYPH_HEIGHT])] = &[
    (' ', [0; 7]),
    ('0', [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110]),
    ('1', [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]),
    ('2', [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111]),
    ('3', [0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110]),
    ('4', [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010]),
    ('5', [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110]),
    ('6', [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110]),
    ('7', [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000]),
    ('8', [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110]),
    ('9', [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100]),
    ('A', [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001]),
    ('B', [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110]),
    ('C', [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110]),
    ('D', [0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100]),
    ('E', [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111]),
    ('F', [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000]),
    ('G', [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111]),
    ('H', [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001]),
    ('I', [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]),
    ('J', [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100]),
    ('K', [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001]),
    ('L', [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111]),
    ('M', [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001]),
    ('N', [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001]),
    ('O', [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]),
    ('P', [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000]),
    ('Q', [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101]),
    ('R', [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001]),
    ('S', [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110]),
    ('T', [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100]),
    ('U', [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]),
    ('V', [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100]),
    ('W', [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001]),
    ('X', [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001]),
    ('Y', [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100]),
    ('Z', [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111]),
    ('.', [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00110, 0b00110]),
    (',', [0b00000, 0b00000, 0b00000, 0b00000, 0b00110, 0b00100, 0b01000]),
    (':', [0b00000, 0b00110, 0b00110, 0b00000, 0b00110, 0b00110, 0b00000]),
    (';', [0b00000, 0b00110, 0b00110, 0b00000, 0b00110, 0b00100, 0b01000]),
    ('!', [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100]),
    ('?', [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b00000, 0b00100]),
    ('\'', [0b00100, 0b00100, 0b01000, 0b00000, 0b00000, 0b00000, 0b00000]),
    ('"', [0b01010, 0b01010, 0b01010, 0b00000, 0b00000, 0b00000, 0b00000]),
    ('-', [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000]),
    ('+', [0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000]),
    ('=', [0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000]),
    ('*', [0b00000, 0b00100, 0b10101, 0b01110, 0b10101, 0b00100, 0b00000]),
    ('/', [0b00001, 0b00010, 0b00100, 0b00100, 0b00100, 0b01000, 0b10000]),
    ('\\', [0b10000, 0b01000, 0b00100, 0b00100, 0b00100, 0b00010, 0b00001]),
    ('(', [0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010]),
    (')', [0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000]),
    ('<', [0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010]),
    ('>', [0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000]),
    ('#', [0b01010, 0b01010, 0b11111, 0b01010, 0b11111, 0b01010, 0b01010]),
    ('@', [0b01110, 0b10001, 0b10111, 0b10101, 0b10111, 0b10000, 0b01110]),
    ('&', [0b01100, 0b10010, 0b10100, 0b01000, 0b10101, 0b10010, 0b01101]),
    ('$', [0b00100, 0b01111, 0b10100, 0b01110, 0b00101, 0b11110, 0b00100]),
    ('%', [0b11001, 0b11010, 0b00100, 0b01011, 0b10011, 0b00000, 0b00000]),
    ('^', [0b00100, 0b01010, 0b10001, 0b00000, 0b00000, 0b00000, 0b00000]),
    ('_', [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111]),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_known_width() {
        // "AB" -> 5 + 1 (spacing) + 5 = 11 columns at spacing 1.
        let mask = TextMask::render("AB", 1);
        assert_eq!(mask.width(), 11);
        assert_eq!(mask.height(), GLYPH_HEIGHT);
    }

    #[test]
    fn space_is_blank_glyph_width() {
        let mask = TextMask::render(" ", 1);
        assert_eq!(mask.width(), GLYPH_WIDTH);
        assert!(
            (0..GLYPH_WIDTH as isize).all(|x| (0..GLYPH_HEIGHT as isize).all(|y| !mask.get(x, y)))
        );
    }

    #[test]
    fn letter_a_has_lit_pixels() {
        let mask = TextMask::render("A", 0);
        // Top row of 'A' (0b01110) lights columns 1..=3.
        assert!(!mask.get(0, 0));
        assert!(mask.get(1, 0));
        assert!(mask.get(2, 0));
        assert!(mask.get(3, 0));
        assert!(!mask.get(4, 0));
    }

    #[test]
    fn unknown_char_falls_back_to_question_mark() {
        assert_eq!(glyph_rows('~'), glyph_rows('?'));
        assert!(!has_glyph('~'));
        assert!(has_glyph('a'));
    }

    #[test]
    fn out_of_range_reads_blank() {
        let mask = TextMask::render("A", 0);
        assert!(!mask.get(-1, 0));
        assert!(!mask.get(0, -1));
        assert!(!mask.get(100, 0));
    }
}
