//! Scrolling text marquee for the iDotMatrix display.
//!
//! Rasterizes a message with the built-in 5x7 font and scrolls it right-to-left
//! across the panel. Supports a solid foreground color or a travelling rainbow,
//! and a `--preview` mode that animates in the terminal with no hardware.
//!
//! ```sh
//! cargo run --example text -- "HELLO WORLD" --rainbow
//! cargo run --example text -- "12:34" --color cyan --preview
//! ```

use anyhow::Result;
use btleplug::api::WriteType;
use clap::Parser;
use idotmatrix::{
    Color, DiyMode, Frame, IdotMatrix, MatrixSize, PixelOrder, SendOptions, StreamOptions,
    TextMask, preview,
};
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[derive(Debug, Parser)]
struct Args {
    /// Message to scroll. Lowercase is folded to uppercase by the font.
    message: String,

    #[arg(long)]
    device: Option<String>,

    #[arg(long, default_value_t = 16)]
    width: usize,

    #[arg(long, default_value_t = 16)]
    height: usize,

    /// Foreground color (ignored when --rainbow is set).
    #[arg(long, default_value = "#ff8000")]
    color: Color,

    #[arg(long, default_value = "#000000")]
    background: Color,

    /// Color each column by a travelling rainbow instead of a solid color.
    #[arg(long)]
    rainbow: bool,

    /// Columns scrolled per second.
    #[arg(long, default_value_t = 12.0)]
    speed: f64,

    /// Display refresh rate; higher is smoother but sends more BLE frames.
    #[arg(long, default_value_t = 30.0)]
    fps: f64,

    /// Blank columns inserted between glyphs.
    #[arg(long, default_value_t = 1)]
    letter_spacing: usize,

    /// Number of full passes to scroll. 0 loops forever.
    #[arg(long, default_value_t = 0)]
    loops: u64,

    /// Render to the terminal instead of connecting over BLE.
    #[arg(long)]
    preview: bool,

    #[arg(long, default_value = "rgb")]
    order: PixelOrder,

    #[arg(long, default_value_t = 244)]
    chunk_size: usize,

    #[arg(long, default_value_t = 1)]
    delay_ms: u64,

    #[arg(long, default_value_t = 1)]
    diy_mode: u8,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let size = MatrixSize::new(args.width, args.height);
    let mask = TextMask::render(&args.message, args.letter_spacing);

    // Scroll the text from fully off the right edge to fully off the left edge.
    let travel = mask.width() as isize + size.width as isize;
    let voffset = (size.height as isize - mask.height() as isize) / 2;
    let frame_period = Duration::from_secs_f64(1.0 / args.fps.max(1.0));

    let mut renderer = Renderer::connect(&args, size).await?;

    let start = Instant::now();
    let mut passes = 0u64;
    let mut last_scroll = -1isize;
    loop {
        let elapsed = start.elapsed().as_secs_f64();
        // Sub-pixel scroll position; the fractional part drives the rainbow phase.
        let scrolled = elapsed * args.speed;
        let scroll = (scrolled as isize) % travel;

        if scroll < last_scroll {
            passes += 1;
            if args.loops != 0 && passes >= args.loops {
                break;
            }
        }
        last_scroll = scroll;

        let phase = elapsed as f32;
        let frame = Frame::from_fn(size, |x, y| {
            let mx = scroll - size.width as isize + x as isize;
            let my = y as isize - voffset;
            if mask.get(mx, my) {
                if args.rainbow {
                    let hue = (mx as f32 * 0.03 + phase * 0.15).rem_euclid(1.0);
                    hsv_to_rgb(hue, 1.0, 1.0)
                } else {
                    args.color
                }
            } else {
                args.background
            }
        });

        renderer.show(&frame).await?;
        sleep(frame_period).await;
    }

    renderer.finish();
    Ok(())
}

/// Either a BLE frame streamer or a terminal preview sink.
enum Renderer<'a> {
    Ble(idotmatrix::FrameStreamer<'a>),
    Preview { first: bool },
}

impl Renderer<'static> {
    async fn connect(args: &Args, _size: MatrixSize) -> Result<Renderer<'static>> {
        if args.preview {
            preview::cursor_visible(false);
            return Ok(Renderer::Preview { first: true });
        }
        // Leak the matrix so the streamer can borrow it for 'static; the process
        // owns it for its whole lifetime anyway.
        let matrix: &'static IdotMatrix = Box::leak(Box::new(
            IdotMatrix::connect(args.device.as_deref(), Duration::from_secs(5)).await?,
        ));
        let send = SendOptions {
            pixel_order: args.order,
            chunk_size: args.chunk_size,
            chunk_delay: Duration::from_millis(args.delay_ms),
            write_type: WriteType::WithoutResponse,
            ..SendOptions::default()
        };
        let stream = matrix
            .stream(StreamOptions {
                send,
                diy_mode: DiyMode::from(args.diy_mode),
                diy_refresh: Some(Duration::from_secs(3)),
            })
            .await?;
        Ok(Renderer::Ble(stream))
    }
}

impl Renderer<'_> {
    async fn show(&mut self, frame: &Frame) -> Result<()> {
        match self {
            Renderer::Ble(stream) => stream.send(frame).await,
            Renderer::Preview { first } => {
                preview::print_in_place(frame, *first);
                *first = false;
                Ok(())
            }
        }
    }

    fn finish(&self) {
        if matches!(self, Renderer::Preview { .. }) {
            preview::cursor_visible(true);
        }
    }
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Color {
    let h = h.rem_euclid(1.0) * 6.0;
    let i = h.floor() as i32;
    let f = h - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match i {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Color::new((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}
