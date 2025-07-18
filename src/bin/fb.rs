use std::io::Write;

use buspirate_hal::bpio2;

/// I2C target base address.
const ADDRESS: u8 = 0x26;

/// Packet bytes output filename.
const FILENAME: &str = "i2crw.bin";

fn main() {
    let mut fbb = flatbuffers::FlatBufferBuilder::with_capacity(256);

    // Create data vector.
    // This needs to happen first because of borrowing rules around &mut fbb.
    let i2c_data = fbb.create_vector(&[60_u8, 10]);

    // Create the I2C request structure.
    let mut i2crw = bpio2::I2CRWRequestBuilder::new(&mut fbb);
    i2crw.add_i2caddr(ADDRESS);
    i2crw.add_i2cdata(i2c_data);
    i2crw.add_i2creadbytes(10);
    let i2crw = i2crw.finish();

    // Create the Packet structure.
    let mut pb = bpio2::PacketBuilder::new(&mut fbb);
    pb.add_type_(bpio2::PacketType::I2CRWRequest);
    pb.add_contents_type(bpio2::PacketContents::I2CRWRequest);
    pb.add_contents(i2crw.as_union_value());
    let pb = pb.finish();
    println!("{pb:#?}");

    fbb.finish(pb, None);

    let bytes_for_bp = fbb.finished_data();
    std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(FILENAME)
        .expect("Failed to open packet output file for writing.")
        .write_all(bytes_for_bp)
        .expect("Failed to write i2crw bytes.");

    let byte_vec = std::fs::read(FILENAME).expect("Failed to read packet output file.");

    let packet = bpio2::root_as_packet(&byte_vec).expect("Failed to get bytes as packet.");
    println!("{packet:#?}");
}
