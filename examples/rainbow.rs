use anyhow::Result;
use btleplug::api::WriteType;
use clap::Parser;
use idotmatrix::{
    Color, DiyMode, Frame, IdotMatrix, MatrixSize, PixelOrder, SendOptions, StreamOptions,
};
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    device: Option<String>,

    #[arg(long, default_value_t = 16)]
    width: usize,

    #[arg(long, default_value_t = 16)]
    height: usize,

    #[arg(long, default_value_t = 30.0)]
    fps: f64,

    #[arg(long, default_value_t = 0)]
    frames: u64,

    #[arg(long, default_value = "rgb")]
    order: PixelOrder,

    #[arg(long, default_value_t = 244)]
    chunk_size: usize,

    #[arg(long, default_value_t = 1)]
    delay_ms: u64,

    #[arg(long, default_value_t = 1)]
    diy_mode: u8,

    #[arg(long, default_value_t = 3.0)]
    diy_refresh_secs: f64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let size = MatrixSize::new(args.width, args.height);
    let matrix = IdotMatrix::connect(args.device.as_deref(), Duration::from_secs(5)).await?;
    let options = SendOptions {
        pixel_order: args.order,
        chunk_size: args.chunk_size,
        chunk_delay: Duration::from_millis(args.delay_ms),
        write_type: WriteType::WithoutResponse,
        ..SendOptions::default()
    };
    let mut stream = matrix
        .stream(StreamOptions {
            send: options,
            diy_mode: DiyMode::from(args.diy_mode),
            diy_refresh: refresh_duration(args.diy_refresh_secs),
        })
        .await?;
    let frame_period = Duration::from_secs_f64(1.0 / args.fps.max(1.0));

    let mut frame_index = 0u64;
    loop {
        if args.frames != 0 && frame_index >= args.frames {
            break;
        }
        let started = Instant::now();
        let t = frame_index as f32 * 0.12;
        let frame = Frame::from_fn(size, |x, y| {
            let phase = (x as f32 * 0.33 + y as f32 * 0.21 + t).sin();
            let hue = (phase * 0.5 + 0.5 + t * 0.03) % 1.0;
            hsv_to_rgb(hue, 1.0, 1.0)
        });
        stream.send(&frame).await?;
        frame_index += 1;
        let elapsed = started.elapsed();
        if frame_period > elapsed {
            sleep(frame_period - elapsed).await;
        }
    }
    Ok(())
}

fn refresh_duration(seconds: f64) -> Option<Duration> {
    (seconds > 0.0).then(|| Duration::from_secs_f64(seconds))
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Color {
    let h = (h.rem_euclid(1.0)) * 6.0;
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
