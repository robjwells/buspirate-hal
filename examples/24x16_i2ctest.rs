use embedded_hal::i2c::I2c;

fn main() {
    let mut bp = buspirate_hal::open("/dev/cu.usbmodem5buspirate3")
        .unwrap()
        .enter_i2c_mode(400_000, false)
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
