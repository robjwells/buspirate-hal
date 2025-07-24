//! Read a 24x16 EEPROM.
#![allow(clippy::unusual_byte_groupings)]

use buspirate_hal::BusPirate;
use embedded_hal::i2c::I2c;

const PAGES: usize = 8;
const PAGE_SIZE: usize = 256;

fn main() -> anyhow::Result<()> {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("Provide the serial port path as the first argument.");
        std::process::exit(1)
    };

    let mut bp = BusPirate::open(&path)?;
    let mut buf = [0u8; PAGE_SIZE * PAGES];
    let address: u8 = 0b1010_000;

    for page in 0..PAGES {
        let start = page * PAGE_SIZE;
        let end = start + PAGE_SIZE;
        bp.write_read(address + (page as u8), &[0], &mut buf[start..end])?;
    }
    println!("{buf:?}");
    Ok(())
}
