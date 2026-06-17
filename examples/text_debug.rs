//! Diagnostic: send native text and print the device's notify replies.
//!
//! The firmware answers text uploads on the notify characteristic (fa03):
//! `..00 03 00 03` = finished/accepted, `..00 03 00 01` = send next page,
//! `..00 03 00 02` = error (insufficient space / rejected). Watching this tells
//! us whether the packet framing is accepted, instead of guessing from the
//! (always-silent) write path.

use anyhow::{Context, Result, anyhow};
use btleplug::api::{CharPropFlags, Characteristic, Peripheral as _, WriteType};
use clap::Parser;
use futures::StreamExt;
use idotmatrix::protocol::{MaterialOptions, build_material_packets, split_chunks};
use idotmatrix::{Color, IdotMatrix, TextEffect, TextStyle, build_text_payload};
use std::time::Duration;
use tokio::time::{sleep, timeout};
use uuid::Uuid;

const NOTIFY_UUID: &str = "0000fa03-0000-1000-8000-00805f9b34fb";

#[derive(Debug, Parser)]
struct Args {
    #[arg(default_value = "ABC")]
    message: String,

    #[arg(long)]
    device: Option<String>,

    #[arg(long, default_value = "left")]
    effect: TextEffect,

    #[arg(long, default_value_t = 85)]
    speed: u8,

    #[arg(long, default_value = "#ffffff")]
    color: Color,

    #[arg(long, default_value_t = 1)]
    scale: u8,

    /// BLE write chunk size. Try a value larger than the whole packet to send
    /// it in a single ATT write, mirroring the app's MTU-512 behavior.
    #[arg(long, default_value_t = 244)]
    chunk_size: usize,

    /// Use write-with-response (lets CoreBluetooth do long writes > MTU).
    #[arg(long)]
    with_response: bool,

    /// Inter-chunk delay in milliseconds.
    #[arg(long, default_value_t = 20)]
    delay_ms: u64,

    /// Seconds to listen for notifications after sending.
    #[arg(long, default_value_t = 4.0)]
    listen_secs: f64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let style = TextStyle::default()
        .with_effect(args.effect)
        .with_speed(args.speed)
        .with_color(args.color)
        .with_scale(args.scale);

    let payload = build_text_payload(&args.message, &style);
    let packets = build_material_packets(&payload, MaterialOptions::text());
    let chunks = split_chunks(&packets, args.chunk_size);

    println!(
        "payload={} bytes, outer packets={}, ble chunks={} (chunk_size={}, {}, delay={}ms)",
        payload.len(),
        packets.len(),
        chunks.len(),
        args.chunk_size,
        if args.with_response {
            "with-response"
        } else {
            "no-response"
        },
        args.delay_ms,
    );
    for (i, packet) in packets.iter().enumerate() {
        let head: Vec<String> = packet.iter().take(16).map(|b| format!("{b:02x}")).collect();
        println!(
            "  packet[{i}] len={} header=[{}]",
            packet.len(),
            head.join(" ")
        );
    }

    let matrix = IdotMatrix::connect(args.device.as_deref(), Duration::from_secs(5)).await?;
    let peripheral = matrix.raw_peripheral();

    let notify_uuid = Uuid::parse_str(NOTIFY_UUID)?;
    let notify_char: Option<Characteristic> = peripheral
        .characteristics()
        .into_iter()
        .find(|c| c.uuid == notify_uuid && c.properties.contains(CharPropFlags::NOTIFY));
    match &notify_char {
        Some(_) => {
            peripheral
                .subscribe(notify_char.as_ref().unwrap())
                .await
                .context("subscribe to notify failed")?;
            println!("subscribed to notify {NOTIFY_UUID}");
        }
        None => println!("WARNING: notify characteristic not found; will only send"),
    }

    let mut notifications = peripheral.notifications().await?;
    let listener = tokio::spawn(async move {
        while let Some(data) = notifications.next().await {
            let hex: Vec<String> = data.value.iter().map(|b| format!("{b:02x}")).collect();
            let tag = classify(&data.value);
            println!("<- notify {} [{}] {tag}", data.uuid, hex.join(" "));
        }
    });

    let write_char = find_write_char(peripheral)?;
    let write_type = if args.with_response {
        WriteType::WithResponse
    } else {
        WriteType::WithoutResponse
    };
    for (i, chunk) in chunks.iter().enumerate() {
        peripheral
            .write(&write_char, chunk, write_type)
            .await
            .with_context(|| format!("write chunk {i} ({} bytes) failed", chunk.len()))?;
        println!("-> wrote chunk {i} ({} bytes)", chunk.len());
        if args.delay_ms > 0 {
            sleep(Duration::from_millis(args.delay_ms)).await;
        }
    }

    println!("listening {:.1}s for replies...", args.listen_secs);
    let _ = timeout(Duration::from_secs_f64(args.listen_secs), listener).await;
    Ok(())
}

fn classify(data: &[u8]) -> &'static str {
    if data.len() >= 5 && data[1] == 0 && data[2] == 3 && data[3] == 0 {
        match data[4] {
            1 => "= send-next-page",
            2 => "= ERROR (rejected / insufficient space)",
            3 => "= FINISHED (accepted)",
            _ => "= text-reply (unknown sub-code)",
        }
    } else {
        ""
    }
}

fn find_write_char(peripheral: &btleplug::platform::Peripheral) -> Result<Characteristic> {
    let write_uuid = Uuid::parse_str("0000fa02-0000-1000-8000-00805f9b34fb")?;
    peripheral
        .characteristics()
        .into_iter()
        .find(|c| {
            c.uuid == write_uuid
                && (c.properties.contains(CharPropFlags::WRITE)
                    || c.properties.contains(CharPropFlags::WRITE_WITHOUT_RESPONSE))
        })
        .ok_or_else(|| anyhow!("write characteristic not found"))
}
