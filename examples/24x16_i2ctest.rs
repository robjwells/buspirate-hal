use buspirate_hal::{Configuration, PsuConfig};
use embedded_hal::i2c::I2c;

fn main() {
    let psu_config = PsuConfig::builder()
        .enable(true)
        .millivolts(3300)
        .milliamps(300)
        .build();
    let extra_config = Configuration::builder()
        .psu(psu_config)
        .pullup(true)
        .build();

    let mut bp = buspirate_hal::open("/dev/cu.usbmodem5buspirate3")
        .unwrap()
        .enter_i2c_mode(400_000, false, Some(extra_config))
        .unwrap();
    #[allow(clippy::unusual_byte_groupings)]
    let address = 0b1010_000;

    println!("Reading 4 bytes:");
    let mut buf = [0u8; 4];
    bp.read(address, &mut buf).unwrap();
    println!("{buf:?}");

    println!("Writing 4 bytes:");
    bp.write(address, &buf).unwrap();
}
