use anyhow::Result;
use btleplug::api::WriteType;
use clap::Parser;
use idotmatrix::{
    Color, DiyMode, IdotMatrix, Life, MatrixSize, PixelOrder, SendOptions, StreamOptions,
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

    #[arg(long, default_value_t = 20.0)]
    fps: f64,

    #[arg(long, default_value_t = 0)]
    generations: u64,

    #[arg(long, default_value_t = 0.28)]
    density: f32,

    #[arg(long)]
    seed: Option<u64>,

    #[arg(long, default_value = "random")]
    pattern: String,

    #[arg(long, default_value = "#00ff80")]
    color: Color,

    #[arg(long, default_value = "#000000")]
    background: Color,

    #[arg(long, default_value = "rgb")]
    order: PixelOrder,

    #[arg(long, default_value_t = 64)]
    trail_decay: u8,

    #[arg(long)]
    wrap: bool,

    #[arg(long, default_value_t = 244)]
    chunk_size: usize,

    #[arg(long, default_value_t = 1)]
    delay_ms: u64,

    #[arg(long, default_value_t = 1)]
    diy_mode: u8,

    #[arg(long, default_value_t = 3.0)]
    diy_refresh_secs: f64,

    #[arg(long)]
    with_response: bool,

    #[arg(long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let size = MatrixSize::new(args.width, args.height);
    let mut life = if args.pattern == "random" {
        Life::random(size, args.density, args.seed, args.wrap)
    } else {
        Life::pattern(size, &args.pattern, args.wrap)
    };

    let matrix = IdotMatrix::connect(args.device.as_deref(), Duration::from_secs(5)).await?;

    let send = SendOptions {
        pixel_order: args.order,
        chunk_size: args.chunk_size,
        chunk_delay: Duration::from_millis(args.delay_ms),
        write_type: if args.with_response {
            WriteType::WithResponse
        } else {
            WriteType::WithoutResponse
        },
        ..SendOptions::default()
    };
    let mut stream = matrix
        .stream(StreamOptions {
            send,
            diy_mode: DiyMode::from(args.diy_mode),
            diy_refresh: refresh_duration(args.diy_refresh_secs),
        })
        .await?;
    let frame_period = if args.fps > 0.0 {
        Duration::from_secs_f64(1.0 / args.fps)
    } else {
        Duration::ZERO
    };

    let mut sent = 0u64;
    loop {
        if args.generations != 0 && sent >= args.generations {
            break;
        }
        let started = Instant::now();
        let frame = life.frame(args.color, args.background, args.trail_decay);
        stream.send(&frame).await?;
        if args.verbose {
            println!(
                "generation={sent} alive={} send_ms={:.2} connected={}",
                life.alive_count(),
                started.elapsed().as_secs_f64() * 1000.0,
                matrix.is_connected().await?
            );
        }
        life.step();
        life.reset_if_stale(args.density);
        sent += 1;
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
