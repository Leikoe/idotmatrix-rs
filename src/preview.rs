//! Terminal preview of a [`Frame`] using truecolor half-block characters.
//!
//! Each text cell renders two vertically-stacked pixels: the upper-half block
//! `▀` draws the top pixel as its foreground color and the bottom pixel as its
//! background color. This halves the row count, so a 16x16 frame previews as 16
//! columns by 8 rows of characters — handy for developing effects without the
//! physical display in hand.

use crate::color::Color;
use crate::frame::Frame;
use std::fmt::Write as _;

const ESC: &str = "\x1b[";

/// Renders `frame` to a string of ANSI half-block characters.
///
/// The string ends with a reset sequence and a newline per character row.
pub fn render(frame: &Frame) -> String {
    let size = frame.size();
    let mut out = String::new();
    let mut y = 0;
    while y < size.height {
        for x in 0..size.width {
            let top = frame.get(x, y).unwrap_or(Color::BLACK);
            let bottom = if y + 1 < size.height {
                frame.get(x, y + 1).unwrap_or(Color::BLACK)
            } else {
                Color::BLACK
            };
            let _ = write!(
                out,
                "{ESC}38;2;{};{};{}m{ESC}48;2;{};{};{}m▀",
                top.r, top.g, top.b, bottom.r, bottom.g, bottom.b
            );
        }
        let _ = write!(out, "{ESC}0m\n");
        y += 2;
    }
    out
}

/// Number of terminal rows [`render`] emits for `frame` (one per two pixels).
pub fn rows(frame: &Frame) -> usize {
    frame.size().height.div_ceil(2)
}

/// Prints `frame` in place, moving the cursor back up so successive frames
/// animate as an overlay rather than scrolling the terminal.
///
/// Pass `first` as `true` for the initial frame (no cursor rewind) and `false`
/// thereafter. Returns the number of rows drawn so callers can pass it back.
pub fn print_in_place(frame: &Frame, first: bool) {
    use std::io::Write as _;
    let mut stdout = std::io::stdout();
    if !first {
        // Move the cursor up to the top-left of the previously drawn frame.
        let _ = write!(stdout, "{ESC}{}A\r", rows(frame));
    }
    let _ = stdout.write_all(render(frame).as_bytes());
    let _ = stdout.flush();
}

/// Shows or hides the terminal cursor; call with `false` before an animation
/// loop and `true` when finished so the cursor does not flicker over the frame.
pub fn cursor_visible(visible: bool) {
    use std::io::Write as _;
    let mut stdout = std::io::stdout();
    let _ = write!(stdout, "{ESC}?25{}", if visible { "h" } else { "l" });
    let _ = stdout.flush();
}
