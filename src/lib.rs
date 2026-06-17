//! BLE control primitives for iDotMatrix pixel displays.
//!
//! The high-level API focuses on the tested live-DIY path: connect once, put
//! the display into DIY mode, then stream raw RGB frames. Lower-level protocol
//! builders are also exposed for experimenting with saved image/GIF material
//! uploads from the vendor app protocol.

pub mod color;
pub mod device;
pub mod font;
pub mod frame;
#[cfg(feature = "image")]
pub mod image;
pub mod life;
pub mod preview;
pub mod protocol;
pub mod text;

pub use color::{Color, PixelOrder};
pub use device::{
    ConnectOptions, DiscoveredDevice, FrameStreamer, IdotMatrix, SendOptions, StreamOptions, scan,
};
pub use font::TextMask;
pub use frame::{Frame, MatrixSize};
pub use life::Life;
pub use protocol::{
    DEFAULT_CHUNK_DELAY, DEFAULT_CHUNK_SIZE, DiyMode, ENTER_DIY_CLEAR_CURRENT,
    ENTER_DIY_NO_CLEAR_CURRENT, NOTIFY_UUID, SERVICE_UUID, WRITE_UUID, build_diy_packets,
    build_material_packets,
};
pub use text::{TextEffect, TextStyle, build_text_payload};

/// Backward-compatible alias for the original prototype spelling.
pub type IdoMatrix = IdotMatrix;
