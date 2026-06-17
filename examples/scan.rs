use anyhow::Result;
use clap::Parser;
use idotmatrix::scan;
use std::time::Duration;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value_t = 5.0)]
    timeout: f64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    for device in scan(Duration::from_secs_f64(args.timeout)).await? {
        let marker = if device.likely_idotmatrix { " *" } else { "" };
        let name = device.name.as_deref().unwrap_or("(unknown)");
        let rssi = device
            .rssi
            .map(|rssi| format!(" RSSI={rssi}"))
            .unwrap_or_default();
        println!("{}  {}{}{}", device.id, name, rssi, marker);
    }
    Ok(())
}
