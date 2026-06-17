//! Native firmware text — the protocol the vendor app's "Text" screen uses.
//!
//! Uploads the message once; on panels that support it the firmware renders and
//! animates the text on its own (scroll, blink, breathe, laser, ...) with no
//! per-frame BLE traffic. Contrast with the `text` example, which streams every
//! frame from the host with full RGB control.
//!
//! NOTE: not every panel implements native text rendering. Some panels
//! acknowledge the upload but do not actually draw it, so fall back to the
//! host-rendered `text` example there. Use `text_debug` to see whether your
//! panel accepts/renders the upload.
//!
//! ```sh
//! cargo run --example marquee -- "HELLO" --effect left --color cyan
//! cargo run --example marquee -- "HI" --effect breathe --scale 2 --speed 60
//! ```

use anyhow::Result;
use btleplug::api::WriteType;
use clap::Parser;
use idotmatrix::{Color, IdotMatrix, SendOptions, TextEffect, TextStyle};
use std::time::Duration;

#[derive(Debug, Parser)]
struct Args {
    /// Message to display. Lowercase is folded to uppercase by the font.
    message: String,

    #[arg(long)]
    device: Option<String>,

    /// Animation effect: fixed, left, right, up, down, blink, breathe,
    /// snowflake, laser, or a raw effect byte.
    #[arg(long, default_value = "left")]
    effect: TextEffect,

    /// Animation speed, 0..=100. Higher is faster.
    #[arg(long, default_value_t = 85)]
    speed: u8,

    /// Text color.
    #[arg(long, default_value = "#ffffff")]
    color: Color,

    /// Background color. Off by default; set this to fill the background.
    #[arg(long)]
    background: Option<Color>,

    /// Glyph scale: 1 = ticker (several chars), 2 = bold full-height glyphs.
    #[arg(long, default_value_t = 1)]
    scale: u8,

    #[arg(long, default_value_t = 244)]
    chunk_size: usize,

    #[arg(long, default_value_t = 1)]
    delay_ms: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let mut style = TextStyle::default()
        .with_effect(args.effect)
        .with_speed(args.speed)
        .with_color(args.color)
        .with_scale(args.scale);
    if let Some(background) = args.background {
        style = style.with_background(background);
    }

    let matrix = IdotMatrix::connect(args.device.as_deref(), Duration::from_secs(5)).await?;
    let options = SendOptions {
        chunk_size: args.chunk_size,
        chunk_delay: Duration::from_millis(args.delay_ms),
        write_type: WriteType::WithoutResponse,
        ..SendOptions::default()
    };

    matrix.send_text(&args.message, &style, &options).await?;
    println!(
        "Sent \"{}\" ({:?}, speed {}). If the panel doesn't render native text, \
         use the `text` example instead (host-rendered, works everywhere).",
        args.message, args.effect, args.speed
    );
    Ok(())
}
