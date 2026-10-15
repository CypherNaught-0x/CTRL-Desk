use std::time::Duration;

use btleplug::api::{
    BDAddr, Central, Characteristic, Manager as _, Peripheral as _, ScanFilter, WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use futures::StreamExt;
use futures::stream::BoxStream;
use tokio::time::{Instant, sleep, timeout};
use uuid::Uuid;

use crate::Error;
use crate::protocol::{self, Capabilities, ControlCommand, DpgCommand, Height, HeightSpeed, uuids};

/// Interval between re-sending the target height while moving. The controller
/// stops on its own if it does not hear from us for roughly a second.
const MOVE_COMMAND_PERIOD: Duration = Duration::from_millis(400);
const DPG_TIMEOUT: Duration = Duration::from_secs(5);

/// A stable, printable identifier for a peripheral: its MAC address where the
/// platform exposes one, otherwise (macOS) the CoreBluetooth peripheral UUID.
pub fn device_id(p: &Peripheral) -> String {
    let addr = p.address();
    if addr == BDAddr::default() {
        p.id().to_string()
    } else {
        addr.to_string()
    }
}

/// Options for [`Desk::move_to`].
#[derive(Debug, Clone)]
pub struct MoveOptions {
    /// Give up (and send stop) if the target is not reached within this time.
    pub timeout: Duration,
}

impl Default for MoveOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(60),
        }
    }
}

/// Static information read from the desk on connect.
#[derive(Debug, Clone)]
pub struct DeskInfo {
    pub name: String,
    pub manufacturer: String,
    pub model: String,
    pub capabilities: Option<Capabilities>,
    /// Height of the tabletop at the lowest position, in mm, as reported by the
    /// controller. `None` if the controller does not report it.
    pub base_height_mm: Option<u16>,
}

/// A connected Linak desk.
pub struct Desk {
    peripheral: Peripheral,
    info: DeskInfo,
    base_height_mm: u16,
}

impl Desk {
    /// Returns the first Bluetooth adapter on the system.
    pub async fn default_adapter() -> Result<Adapter, Error> {
        Manager::new()
            .await?
            .adapters()
            .await?
            .into_iter()
            .next()
            .ok_or(Error::NoAdapter)
    }

    /// Scans until a device whose advertised name equals `name`
    /// (case-insensitive) is seen, or `scan_timeout` elapses.
    pub async fn find_by_name(
        adapter: &Adapter,
        name: &str,
        scan_timeout: Duration,
    ) -> Result<Peripheral, Error> {
        Self::find(adapter, scan_timeout, |local_name, _| {
            local_name.is_some_and(|n| n.eq_ignore_ascii_case(name))
        })
        .await
    }

    /// Scans until a device with the given MAC address (e.g.
    /// `"AA:BB:CC:DD:EE:FF"`, case-insensitive) is seen, or `scan_timeout`
    /// elapses. MAC addresses are not available on macOS; prefer
    /// [`Desk::find_by_id`] for portable code.
    pub async fn find_by_address(
        adapter: &Adapter,
        address: &str,
        scan_timeout: Duration,
    ) -> Result<Peripheral, Error> {
        Self::find(adapter, scan_timeout, |_, p| {
            p.address().to_string().eq_ignore_ascii_case(address)
        })
        .await
    }

    /// Scans until a device whose [`device_id`] equals `id` (case-insensitive)
    /// is seen, or `scan_timeout` elapses.
    pub async fn find_by_id(
        adapter: &Adapter,
        id: &str,
        scan_timeout: Duration,
    ) -> Result<Peripheral, Error> {
        Self::find(adapter, scan_timeout, |_, p| {
            device_id(p).eq_ignore_ascii_case(id)
        })
        .await
    }

    async fn find(
        adapter: &Adapter,
        scan_timeout: Duration,
        matches: impl Fn(Option<&str>, &Peripheral) -> bool,
    ) -> Result<Peripheral, Error> {
        adapter.start_scan(ScanFilter::default()).await?;
        let deadline = Instant::now() + scan_timeout;
        let result = loop {
            let mut found = None;
            for p in adapter.peripherals().await? {
                let name = p.properties().await?.and_then(|props| props.local_name);
                if matches(name.as_deref(), &p) {
                    found = Some(p);
                    break;
                }
            }
            if let Some(p) = found {
                break Ok(p);
            }
            if Instant::now() >= deadline {
                break Err(Error::NotFound);
            }
            sleep(Duration::from_millis(250)).await;
        };
        adapter.stop_scan().await?;
        result
    }

    /// Connects to `peripheral` and initialises the controller.
    ///
    /// Like the reference Python implementation, this sets the first byte of the
    /// DPG user ID to `1` if it is not already, which DPG1C controllers require
    /// before they accept movement commands from a new client.
    ///
    /// `base_height_mm` overrides the controller-reported height of the tabletop
    /// at its lowest position.
    pub async fn connect(
        peripheral: Peripheral,
        base_height_mm: Option<u16>,
    ) -> Result<Self, Error> {
        if !peripheral.is_connected().await? {
            peripheral.connect().await?;
        }
        peripheral.discover_services().await?;

        let mut desk = Self {
            peripheral,
            info: DeskInfo {
                name: String::new(),
                manufacturer: String::new(),
                model: String::new(),
                capabilities: None,
                base_height_mm: None,
            },
            base_height_mm: 0,
        };
        // Fail early with a clear error if this is not a Linak DPG desk.
        for uuid in [
            uuids::CONTROL_COMMAND,
            uuids::REFERENCE_OUTPUT_ONE,
            uuids::REFERENCE_INPUT_ONE,
            uuids::DPG,
        ] {
            desk.characteristic(uuid)?;
        }

        desk.info.name = desk.read_string(uuids::DEVICE_NAME).await;
        desk.info.manufacturer = desk.read_string(uuids::MANUFACTURER).await;
        desk.info.model = desk.read_string(uuids::MODEL_NUMBER).await;
        desk.info.capabilities = desk
            .dpg_command(DpgCommand::GetCapabilities, None)
            .await?
            .and_then(|p| Capabilities::decode(&p));

        if let Some(mut user_id) = desk.dpg_command(DpgCommand::UserId, None).await?
            && user_id.first().is_some_and(|&b| b != 1)
        {
            user_id[0] = 1;
            desk.dpg_command(DpgCommand::UserId, Some(&user_id)).await?;
        }

        desk.info.base_height_mm = desk
            .dpg_command(DpgCommand::BaseOffset, None)
            .await?
            .and_then(|p| protocol::decode_base_offset_mm(&p));
        desk.base_height_mm = base_height_mm.or(desk.info.base_height_mm).unwrap_or(0);
        Ok(desk)
    }

    pub fn info(&self) -> &DeskInfo {
        &self.info
    }

    /// The base height in effect (override or controller-reported, else 0).
    pub fn base_height_mm(&self) -> u16 {
        self.base_height_mm
    }

    pub fn peripheral(&self) -> &Peripheral {
        &self.peripheral
    }

    pub async fn disconnect(&self) -> Result<(), Error> {
        Ok(self.peripheral.disconnect().await?)
    }

    /// Reads the current height and speed.
    pub async fn height_speed(&self) -> Result<HeightSpeed, Error> {
        let data = self
            .peripheral
            .read(&self.characteristic(uuids::REFERENCE_OUTPUT_ONE)?)
            .await?;
        HeightSpeed::decode(&data).ok_or(Error::InvalidResponse)
    }

    /// Reads the current absolute height in mm.
    pub async fn height_mm(&self) -> Result<f32, Error> {
        Ok(self.height_speed().await?.height.to_mm(self.base_height_mm))
    }

    /// Streams height/speed updates as the desk moves.
    pub async fn watch(&self) -> Result<BoxStream<'static, HeightSpeed>, Error> {
        let ch = self.characteristic(uuids::REFERENCE_OUTPUT_ONE)?;
        let notifications = self.peripheral.notifications().await?;
        self.peripheral.subscribe(&ch).await?;
        Ok(notifications
            .filter_map(|n| async move {
                (n.uuid == uuids::REFERENCE_OUTPUT_ONE)
                    .then(|| HeightSpeed::decode(&n.value))
                    .flatten()
            })
            .boxed())
    }

    pub async fn send(&self, command: ControlCommand) -> Result<(), Error> {
        self.write(uuids::CONTROL_COMMAND, &command.encode()).await
    }

    pub async fn wakeup(&self) -> Result<(), Error> {
        self.send(ControlCommand::Wakeup).await
    }

    pub async fn stop(&self) -> Result<(), Error> {
        self.send(ControlCommand::Stop).await
    }

    /// Nudges the desk up for roughly one second.
    pub async fn move_up(&self) -> Result<(), Error> {
        self.send(ControlCommand::MoveUp).await
    }

    /// Nudges the desk down for roughly one second.
    pub async fn move_down(&self) -> Result<(), Error> {
        self.send(ControlCommand::MoveDown).await
    }

    /// Moves to an absolute height in mm.
    pub async fn move_to_mm(&self, mm: f32, opts: &MoveOptions) -> Result<HeightSpeed, Error> {
        let target = Height::from_mm(mm, self.base_height_mm).ok_or(Error::OutOfRange(mm))?;
        self.move_to(target, opts).await
    }

    /// Moves to a raw height and waits until the desk stops. Returns the final
    /// height/speed. The controller itself enforces its min/max limits and
    /// collision detection.
    pub async fn move_to(&self, target: Height, opts: &MoveOptions) -> Result<HeightSpeed, Error> {
        let current = self.height_speed().await?;
        if current.height == target {
            return Ok(current);
        }

        self.wakeup().await?;
        self.stop().await?;

        let deadline = Instant::now() + opts.timeout;
        let input = target.encode();
        loop {
            self.write(uuids::REFERENCE_INPUT_ONE, &input).await?;
            sleep(MOVE_COMMAND_PERIOD).await;
            let hs = self.height_speed().await?;
            if hs.speed.0 == 0 {
                return Ok(hs);
            }
            if Instant::now() >= deadline {
                self.stop().await?;
                return Err(Error::Timeout);
            }
        }
    }

    /// Sends a DPG request and waits for the response notification. Returns
    /// `None` if the controller rejected the request.
    async fn dpg_command(
        &self,
        cmd: DpgCommand,
        data: Option<&[u8]>,
    ) -> Result<Option<Vec<u8>>, Error> {
        let ch = self.characteristic(uuids::DPG)?;
        let mut notifications = self.peripheral.notifications().await?;
        self.peripheral.subscribe(&ch).await?;

        let request = match data {
            Some(d) => protocol::encode_dpg_write(cmd, d),
            None => protocol::encode_dpg_read(cmd),
        };
        self.peripheral
            .write(&ch, &request, WriteType::WithResponse)
            .await?;

        let response = timeout(DPG_TIMEOUT, async {
            while let Some(n) = notifications.next().await {
                if n.uuid == uuids::DPG {
                    return Some(n.value);
                }
            }
            None
        })
        .await;
        self.peripheral.unsubscribe(&ch).await?;

        let value = response
            .map_err(|_| Error::Timeout)?
            .ok_or(Error::InvalidResponse)?;
        Ok(protocol::decode_dpg_response(&value).map(<[u8]>::to_vec))
    }

    async fn write(&self, uuid: Uuid, data: &[u8]) -> Result<(), Error> {
        let ch = self.characteristic(uuid)?;
        Ok(self
            .peripheral
            .write(&ch, data, WriteType::WithResponse)
            .await?)
    }

    async fn read_string(&self, uuid: Uuid) -> String {
        let Ok(ch) = self.characteristic(uuid) else {
            return String::new();
        };
        match self.peripheral.read(&ch).await {
            Ok(v) => String::from_utf8_lossy(&v)
                .trim_end_matches('\0')
                .to_string(),
            Err(_) => String::new(),
        }
    }

    fn characteristic(&self, uuid: Uuid) -> Result<Characteristic, Error> {
        self.peripheral
            .characteristics()
            .into_iter()
            .find(|c| c.uuid == uuid)
            .ok_or(Error::MissingCharacteristic(uuid))
    }
}
