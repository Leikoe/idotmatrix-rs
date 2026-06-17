use anyhow::Result;
use clap::Parser;
use idotmatrix::image::frame_from_dynamic_image;
use idotmatrix::{DiyMode, IdotMatrix, MatrixSize, PixelOrder, SendOptions};
use image::imageops::FilterType;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Parser)]
struct Args {
    path: PathBuf,

    #[arg(long)]
    device: Option<String>,

    #[arg(long, default_value_t = 16)]
    width: usize,

    #[arg(long, default_value_t = 16)]
    height: usize,

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
    let image = image::open(&args.path)?;
    let frame = frame_from_dynamic_image(&image, size, FilterType::Nearest);

    let matrix = IdotMatrix::connect(args.device.as_deref(), Duration::from_secs(5)).await?;
    let options = SendOptions {
        pixel_order: args.order,
        chunk_size: args.chunk_size,
        chunk_delay: Duration::from_millis(args.delay_ms),
        diy_mode: DiyMode::from(args.diy_mode),
        ..SendOptions::default()
    };
    matrix.send_frame(&frame, &options).await?;
    Ok(())
}
