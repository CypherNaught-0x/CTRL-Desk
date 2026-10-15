//! Small CLI around the library.
//!
//! deskctl [--name NAME | --id ID] [--base-height MM] <info|height|watch|up|down|stop|move-to MM>
use std::time::Duration;

use futures::StreamExt;
use linak_desk::{Desk, MoveOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (mut name, mut address, mut base_height) = ("CTRL DESK".to_string(), None, None);
    let mut command = Vec::new();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--name" => name = args.next().ok_or("--name needs a value")?,
            "--id" | "--address" => address = Some(args.next().ok_or("--id needs a value")?),
            "--base-height" => {
                base_height = Some(args.next().ok_or("--base-height needs a value")?.parse()?)
            }
            _ => command.push(a),
        }
    }
    let usage = "usage: deskctl [--name NAME | --id ID] [--base-height MM] <info|height|watch|up|down|stop|move-to MM>";

    let adapter = Desk::default_adapter().await?;
    let scan = Duration::from_secs(15);
    let peripheral = match &address {
        Some(id) => Desk::find_by_id(&adapter, id, scan).await?,
        None => Desk::find_by_name(&adapter, &name, scan).await?,
    };
    let desk = Desk::connect(peripheral, base_height).await?;
    let base = desk.base_height_mm();
    let show = |hs: linak_desk::HeightSpeed| {
        println!(
            "height {:7.1} mm (raw {:5})  speed {:6.2} mm/s",
            hs.height.to_mm(base),
            hs.height.0,
            hs.speed.mm_per_s()
        )
    };

    match command.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["info"] => {
            println!("{:#?}", desk.info());
            println!("id: {}", linak_desk::device_id(desk.peripheral()));
            show(desk.height_speed().await?);
        }
        ["height"] => show(desk.height_speed().await?),
        ["watch"] => {
            show(desk.height_speed().await?);
            let mut updates = desk.watch().await?;
            while let Some(hs) = updates.next().await {
                show(hs);
            }
        }
        ["up"] => desk.move_up().await?,
        ["down"] => desk.move_down().await?,
        ["stop"] => desk.stop().await?,
        ["move-to", mm] => {
            let final_pos = desk
                .move_to_mm(mm.parse()?, &MoveOptions::default())
                .await?;
            show(final_pos);
        }
        _ => eprintln!("{usage}"),
    }
    desk.disconnect().await?;
    Ok(())
}
