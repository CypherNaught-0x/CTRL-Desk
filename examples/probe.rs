//! Read-only probe: finds a desk by name, dumps its GATT table and readable values.
use btleplug::api::{Central, CharPropFlags, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::Manager;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let name = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "CTRL Desk".into());
    let adapter = Manager::new()
        .await?
        .adapters()
        .await?
        .into_iter()
        .next()
        .ok_or("no adapter")?;
    adapter.start_scan(ScanFilter::default()).await?;
    tokio::time::sleep(Duration::from_secs(6)).await;
    let mut found = None;
    for p in adapter.peripherals().await? {
        let props = p.properties().await?.unwrap_or_default();
        println!(
            "seen: {:?} {:?} rssi={:?}",
            props.local_name,
            p.address(),
            props.rssi
        );
        if props
            .local_name
            .as_deref()
            .is_some_and(|n| n.eq_ignore_ascii_case(&name))
        {
            found = Some(p);
        }
    }
    adapter.stop_scan().await?;
    let p = found.ok_or("desk not found")?;
    p.connect().await?;
    p.discover_services().await?;
    for s in p.services() {
        println!("service {}", s.uuid);
        for c in &s.characteristics {
            print!("  char {} {:?}", c.uuid, c.properties);
            if c.properties.contains(CharPropFlags::READ) {
                match p.read(c).await {
                    Ok(v) => print!(" = {:02x?} {:?}", v, String::from_utf8_lossy(&v)),
                    Err(e) => print!(" read err {e}"),
                }
            }
            println!();
        }
    }
    p.disconnect().await?;
    Ok(())
}
