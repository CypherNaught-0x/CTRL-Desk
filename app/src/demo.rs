//! A simulated desk that speaks the same [`Command`]/[`Event`] protocol as the
//! Bluetooth worker. Used by `--demo`, for screenshots and in CI.

use std::time::Duration;

use linak_desk::{Capabilities, DeskInfo, Height, HeightSpeed, Speed};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::{Instant, sleep};

use crate::worker::{Command, Direction, Event, Events, FoundDevice};

const BASE_MM: u16 = 680;
const MAX_RAW: u16 = 6500;
/// Raw units (0.1 mm) per second, about 3.8 cm/s like the real desk.
const SPEED: f32 = 380.0;
const TICK: Duration = Duration::from_millis(50);

/// A random, locally administered MAC address, so no real hardware identifiers
/// appear in the demo or in screenshots.
fn random_id() -> String {
    use std::hash::{BuildHasher, RandomState};
    let bits = RandomState::new().hash_one(std::time::SystemTime::now());
    let mut b = bits.to_le_bytes();
    b[0] = (b[0] | 0x02) & 0xfe;
    b[..6]
        .iter()
        .map(|x| format!("{x:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}

/// The simulated desk plus a few decoy devices, with fresh IDs each run.
struct Devices {
    desk: FoundDevice,
    decoys: Vec<FoundDevice>,
}

impl Devices {
    fn new() -> Self {
        let d = |name: Option<&str>, rssi: i16, likely_desk: bool| FoundDevice {
            name: name.map(Into::into),
            id: random_id(),
            rssi: Some(rssi),
            likely_desk,
        };
        Self {
            desk: d(Some("CTRL DESK"), -58, true),
            decoys: vec![
                d(Some("Living Room Speaker"), -71, false),
                d(Some("Headphones"), -64, false),
                d(Some("Fitness Band"), -83, false),
                d(None, -90, false),
            ],
        }
    }

    fn all(&self) -> Vec<FoundDevice> {
        std::iter::once(&self.desk)
            .chain(&self.decoys)
            .cloned()
            .collect()
    }

    /// Any ID that is not a decoy is treated as the desk, so a desk saved by
    /// an earlier demo session (with a different random ID) still connects.
    fn is_desk(&self, id: &str) -> bool {
        !self.decoys.iter().any(|d| d.id.eq_ignore_ascii_case(id))
    }
}

fn info() -> DeskInfo {
    DeskInfo {
        name: "CTRL DESK".into(),
        manufacturer: "LINAK A/S".into(),
        model: "DPG (demo)".into(),
        capabilities: Some(Capabilities {
            memory_size: 4,
            auto_up: true,
            auto_down: true,
            ble_allow: true,
            has_display: true,
            has_light: true,
        }),
        base_height_mm: Some(BASE_MM),
    }
}

enum Motion {
    Idle,
    To(f32),
    Jog(Direction),
}

pub async fn run(mut cmds: UnboundedReceiver<Command>, ev: Events) {
    let devices = Devices::new();
    // 104.5 cm, like the mockups.
    let mut raw: f32 = 3650.0;
    let mut motion = Motion::Idle;
    let mut connected = false;
    let mut scanning = false;
    let mut last_scan = Instant::now() - Duration::from_secs(1);
    let mut tick = tokio::time::interval(TICK);

    loop {
        tokio::select! {
            cmd = cmds.recv() => {
                let Some(cmd) = cmd else { return };
                match cmd {
                    Command::StartScan => {
                        if connected {
                            connected = false;
                            ev.send(Event::Disconnected);
                        }
                        scanning = true;
                    }
                    Command::StopScan => scanning = false,
                    Command::Connect { id } => {
                        scanning = false;
                        ev.send(Event::Connecting);
                        sleep(Duration::from_millis(900)).await;
                        if devices.is_desk(&id) {
                            connected = true;
                            ev.send(Event::Connected { info: info() });
                            ev.send(Event::Sample(sample(raw, 0.0)));
                        } else {
                            ev.send(Event::ConnectFailed("this device is not a Linak desk".into()));
                        }
                    }
                    Command::Disconnect => {
                        if connected {
                            connected = false;
                            ev.send(Event::Disconnected);
                        }
                        motion = Motion::Idle;
                    }
                    _ if !connected => {}
                    Command::MoveTo(h) => motion = Motion::To(h.0.min(MAX_RAW) as f32),
                    Command::Jog(dir) => motion = Motion::Jog(dir),
                    Command::Stop => {
                        if !matches!(motion, Motion::Idle) {
                            motion = Motion::Idle;
                            ev.send(Event::Sample(sample(raw, 0.0)));
                            ev.send(Event::MotionDone);
                        }
                    }
                }
            }
            _ = tick.tick() => {
                if scanning && last_scan.elapsed() >= Duration::from_millis(800) {
                    last_scan = Instant::now();
                    ev.send(Event::ScanResults(devices.all()));
                }
                if !connected {
                    continue;
                }
                let step = SPEED * TICK.as_secs_f32();
                let velocity = match motion {
                    Motion::Idle => continue,
                    Motion::To(target) => {
                        let delta = target - raw;
                        if delta.abs() <= step {
                            raw = target;
                            motion = Motion::Idle;
                            ev.send(Event::Sample(sample(raw, 0.0)));
                            ev.send(Event::MotionDone);
                            continue;
                        }
                        step.copysign(delta)
                    }
                    Motion::Jog(Direction::Up) => step,
                    Motion::Jog(Direction::Down) => -step,
                };
                let next = (raw + velocity).clamp(0.0, MAX_RAW as f32);
                let speed = if next == raw { 0.0 } else { SPEED.copysign(velocity) };
                raw = next;
                ev.send(Event::Sample(sample(raw, speed)));
            }
        }
    }
}

fn sample(raw: f32, raw_per_s: f32) -> HeightSpeed {
    HeightSpeed {
        height: Height(raw.round() as u16),
        // Speed is reported in 1/100 mm/s; raw is 1/10 mm.
        speed: Speed((raw_per_s * 10.0) as i16),
    }
}
