//! Control Linak DPG desk controllers (IKEA Idåsen, and white-label desks such as
//! "CTRL DESK") over Bluetooth LE.
//!
//! ```no_run
//! use std::time::Duration;
//! use linak_desk::{Desk, MoveOptions};
//!
//! # async fn run() -> Result<(), linak_desk::Error> {
//! let adapter = Desk::default_adapter().await?;
//! let peripheral = Desk::find_by_name(&adapter, "CTRL DESK", Duration::from_secs(10)).await?;
//! let desk = Desk::connect(peripheral, None).await?;
//! println!("height: {:.1} mm", desk.height_mm().await?);
//! desk.move_to_mm(1000.0, &MoveOptions::default()).await?;
//! desk.disconnect().await?;
//! # Ok(()) }
//! ```

mod desk;
pub mod protocol;

pub use btleplug;
pub use desk::{Desk, DeskInfo, MoveOptions, device_id};
pub use protocol::{Capabilities, ControlCommand, Height, HeightSpeed, Speed};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("bluetooth error: {0}")]
    Bluetooth(#[from] btleplug::Error),
    #[error("no bluetooth adapter found")]
    NoAdapter,
    #[error("desk not found")]
    NotFound,
    #[error("characteristic {0} not found; is this a Linak DPG desk?")]
    MissingCharacteristic(uuid::Uuid),
    #[error("invalid response from desk")]
    InvalidResponse,
    #[error("timed out")]
    Timeout,
    #[error("height {0} mm is out of range")]
    OutOfRange(f32),
}
