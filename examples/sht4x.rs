//! Read a SHT4x sensor with an embedded-hal driver.
use buspirate_hal::BusPirate;

use embedded_hal_mock::eh1::delay::StdSleep;
use sht4x_rjw::blocking::SHT4x;

fn main() -> anyhow::Result<()> {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("Provide the serial port path as the first argument.");
        std::process::exit(1)
    };

    let bp = BusPirate::open(&path)?;
    let mut sht40 = SHT4x::new(bp, Default::default());
    let reading = sht40.measure(StdSleep::new())?;
    println!("{:.1} °C", reading.celsius());
    println!("{:.1} %RH", reading.humidity());
    Ok(())
}
