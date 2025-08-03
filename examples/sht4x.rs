//! Read a SHT4x sensor with an embedded-hal driver.

use buspirate_hal::{ConfigurationRequest, PsuConfig};
use embedded_hal_mock::eh1::delay::StdSleep;
use sht4x_rjw::blocking::SHT4x;

fn main() -> anyhow::Result<()> {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("Provide the serial port path as the first argument.");
        std::process::exit(1)
    };

    let psu_config = PsuConfig::builder()
        .enable(true)
        .millivolts(3300)
        .milliamps(300)
        .build();
    let extra_config = ConfigurationRequest::builder()
        .psu(psu_config)
        .pullup(true)
        .build();

    let bp = buspirate_hal::open(&path)?.enter_i2c_mode(400_000, false, Some(extra_config))?;
    let mut sht40 = SHT4x::new(bp, Default::default());
    let reading = sht40.measure(StdSleep::new())?;
    println!("{:.1} °C", reading.celsius());
    println!("{:.1} %RH", reading.humidity());
    Ok(())
}
