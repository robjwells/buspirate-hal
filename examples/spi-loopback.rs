use buspirate_hal::{Configuration, PsuConfig};
use embedded_hal::spi::SpiDevice;

fn main() -> anyhow::Result<()> {
    env_logger::builder().format_timestamp_millis().init();

    let Some(path) = std::env::args().nth(1) else {
        eprintln!("Provide the serial port path as the first argument.");
        std::process::exit(1)
    };

    let psu_config = PsuConfig::builder()
        .enable(true)
        .millivolts(3300)
        .milliamps(300)
        .build();
    let extra_config = Configuration::builder()
        .psu(psu_config)
        .pullup(true)
        .build();
    let mut bp = buspirate_hal::open(&path)?.enter_spi_mode(
        5_000,
        8,
        buspirate_hal::ClockPolarity::ActiveLow,
        buspirate_hal::ClockPhase::TrailingEdge,
        buspirate_hal::ChipSelectPolarity::ActiveLow,
        Some(extra_config),
    )?;

    let write = [0xAA];
    let mut read = [0u8; 1];
    bp.transfer(&mut read, &write)?;
    assert_eq!(write, read);

    Ok(())
}
