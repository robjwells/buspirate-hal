
use buspirate_hal::{
    open, ChipSelectPolarity, ClockPhase, ClockPolarity, Configuration, PsuConfig,
};
use embedded_hal::spi::SpiDevice;

const N_BYTES: usize = 1024 * 1024 * 32 / 8;
const PAGE_SIZE: usize = 512;
const N_PAGES: usize = N_BYTES / PAGE_SIZE;

// const READ_CHIP_INFO: u8 = 0x9F;
const READ: u8 = 0x03;

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("Provide the serial port path as the first argument.");
        std::process::exit(1)
    };

    let config = Configuration::builder()
        .psu(PsuConfig::enable(3_300, 300))
        .pullup(true)
        .build();
    let mut bp = open(&path)
        .unwrap()
        .enter_spi_mode(
            62_500_000,
            8,
            ClockPolarity::ActiveHigh,
            ClockPhase::LeadingEdge,
            ChipSelectPolarity::ActiveLow,
            Some(config),
        )
        .unwrap();

    let mut dump = vec![0u8; N_BYTES];
    for page_idx in 0..N_PAGES {
        let start_idx = page_idx * PAGE_SIZE;
        let end_idx = start_idx + PAGE_SIZE;
        let page_buf = &mut dump[start_idx..end_idx];

        let [_, a2, a1, a0] = (start_idx as u32).to_be_bytes();
        bp.transfer(page_buf, &[READ, a2, a1, a0]).unwrap();
    }
}
