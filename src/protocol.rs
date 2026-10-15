//! Pure (I/O free) parts of the Linak DPG BLE protocol: UUIDs, command codes and
//! byte encoding/decoding.

use uuid::{Uuid, uuid};

/// GATT characteristic UUIDs used by Linak DPG controllers.
pub mod uuids {
    use super::*;

    pub const DEVICE_NAME: Uuid = uuid!("00002a00-0000-1000-8000-00805f9b34fb");
    pub const MANUFACTURER: Uuid = uuid!("00002a29-0000-1000-8000-00805f9b34fb");
    pub const MODEL_NUMBER: Uuid = uuid!("00002a24-0000-1000-8000-00805f9b34fb");

    /// Control service: accepts move/stop/wakeup commands.
    pub const CONTROL_SERVICE: Uuid = uuid!("99fa0001-338a-1024-8a49-009c0215f78a");
    pub const CONTROL_COMMAND: Uuid = uuid!("99fa0002-338a-1024-8a49-009c0215f78a");
    pub const CONTROL_ERROR: Uuid = uuid!("99fa0003-338a-1024-8a49-009c0215f78a");

    /// DPG service: request/response channel for controller settings.
    pub const DPG_SERVICE: Uuid = uuid!("99fa0010-338a-1024-8a49-009c0215f78a");
    pub const DPG: Uuid = uuid!("99fa0011-338a-1024-8a49-009c0215f78a");

    /// Reference output: current height + speed (read / notify).
    pub const REFERENCE_OUTPUT_SERVICE: Uuid = uuid!("99fa0020-338a-1024-8a49-009c0215f78a");
    pub const REFERENCE_OUTPUT_ONE: Uuid = uuid!("99fa0021-338a-1024-8a49-009c0215f78a");

    /// Reference input: target height for the controller to drive to.
    pub const REFERENCE_INPUT_SERVICE: Uuid = uuid!("99fa0030-338a-1024-8a49-009c0215f78a");
    pub const REFERENCE_INPUT_ONE: Uuid = uuid!("99fa0031-338a-1024-8a49-009c0215f78a");
}

/// Commands written to [`uuids::CONTROL_COMMAND`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ControlCommand {
    MoveDown = 70,
    MoveUp = 71,
    Wakeup = 254,
    Stop = 255,
}

impl ControlCommand {
    pub fn encode(self) -> [u8; 2] {
        [self as u8, 0]
    }
}

/// Commands sent over the [`uuids::DPG`] characteristic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DpgCommand {
    GetCapabilities = 128,
    BaseOffset = 129,
    UserId = 134,
}

const DPG_PREFIX: u8 = 0x7f;
const DPG_READ: u8 = 0x00;
const DPG_WRITE: u8 = 0x80;

/// Encodes a DPG read request.
pub fn encode_dpg_read(cmd: DpgCommand) -> Vec<u8> {
    vec![DPG_PREFIX, cmd as u8, DPG_READ]
}

/// Encodes a DPG write request carrying `data`.
pub fn encode_dpg_write(cmd: DpgCommand, data: &[u8]) -> Vec<u8> {
    let mut buf = vec![DPG_PREFIX, cmd as u8, DPG_WRITE];
    buf.extend_from_slice(data);
    buf
}

/// Decodes a DPG notification. Returns the payload on success, `None` if the
/// controller signalled failure.
pub fn decode_dpg_response(data: &[u8]) -> Option<&[u8]> {
    match data {
        [1, _, payload @ ..] => Some(payload),
        _ => None,
    }
}

/// Raw desk height in 1/10 mm above the lowest position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Height(pub u16);

impl Height {
    /// Absolute height in mm, given the height of the desk at its lowest position.
    pub fn to_mm(self, base_height_mm: u16) -> f32 {
        self.0 as f32 / 10.0 + base_height_mm as f32
    }

    /// Converts an absolute height in mm into a raw height. Returns `None` if the
    /// requested height is below `base_height_mm` or out of range.
    pub fn from_mm(mm: f32, base_height_mm: u16) -> Option<Self> {
        let raw = ((mm - base_height_mm as f32) * 10.0).round();
        (0.0..=u16::MAX as f32)
            .contains(&raw)
            .then_some(Height(raw as u16))
    }

    pub fn encode(self) -> [u8; 2] {
        self.0.to_le_bytes()
    }
}

/// Raw desk speed in 1/100 mm/s. Negative when moving down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Speed(pub i16);

impl Speed {
    pub fn mm_per_s(self) -> f32 {
        self.0 as f32 / 100.0
    }
}

/// A sample from the reference output characteristic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HeightSpeed {
    pub height: Height,
    pub speed: Speed,
}

impl HeightSpeed {
    pub fn decode(data: &[u8]) -> Option<Self> {
        let [h0, h1, s0, s1, ..] = *data else {
            return None;
        };
        Some(Self {
            height: Height(u16::from_le_bytes([h0, h1])),
            speed: Speed(i16::from_le_bytes([s0, s1])),
        })
    }
}

/// Controller capabilities reported via [`DpgCommand::GetCapabilities`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    /// Number of memory positions supported.
    pub memory_size: u8,
    pub auto_up: bool,
    pub auto_down: bool,
    pub ble_allow: bool,
    pub has_display: bool,
    pub has_light: bool,
}

impl Capabilities {
    pub fn decode(payload: &[u8]) -> Option<Self> {
        let (&cap, _) = payload.split_first()?;
        if payload.len() < 2 {
            return None;
        }
        Some(Self {
            memory_size: cap & 0x07,
            auto_up: cap & 0x08 != 0,
            auto_down: cap & 0x10 != 0,
            ble_allow: cap & 0x20 != 0,
            has_display: cap & 0x40 != 0,
            has_light: cap & 0x80 != 0,
        })
    }
}

/// Decodes the payload of a [`DpgCommand::BaseOffset`] response into mm.
pub fn decode_base_offset_mm(payload: &[u8]) -> Option<u16> {
    let [_, lo, hi, ..] = *payload else {
        return None;
    };
    Some((u16::from_le_bytes([lo, hi]) as f32 / 10.0).round() as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_commands() {
        assert_eq!(ControlCommand::MoveUp.encode(), [71, 0]);
        assert_eq!(ControlCommand::Stop.encode(), [255, 0]);
    }

    #[test]
    fn dpg_framing() {
        assert_eq!(encode_dpg_read(DpgCommand::UserId), [0x7f, 134, 0]);
        assert_eq!(
            encode_dpg_write(DpgCommand::UserId, &[1, 2]),
            [0x7f, 134, 0x80, 1, 2]
        );
        assert_eq!(
            decode_dpg_response(&[1, 3, 0xaa, 0xbb]),
            Some(&[0xaa, 0xbb][..])
        );
        assert_eq!(decode_dpg_response(&[0xf3]), None);
        assert_eq!(decode_dpg_response(&[]), None);
    }

    #[test]
    fn height_speed_decode() {
        // Captured from the real desk at rest.
        let hs = HeightSpeed::decode(&[0x7c, 0x08, 0x00, 0x00]).unwrap();
        assert_eq!(hs.height, Height(2172));
        assert_eq!(hs.speed, Speed(0));
        let hs = HeightSpeed::decode(&[0, 0, 0x18, 0xfc]).unwrap();
        assert_eq!(hs.speed.mm_per_s(), -10.0);
        assert!(HeightSpeed::decode(&[1, 2, 3]).is_none());
    }

    #[test]
    fn height_conversion() {
        assert_eq!(Height(2172).to_mm(620), 837.2);
        assert_eq!(Height::from_mm(837.2, 620), Some(Height(2172)));
        assert_eq!(Height::from_mm(600.0, 620), None);
        assert_eq!(Height(500).encode(), [0xf4, 0x01]);
    }

    #[test]
    fn capabilities() {
        let c = Capabilities::decode(&[0b0110_1011, 0]).unwrap();
        assert_eq!(c.memory_size, 3);
        assert!(c.auto_up && !c.auto_down && c.ble_allow && c.has_display && !c.has_light);
        assert!(Capabilities::decode(&[1]).is_none());
    }

    #[test]
    fn base_offset() {
        assert_eq!(decode_base_offset_mm(&[0, 0x38, 0x18]), Some(620));
        assert_eq!(decode_base_offset_mm(&[0, 1]), None);
    }
}
