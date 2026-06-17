//! Animated GIF player: resize any `.gif` to the panel and stream its frames.
//!
//! The whole internet becomes panel content — pixel-art loops, sprite anims,
//! fireplaces, memes. Frames are decoded once and streamed as DIY pixels with
//! the GIF's own per-frame timing (override with `--fps`). `--preview` plays it
//! in the terminal with no hardware.
//!
//! ```sh
//! cargo run --example gif -- ./fire.gif
//! cargo run --example gif -- ./parrot.gif --preview
//! cargo run --example gif -- ./logo.gif --fps 15 --loops 3 --filter triangle
//! ```

use anyhow::{Context, Result};
use btleplug::api::WriteType;
use clap::Parser;
use idotmatrix::image::{Fit, frames_from_gif};
use idotmatrix::{
    DiyMode, Frame, IdotMatrix, MatrixSize, PixelOrder, SendOptions, StreamOptions, preview,
};
use image::imageops::FilterType;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[derive(Debug, Parser)]
struct Args {
    /// Path to the .gif file.
    path: PathBuf,

    #[arg(long)]
    device: Option<String>,

    #[arg(long, default_value_t = 16)]
    width: usize,

    #[arg(long, default_value_t = 16)]
    height: usize,

    /// Resize filter: nearest (crisp pixel-art), triangle, catmull-rom, gaussian,
    /// or lanczos3 (smooth).
    #[arg(long, default_value = "nearest")]
    filter: ScaleFilter,

    /// How to fit non-square sources: cover (fill+crop), contain (letterbox),
    /// or stretch (distort to fill).
    #[arg(long, default_value = "cover")]
    fit: FitMode,

    /// Override every frame to this rate. 0 uses the GIF's own per-frame delays.
    #[arg(long, default_value_t = 0.0)]
    fps: f64,

    /// Floor for per-frame delay in ms; stops 0-delay GIFs from running too fast.
    #[arg(long, default_value_t = 20)]
    min_delay_ms: u64,

    /// Number of times to play through. 0 loops forever.
    #[arg(long, default_value_t = 0)]
    loops: u64,

    /// Render to the terminal instead of connecting over BLE.
    #[arg(long)]
    preview: bool,

    #[arg(long, default_value = "rgb")]
    order: PixelOrder,

    #[arg(long, default_value_t = 244)]
    chunk_size: usize,

    /// Inter-chunk BLE delay in ms.
    #[arg(long, default_value_t = 1)]
    delay_ms: u64,

    #[arg(long, default_value_t = 1)]
    diy_mode: u8,
}

#[derive(Debug, Clone, Copy)]
struct FitMode(Fit);

impl std::str::FromStr for FitMode {
    type Err = anyhow::Error;
    fn from_str(input: &str) -> Result<Self> {
        Ok(FitMode(match input.to_ascii_lowercase().as_str() {
            "cover" | "crop" => Fit::Cover,
            "contain" | "fit" | "letterbox" => Fit::Contain,
            "stretch" | "fill" => Fit::Stretch,
            other => anyhow::bail!("unknown fit '{other}' (cover|contain|stretch)"),
        }))
    }
}

#[derive(Debug, Clone, Copy)]
struct ScaleFilter(FilterType);

impl std::str::FromStr for ScaleFilter {
    type Err = anyhow::Error;
    fn from_str(input: &str) -> Result<Self> {
        Ok(ScaleFilter(match input.to_ascii_lowercase().as_str() {
            "nearest" => FilterType::Nearest,
            "triangle" => FilterType::Triangle,
            "catmull-rom" | "catmullrom" | "cubic" => FilterType::CatmullRom,
            "gaussian" => FilterType::Gaussian,
            "lanczos3" | "lanczos" => FilterType::Lanczos3,
            other => anyhow::bail!("unknown filter '{other}'"),
        }))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let size = MatrixSize::new(args.width, args.height);

    let reader = BufReader::new(
        File::open(&args.path).with_context(|| format!("opening {}", args.path.display()))?,
    );
    let frames = frames_from_gif(reader, size, args.filter.0, args.fit.0)?;
    if frames.is_empty() {
        anyhow::bail!("GIF decoded to zero frames");
    }

    let min_delay = Duration::from_millis(args.min_delay_ms);
    let fixed = (args.fps > 0.0).then(|| Duration::from_secs_f64(1.0 / args.fps));
    println!(
        "{} frames from {} ({}x{}); playing {}",
        frames.len(),
        args.path.display(),
        size.width,
        size.height,
        match args.loops {
            0 => "forever".to_string(),
            n => format!("{n} time(s)"),
        }
    );

    let mut renderer = Renderer::connect(&args, size).await?;

    let mut played = 0u64;
    'outer: loop {
        for animation in &frames {
            let started = Instant::now();
            renderer.show(&animation.frame).await?;
            let hold = fixed.unwrap_or(animation.delay).max(min_delay);
            let elapsed = started.elapsed();
            if hold > elapsed {
                sleep(hold - elapsed).await;
            }
        }
        played += 1;
        if args.loops != 0 && played >= args.loops {
            break 'outer;
        }
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
        // Leak the matrix so the streamer can borrow it for the process lifetime.
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
