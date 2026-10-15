//! Background thread that owns the Bluetooth connection. The UI talks to it with
//! [`Command`]s and receives [`Event`]s.

use std::future::Future;
use std::pin::Pin;
use std::sync::mpsc;
use std::time::Duration;

use futures::StreamExt;
use linak_desk::btleplug::api::{Central, Peripheral as _, ScanFilter};
use linak_desk::btleplug::platform::Adapter;
use linak_desk::protocol::uuids;
use linak_desk::{ControlCommand, Desk, DeskInfo, Height, HeightSpeed, MoveOptions};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::time::sleep;

/// Stop jogging even if the UI never sends a release (e.g. lost focus).
const MAX_JOG: Duration = Duration::from_secs(30);
const JOG_PERIOD: Duration = Duration::from_millis(300);
const RECONNECT_DELAY: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
}

#[derive(Debug)]
pub enum Command {
    StartScan,
    StopScan,
    /// Connect (and keep reconnecting) to the desk with this [`linak_desk::device_id`].
    Connect {
        id: String,
    },
    Disconnect,
    MoveTo(Height),
    /// Start moving; continues until [`Command::Stop`] or any other command.
    Jog(Direction),
    Stop,
}

#[derive(Debug, Clone)]
pub struct FoundDevice {
    pub name: Option<String>,
    pub id: String,
    pub rssi: Option<i16>,
    /// Advertises the Linak control service, or is named like a desk.
    pub likely_desk: bool,
}

#[derive(Debug)]
pub enum Event {
    /// No Bluetooth adapter or it could not be opened. Fatal for the worker.
    AdapterError(String),
    ScanResults(Vec<FoundDevice>),
    Connecting,
    Connected {
        info: DeskInfo,
    },
    ConnectFailed(String),
    Disconnected,
    Sample(HeightSpeed),
    /// A move or jog finished (or was stopped).
    MotionDone,
    Error(String),
}

pub struct Worker {
    commands: UnboundedSender<Command>,
    events: mpsc::Receiver<Event>,
}

impl Worker {
    /// Starts the Bluetooth worker, or the simulated desk if `demo` is set.
    pub fn spawn(ctx: egui::Context, demo: bool) -> Self {
        let (cmd_tx, cmd_rx) = unbounded_channel();
        let (ev_tx, ev_rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("bluetooth".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("tokio runtime");
                let ev = Events { tx: ev_tx, ctx };
                if demo {
                    rt.block_on(crate::demo::run(cmd_rx, ev));
                } else {
                    rt.block_on(run(cmd_rx, ev));
                }
            })
            .expect("spawn bluetooth thread");
        Self {
            commands: cmd_tx,
            events: ev_rx,
        }
    }

    pub fn send(&self, cmd: Command) {
        let _ = self.commands.send(cmd);
    }

    pub fn poll(&self) -> impl Iterator<Item = Event> + '_ {
        self.events.try_iter()
    }
}

#[derive(Clone)]
pub struct Events {
    tx: mpsc::Sender<Event>,
    ctx: egui::Context,
}

impl Events {
    pub fn send(&self, e: Event) {
        let _ = self.tx.send(e);
        self.ctx.request_repaint();
    }
}

async fn run(mut cmds: UnboundedReceiver<Command>, ev: Events) {
    let adapter = match Desk::default_adapter().await {
        Ok(a) => a,
        Err(e) => {
            ev.send(Event::AdapterError(e.to_string()));
            while cmds.recv().await.is_some() {}
            return;
        }
    };

    // Each state handler returns the command that made it exit, if any, so it
    // can be handled here.
    let mut pending = None;
    loop {
        let cmd = match pending.take() {
            Some(c) => c,
            None => match cmds.recv().await {
                Some(c) => c,
                None => return,
            },
        };
        pending = match cmd {
            Command::StartScan => scan(&adapter, &mut cmds, &ev).await,
            Command::Connect { id } => session(&adapter, &id, &mut cmds, &ev).await,
            // Movement and stop requests are meaningless while disconnected.
            _ => None,
        };
    }
}

async fn scan(
    adapter: &Adapter,
    cmds: &mut UnboundedReceiver<Command>,
    ev: &Events,
) -> Option<Command> {
    if let Err(e) = adapter.start_scan(ScanFilter::default()).await {
        ev.send(Event::Error(format!("Could not start scanning: {e}")));
        return None;
    }
    let mut tick = tokio::time::interval(Duration::from_millis(800));
    let next = loop {
        tokio::select! {
            c = cmds.recv() => match c {
                Some(Command::StartScan) => {}
                Some(Command::StopScan) | None => break None,
                Some(other) => break Some(other),
            },
            _ = tick.tick() => ev.send(Event::ScanResults(collect_devices(adapter).await)),
        }
    };
    let _ = adapter.stop_scan().await;
    next
}

async fn collect_devices(adapter: &Adapter) -> Vec<FoundDevice> {
    let mut out = Vec::new();
    for p in adapter.peripherals().await.unwrap_or_default() {
        let Ok(Some(props)) = p.properties().await else {
            continue;
        };
        let named_desk = props
            .local_name
            .as_deref()
            .is_some_and(|n| n.to_ascii_lowercase().contains("desk"));
        out.push(FoundDevice {
            likely_desk: named_desk || props.services.contains(&uuids::CONTROL_SERVICE),
            name: props.local_name,
            id: linak_desk::device_id(&p),
            rssi: props.rssi,
        });
    }
    out
}

/// Waits for a command that should end the current connection attempt. Motion
/// commands are dropped. Returns the command to pass on, if any.
async fn interrupted(cmds: &mut UnboundedReceiver<Command>) -> Option<Command> {
    loop {
        match cmds.recv().await? {
            Command::Disconnect => return None,
            c @ (Command::StartScan | Command::Connect { .. }) => return Some(c),
            _ => {}
        }
    }
}

async fn session(
    adapter: &Adapter,
    id: &str,
    cmds: &mut UnboundedReceiver<Command>,
    ev: &Events,
) -> Option<Command> {
    loop {
        ev.send(Event::Connecting);
        let connect = async {
            let p = Desk::find_by_id(adapter, id, Duration::from_secs(20)).await?;
            Desk::connect(p, None).await
        };
        let result = tokio::select! {
            r = connect => r,
            c = interrupted(cmds) => {
                let _ = adapter.stop_scan().await;
                return c;
            }
        };

        match result {
            Ok(desk) => {
                let outcome = connected(&desk, cmds, ev).await;
                let _ = desk.disconnect().await;
                ev.send(Event::Disconnected);
                match outcome {
                    Ok(next) => return next,
                    Err(e) => ev.send(Event::Error(format!("Lost connection: {e}"))),
                }
            }
            Err(e) => ev.send(Event::ConnectFailed(e.to_string())),
        }

        tokio::select! {
            _ = sleep(RECONNECT_DELAY) => {}
            c = interrupted(cmds) => return c,
        }
    }
}

type Action<'a> = Pin<Box<dyn Future<Output = Result<(), linak_desk::Error>> + 'a>>;

/// Runs while connected. `Ok(next)` means the UI asked us to leave;
/// `Err` means the connection broke.
async fn connected(
    desk: &Desk,
    cmds: &mut UnboundedReceiver<Command>,
    ev: &Events,
) -> Result<Option<Command>, String> {
    ev.send(Event::Connected {
        info: desk.info().clone(),
    });
    ev.send(Event::Sample(
        desk.height_speed().await.map_err(|e| e.to_string())?,
    ));
    let mut samples = desk.watch().await.map_err(|e| e.to_string())?;
    let mut health = tokio::time::interval(Duration::from_secs(2));
    let mut action: Option<Action<'_>> = None;

    loop {
        tokio::select! {
            c = cmds.recv() => {
                // Any new command supersedes the current motion.
                let was_moving = action.take().is_some();
                match c {
                    Some(Command::MoveTo(h)) => {
                        action = Some(Box::pin(async move {
                            desk.move_to(h, &MoveOptions::default()).await.map(|_| ())
                        }));
                    }
                    Some(Command::Jog(dir)) => action = Some(Box::pin(jog(desk, dir))),
                    Some(Command::Stop) => {
                        desk.stop().await.map_err(|e| e.to_string())?;
                        if was_moving {
                            ev.send(Event::MotionDone);
                        }
                    }
                    Some(Command::StopScan) => {}
                    Some(Command::Disconnect) | None => {
                        if was_moving {
                            let _ = desk.stop().await;
                        }
                        return Ok(None);
                    }
                    Some(other) => {
                        if was_moving {
                            let _ = desk.stop().await;
                        }
                        return Ok(Some(other));
                    }
                }
            }
            Some(hs) = samples.next() => ev.send(Event::Sample(hs)),
            r = async { action.as_mut().expect("guarded").await }, if action.is_some() => {
                action = None;
                if let Err(e) = r {
                    ev.send(Event::Error(e.to_string()));
                }
                // The final notification can lag; read the resting value.
                if let Ok(hs) = desk.height_speed().await {
                    ev.send(Event::Sample(hs));
                }
                ev.send(Event::MotionDone);
            }
            _ = health.tick() => {
                if !desk.peripheral().is_connected().await.unwrap_or(false) {
                    return Err("the desk stopped responding".into());
                }
            }
        }
    }
}

/// Keeps the motor running by re-sending the move command before the
/// controller's ~1 s run-on expires.
async fn jog(desk: &Desk, dir: Direction) -> Result<(), linak_desk::Error> {
    let cmd = match dir {
        Direction::Up => ControlCommand::MoveUp,
        Direction::Down => ControlCommand::MoveDown,
    };
    desk.wakeup().await?;
    let until = tokio::time::Instant::now() + MAX_JOG;
    while tokio::time::Instant::now() < until {
        desk.send(cmd).await?;
        sleep(JOG_PERIOD).await;
    }
    desk.stop().await
}
