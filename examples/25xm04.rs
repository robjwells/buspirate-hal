use buspirate_hal::{
    open, ChipSelectPolarity, ClockPhase, ClockPolarity, Configuration, PsuConfig,
};

fn main() {
    let config = Configuration::builder()
        .psu(PsuConfig::enable(3_300, 300))
        .pullup(true)
        .build();
    let bp = open("/dev/cu.usbmodem5buspirate3").unwrap().enter_spi_mode(
        5_000_000,
        8,
        ClockPolarity::ActiveHigh,
        ClockPhase::LeadingEdge,
        ChipSelectPolarity::ActiveLow,
        Some(config),
    );
    if let Err(e) = bp {
        println!("{e:?}");
    }
}
