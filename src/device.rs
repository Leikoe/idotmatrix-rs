use crate::color::PixelOrder;
use crate::frame::Frame;
use crate::protocol::{
    DEFAULT_CHUNK_DELAY, DEFAULT_CHUNK_SIZE, DiyMode, MaterialOptions, WRITE_UUID,
    build_diy_packets, build_material_packets, enter_diy_command, split_chunks,
};
use crate::text::{TextStyle, build_text_payload};
use anyhow::{Context, Result, anyhow};
use btleplug::api::{
    Central, CharPropFlags, Characteristic, Manager as _, Peripheral as _, ScanFilter, WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ConnectOptions<'a> {
    pub selector: Option<&'a str>,
    pub timeout: Duration,
}

impl<'a> Default for ConnectOptions<'a> {
    fn default() -> Self {
        Self {
            selector: None,
            timeout: Duration::from_secs(5),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    pub id: String,
    pub name: Option<String>,
    pub rssi: Option<i16>,
    pub likely_idotmatrix: bool,
}

#[derive(Debug, Clone)]
pub struct SendOptions {
    pub pixel_order: PixelOrder,
    pub chunk_size: usize,
    pub chunk_delay: Duration,
    pub write_type: WriteType,
    pub enter_diy: bool,
    pub diy_mode: DiyMode,
}

impl Default for SendOptions {
    fn default() -> Self {
        Self {
            pixel_order: PixelOrder::Rgb,
            chunk_size: DEFAULT_CHUNK_SIZE,
            chunk_delay: DEFAULT_CHUNK_DELAY,
            write_type: WriteType::WithoutResponse,
            enter_diy: true,
            diy_mode: DiyMode::ClearCurrent,
        }
    }
}

impl SendOptions {
    pub fn animation() -> Self {
        Self {
            enter_diy: false,
            ..Self::default()
        }
    }

    pub fn with_pixel_order(mut self, order: PixelOrder) -> Self {
        self.pixel_order = order;
        self
    }

    pub fn with_chunk_size(mut self, chunk_size: usize) -> Self {
        self.chunk_size = chunk_size.max(1);
        self
    }

    pub fn with_chunk_delay(mut self, delay: Duration) -> Self {
        self.chunk_delay = delay;
        self
    }

    pub fn with_write_type(mut self, write_type: WriteType) -> Self {
        self.write_type = write_type;
        self
    }

    pub fn with_enter_diy(mut self, mode: impl Into<DiyMode>) -> Self {
        self.enter_diy = true;
        self.diy_mode = mode.into();
        self
    }

    pub fn without_enter_diy(mut self) -> Self {
        self.enter_diy = false;
        self
    }
}

#[derive(Debug, Clone)]
pub struct StreamOptions {
    pub send: SendOptions,
    pub diy_mode: DiyMode,
    pub diy_refresh: Option<Duration>,
}

impl Default for StreamOptions {
    fn default() -> Self {
        Self {
            send: SendOptions::animation(),
            diy_mode: DiyMode::ClearCurrent,
            diy_refresh: Some(Duration::from_secs(3)),
        }
    }
}

pub struct IdotMatrix {
    peripheral: Peripheral,
    write_char: Characteristic,
}

impl IdotMatrix {
    pub async fn connect(selector: Option<&str>, timeout: Duration) -> Result<Self> {
        Self::connect_with(ConnectOptions { selector, timeout }).await
    }

    pub async fn connect_with(options: ConnectOptions<'_>) -> Result<Self> {
        let adapter = default_adapter().await?;
        let peripheral = find_peripheral(&adapter, options.selector, options.timeout).await?;
        if !peripheral.is_connected().await? {
            peripheral.connect().await.context("connect failed")?;
        }
        peripheral
            .discover_services()
            .await
            .context("discover_services failed")?;

        let write_uuid = Uuid::parse_str(WRITE_UUID)?;
        let write_char = peripheral
            .characteristics()
            .into_iter()
            .find(|c| {
                c.uuid == write_uuid
                    && (c.properties.contains(CharPropFlags::WRITE)
                        || c.properties.contains(CharPropFlags::WRITE_WITHOUT_RESPONSE))
            })
            .ok_or_else(|| anyhow!("write characteristic {WRITE_UUID} not found"))?;

        Ok(Self {
            peripheral,
            write_char,
        })
    }

    pub async fn enter_diy(&self, mode: impl Into<DiyMode>) -> Result<()> {
        self.write_diy_command(mode.into()).await?;
        // Let the firmware settle into DIY mode before the first frame arrives.
        sleep(Duration::from_millis(200)).await;
        Ok(())
    }

    /// Writes the enter-DIY command without the settle delay. Used for the
    /// periodic mid-stream reassert, where a 200ms pause would stutter playback.
    async fn write_diy_command(&self, mode: DiyMode) -> Result<()> {
        let command = enter_diy_command(mode);
        if self
            .peripheral
            .write(&self.write_char, &command, WriteType::WithResponse)
            .await
            .is_err()
        {
            self.peripheral
                .write(&self.write_char, &command, WriteType::WithoutResponse)
                .await
                .context("enter DIY command failed")?;
        }
        Ok(())
    }

    pub async fn stream(&self, options: StreamOptions) -> Result<FrameStreamer<'_>> {
        self.enter_diy(options.diy_mode).await?;
        Ok(FrameStreamer {
            matrix: self,
            options: options.send.without_enter_diy(),
            // Mid-stream refreshes only reassert DIY mode; they must not clear
            // the screen (which would flash black) the way the initial enter can.
            refresh_mode: DiyMode::NoClearCurrent,
            diy_refresh: options.diy_refresh,
            last_diy: Instant::now(),
        })
    }

    pub async fn send_frame(&self, frame: &Frame, options: &SendOptions) -> Result<()> {
        if options.enter_diy {
            self.enter_diy(options.diy_mode).await?;
        }
        self.send_frame_pixels(&frame.to_bytes(options.pixel_order), options)
            .await
    }

    /// Uploads `text` for the firmware to render and animate on its own.
    ///
    /// Unlike [`send_frame`](Self::send_frame), this does not stream pixels: the
    /// glyphs and a style block are sent once and the panel handles the
    /// scrolling/blinking effect. The text stays on screen until replaced.
    pub async fn send_text(
        &self,
        text: &str,
        style: &TextStyle,
        options: &SendOptions,
    ) -> Result<()> {
        let payload = build_text_payload(text, style);
        let packets = build_material_packets(&payload, MaterialOptions::text());
        self.send_packets(&packets, options).await
    }

    pub async fn send_material(
        &self,
        data: &[u8],
        material: MaterialOptions,
        options: &SendOptions,
    ) -> Result<()> {
        let packets = build_material_packets(data, material);
        self.send_packets(&packets, options).await
    }

    pub async fn send_frame_pixels(&self, pixels: &[u8], options: &SendOptions) -> Result<()> {
        let packets = build_diy_packets(pixels);
        self.send_packets(&packets, options).await
    }

    async fn send_packets(&self, packets: &[Vec<u8>], options: &SendOptions) -> Result<()> {
        let chunks = split_chunks(packets, options.chunk_size);
        for chunk in chunks {
            self.peripheral
                .write(&self.write_char, &chunk, options.write_type)
                .await
                .context("frame write failed")?;
            if !options.chunk_delay.is_zero() {
                sleep(options.chunk_delay).await;
            }
        }
        Ok(())
    }

    pub fn raw_peripheral(&self) -> &Peripheral {
        &self.peripheral
    }

    pub async fn is_connected(&self) -> Result<bool> {
        Ok(self.peripheral.is_connected().await?)
    }
}

pub struct FrameStreamer<'a> {
    matrix: &'a IdotMatrix,
    options: SendOptions,
    refresh_mode: DiyMode,
    diy_refresh: Option<Duration>,
    last_diy: Instant,
}

impl<'a> FrameStreamer<'a> {
    pub async fn send(&mut self, frame: &Frame) -> Result<()> {
        if self
            .diy_refresh
            .is_some_and(|refresh| self.last_diy.elapsed() >= refresh)
        {
            self.matrix.write_diy_command(self.refresh_mode).await?;
            self.last_diy = Instant::now();
        }
        self.matrix.send_frame(frame, &self.options).await
    }

    pub fn options(&self) -> &SendOptions {
        &self.options
    }

    pub fn options_mut(&mut self) -> &mut SendOptions {
        &mut self.options
    }
}

pub async fn scan(timeout: Duration) -> Result<Vec<DiscoveredDevice>> {
    let adapter = default_adapter().await?;
    adapter.start_scan(ScanFilter::default()).await?;
    sleep(timeout).await;
    let peripherals = adapter.peripherals().await?;
    let mut out = Vec::new();
    for peripheral in peripherals {
        let properties = peripheral.properties().await?;
        let name = properties.as_ref().and_then(|p| p.local_name.clone());
        let id = peripheral.id().to_string();
        let rssi = properties.as_ref().and_then(|p| p.rssi);
        let likely_idotmatrix = looks_like_idotmatrix(&id, name.as_deref());
        out.push(DiscoveredDevice {
            id,
            name,
            rssi,
            likely_idotmatrix,
        });
    }
    out.sort_by_key(|d| (!d.likely_idotmatrix, d.name.clone(), d.id.clone()));
    Ok(out)
}

async fn default_adapter() -> Result<Adapter> {
    let manager = Manager::new().await.context("create BLE manager failed")?;
    manager
        .adapters()
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("no BLE adapter found"))
}

async fn find_peripheral(
    adapter: &Adapter,
    selector: Option<&str>,
    timeout: Duration,
) -> Result<Peripheral> {
    adapter.start_scan(ScanFilter::default()).await?;
    sleep(timeout).await;
    let peripherals = adapter.peripherals().await?;
    let selector = selector.map(str::to_ascii_lowercase);

    let mut candidates = Vec::new();
    for peripheral in peripherals {
        let properties = peripheral.properties().await?;
        let name = properties.as_ref().and_then(|p| p.local_name.clone());
        let id = peripheral.id().to_string();
        let matches = match selector.as_deref() {
            Some(selector) => {
                id.to_ascii_lowercase().contains(selector)
                    || name
                        .as_deref()
                        .unwrap_or("")
                        .to_ascii_lowercase()
                        .contains(selector)
            }
            None => looks_like_idotmatrix(&id, name.as_deref()),
        };
        if matches {
            candidates.push(peripheral);
        }
    }

    candidates
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("no iDotMatrix device found; run the scan example first"))
}

fn looks_like_idotmatrix(id: &str, name: Option<&str>) -> bool {
    let haystack = format!("{} {}", id, name.unwrap_or("")).to_ascii_lowercase();
    ["idot", "idm", "dotmatrix", "matrix", "tech"]
        .iter()
        .any(|token| haystack.contains(token))
}
