# idotmatrix

Rust BLE control library for iDotMatrix pixel displays.

This is an experimental library for driving iDotMatrix-style RGB matrix panels
from Rust. The main supported path is live DIY frame streaming: connect to a
panel, enter DIY mode, and send RGB frames from the host.

## Features

- BLE scanning and device selection by name/address substring.
- Live RGB frame streaming for animations.
- Frame helpers for solid colors, images, text, and Conway's Game of Life.
- Example programs for scanning, solid color, images, GIF-style uploads,
  rainbow animation, scrolling text, and Life.
- Low-level packet builders for experimenting with the vendor protocol.

## Quick Start

Scan for panels:

```sh
cargo run --example scan
```

Send a solid color:

```sh
cargo run --example solid -- red
```

Display an image resized to the panel:

```sh
cargo run --example image -- ./picture.png
```

Run scrolling text:

```sh
cargo run --example text -- "HELLO WORLD" --rainbow
```

Run Conway's Game of Life:

```sh
cargo run --example life -- --fps 20 --wrap
```

If auto-detection picks the wrong BLE peripheral, pass a name or address
substring:

```sh
cargo run --example solid -- --device NAME_OR_ADDRESS_SUBSTRING red
```

## Library Usage

```rust
use idotmatrix::{Color, Frame, IdotMatrix, MatrixSize, StreamOptions};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let matrix = IdotMatrix::connect(None, Duration::from_secs(5)).await?;
    let mut stream = matrix.stream(StreamOptions::default()).await?;

    let frame = Frame::solid(MatrixSize::IDOTMATRIX_16X16, Color::GREEN);
    stream.send(&frame).await?;

    Ok(())
}
```

Use `send_frame` for one-off frames. Use `stream` for animations.

## Notes

On macOS, the terminal running Cargo must have Bluetooth permission. The display
may only accept one central connection at a time, so disconnect the vendor app
before connecting from Rust.
