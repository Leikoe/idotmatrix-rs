use std::time::Duration;

pub const SERVICE_UUID: &str = "000000fa-0000-1000-8000-00805f9b34fb";
pub const WRITE_UUID: &str = "0000fa02-0000-1000-8000-00805f9b34fb";
pub const NOTIFY_UUID: &str = "0000fa03-0000-1000-8000-00805f9b34fb";

pub const DEFAULT_CHUNK_SIZE: usize = 244;
pub const DEFAULT_CHUNK_DELAY: Duration = Duration::from_millis(1);
pub const APP_PACKET_PAYLOAD_SIZE: usize = 4096;

pub const ENTER_DIY_CLEAR_CURRENT: u8 = 1;
pub const ENTER_DIY_NO_CLEAR_CURRENT: u8 = 3;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DiyMode {
    #[default]
    ClearCurrent,
    NoClearCurrent,
    Raw(u8),
}

impl DiyMode {
    pub const fn as_byte(self) -> u8 {
        match self {
            Self::ClearCurrent => ENTER_DIY_CLEAR_CURRENT,
            Self::NoClearCurrent => ENTER_DIY_NO_CLEAR_CURRENT,
            Self::Raw(value) => value,
        }
    }
}

impl From<u8> for DiyMode {
    fn from(value: u8) -> Self {
        match value {
            ENTER_DIY_CLEAR_CURRENT => Self::ClearCurrent,
            ENTER_DIY_NO_CLEAR_CURRENT => Self::NoClearCurrent,
            other => Self::Raw(other),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialKind {
    Gif,
    Image,
    /// Native firmware text. The payload is built by
    /// [`crate::text::build_text_payload`].
    Text,
}

impl MaterialKind {
    const fn packet_type(self) -> u8 {
        match self {
            Self::Gif => 1,
            Self::Image => 2,
            Self::Text => 3,
        }
    }
}

/// Live-display material slot the vendor app uses to push to the current screen
/// rather than a saved bank.
pub const LIVE_DISPLAY_SLOT: u8 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaterialOptions {
    pub kind: MaterialKind,
    pub slot: u8,
    pub display_time: u16,
}

impl MaterialOptions {
    pub const fn gif(slot: u8) -> Self {
        Self {
            kind: MaterialKind::Gif,
            slot,
            display_time: 0,
        }
    }

    pub const fn image(slot: u8) -> Self {
        Self {
            kind: MaterialKind::Image,
            slot,
            display_time: 0,
        }
    }

    /// Options for a native text upload pushed to the live display.
    pub const fn text() -> Self {
        Self {
            kind: MaterialKind::Text,
            slot: LIVE_DISPLAY_SLOT,
            display_time: 0,
        }
    }
}

pub fn build_diy_packets(pixel_bytes: &[u8]) -> Vec<Vec<u8>> {
    let total = pixel_bytes.len() as u32;
    let mut packets = Vec::new();

    for (index, part) in pixel_bytes.chunks(APP_PACKET_PAYLOAD_SIZE).enumerate() {
        let packet_len = (part.len() + 9) as u16;
        let marker = if index == 0 { 0 } else { 2 };
        let mut packet = Vec::with_capacity(part.len() + 9);
        packet.extend_from_slice(&packet_len.to_le_bytes());
        packet.extend_from_slice(&[0, 0, marker]);
        packet.extend_from_slice(&total.to_le_bytes());
        packet.extend_from_slice(part);
        packets.push(packet);
    }

    packets
}

pub fn build_material_packets(data: &[u8], options: MaterialOptions) -> Vec<Vec<u8>> {
    let total = data.len() as u32;
    let crc = crc32fast::hash(data);
    let mut packets = Vec::new();

    for (index, part) in data.chunks(APP_PACKET_PAYLOAD_SIZE).enumerate() {
        let packet_len = (part.len() + 16) as u16;
        let marker = if index == 0 { 0 } else { 2 };
        let mut packet = Vec::with_capacity(part.len() + 16);
        packet.extend_from_slice(&packet_len.to_le_bytes());
        packet.push(options.kind.packet_type());
        packet.push(0);
        packet.push(marker);
        packet.extend_from_slice(&total.to_le_bytes());
        packet.extend_from_slice(&crc.to_le_bytes());
        packet.extend_from_slice(&options.display_time.to_le_bytes());
        packet.push(options.slot);
        packet.extend_from_slice(part);
        packets.push(packet);
    }

    packets
}

pub fn split_chunks(packets: &[Vec<u8>], chunk_size: usize) -> Vec<Vec<u8>> {
    packets
        .iter()
        .flat_map(|packet| packet.chunks(chunk_size.max(1)).map(Vec::from))
        .collect()
}

pub fn enter_diy_command(mode: impl Into<DiyMode>) -> [u8; 5] {
    [5, 0, 4, 1, mode.into().as_byte()]
}

pub fn looks_like_diy_ack(data: &[u8]) -> bool {
    data.len() >= 5 && data[1] == 0 && data[2] == 0 && data[3] == 0 && matches!(data[4], 0 | 1)
}

pub fn looks_like_next_diy_packet_ack(data: &[u8]) -> bool {
    data.len() >= 5 && data[1] == 0 && data[2] == 0 && data[3] == 0 && data[4] == 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_single_16x16_diy_packet() {
        let pixels = vec![0xab; 16 * 16 * 3];
        let packets = build_diy_packets(&pixels);
        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0].len(), 777);
        assert_eq!(&packets[0][0..2], &777u16.to_le_bytes());
        assert_eq!(&packets[0][2..5], &[0, 0, 0]);
        assert_eq!(&packets[0][5..9], &(768u32).to_le_bytes());
        assert_eq!(&packets[0][9..], pixels.as_slice());
    }

    #[test]
    fn splits_large_diy_payload_into_continuations() {
        let pixels = vec![0xcd; 5000];
        let packets = build_diy_packets(&pixels);
        assert_eq!(packets.len(), 2);
        assert_eq!(packets[0][4], 0);
        assert_eq!(packets[1][4], 2);
        assert_eq!(&packets[0][5..9], &(5000u32).to_le_bytes());
        assert_eq!(&packets[1][5..9], &(5000u32).to_le_bytes());
    }

    #[test]
    fn diy_mode_command_matches_app() {
        assert_eq!(enter_diy_command(DiyMode::ClearCurrent), [5, 0, 4, 1, 1]);
        assert_eq!(enter_diy_command(DiyMode::NoClearCurrent), [5, 0, 4, 1, 3]);
        assert_eq!(enter_diy_command(9), [5, 0, 4, 1, 9]);
    }

    #[test]
    fn builds_material_upload_packet_like_app() {
        let data = b"GIF89a tiny fake data";
        let packets = build_material_packets(data, MaterialOptions::gif(12));
        assert_eq!(packets.len(), 1);
        assert_eq!(&packets[0][0..2], &((data.len() + 16) as u16).to_le_bytes());
        assert_eq!(packets[0][2], 1);
        assert_eq!(packets[0][3], 0);
        assert_eq!(packets[0][4], 0);
        assert_eq!(&packets[0][5..9], &(data.len() as u32).to_le_bytes());
        assert_eq!(&packets[0][9..13], &crc32fast::hash(data).to_le_bytes());
        assert_eq!(&packets[0][13..15], &0u16.to_le_bytes());
        assert_eq!(packets[0][15], 12);
        assert_eq!(&packets[0][16..], data);
    }

    #[test]
    fn recognizes_diy_acks_from_app_parser() {
        assert!(looks_like_diy_ack(&[5, 0, 0, 0, 1]));
        assert!(looks_like_diy_ack(&[5, 0, 0, 0, 0]));
        assert!(looks_like_next_diy_packet_ack(&[5, 0, 0, 0, 2]));
        assert!(!looks_like_diy_ack(&[5, 0, 2, 0, 1]));
    }
}
