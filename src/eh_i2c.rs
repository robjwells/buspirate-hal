use std::mem::{discriminant, Discriminant};

use embedded_hal::i2c::{ErrorType, I2c, Operation};

use crate::{bpio, error::Error, modes, BusPirate, Request};

macro_rules! create_write_vector {
    ($builder:ident, $address:expr) => {
        $builder.create_vector(&[$address])
    };
    ($builder:ident, $address:expr, data = $data:ident ) => {{
        $builder.start_vector::<u8>($data.len() + 1);
        // Bytes have to be added in reverse order.
        for byte in $data.iter().rev() {
            $builder.push(*byte);
        }
        $builder.push($address);
        $builder.end_vector::<u8>($data.len() + 1)
    }};
}

/// Issue a single I2C transaction, with start and stop conditions
macro_rules! single_data_request {
    (
        $builder:ident,
        addr = $addr:expr
        $(, write = $write:ident )?
        $(, read = $read:ident )?
        $( , )?                     // Optional trailing comma
    ) => {{
        // Create a write vector if either the addres or write data were supplied.
        let wv = create_write_vector!($builder, $addr  $(, data = $write )? );

        let mut req = crate::bpio::DataRequestBuilder::new(&mut $builder);
        req.add_start_main(true);
        req.add_stop_main(true);
        // Always a write vector, even if it's just the address.
        req.add_data_write(wv);
        $( req.add_bytes_read($read.len() as u16); )?

        req.finish()
    }};
}

macro_rules! build_data_request_packet {
    ($builder:ident, $dr:expr) => {{
        // Result of data_request!(...)
        let data_req = $dr;

        let mut packet = crate::bpio::RequestPacketBuilder::new(&mut $builder);
        packet.add_contents_type(crate::bpio::RequestPacketContents::DataRequest);
        packet.add_contents(data_req.as_union_value());
        let packet = packet.finish();

        $builder.finish_minimal(packet);
    }};
}

fn i2c_read_address(address: u8) -> u8 {
    (address << 1) + 1
}

fn i2c_write_address(address: u8) -> u8 {
    address << 1
}

impl ErrorType for BusPirate<modes::I2c> {
    type Error = Error;
}

impl I2c for BusPirate<modes::I2c> {
    fn transaction(
        &mut self,
        address: u8,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        // TODO: Audit error points for I2C transaction cleanup.

        // TODO: Choose a sensible capacity for the flatbuffer builder.
        let mut builder = flatbuffers::FlatBufferBuilder::with_capacity(1024);

        type PreviousOp<'a> = Option<Discriminant<Operation<'a>>>;
        // Track the type (Read/Write) of the previous I2C operation to allow
        // for operation coalescing.
        let mut previous_operation: PreviousOp = None;
        // A Start condition is needed when:
        // - It is the first operation (previous_operation is None).
        // - The previous operation is of a different type to the current operation.
        fn needs_start<'a>(po: PreviousOp<'a>, op: &Operation<'a>) -> bool {
            !po.is_some_and(|po| discriminant(op) == po)
        }

        for operation in operations {
            // Re-use the builder buffer.
            builder.reset();

            // Build I2C request.
            let i2c_req = match operation {
                Operation::Read(read_buffer) => {
                    let Ok(bytes_to_read) = read_buffer.len().try_into() else {
                        // TODO: Return a meaningful error if the read is too long.
                        // TODO: Question: should there be a max read length?
                        return Err(Error::Other);
                    };

                    // TODO: address shouldn't be reissued with adjacent operations.
                    let write_data = create_write_vector!(builder, i2c_read_address(address));

                    let mut i2c_req = bpio::DataRequestBuilder::new(&mut builder);
                    i2c_req.add_start_main(needs_start(previous_operation, operation));
                    i2c_req.add_stop_main(false);
                    // I2C address
                    i2c_req.add_data_write(write_data);
                    i2c_req.add_bytes_read(bytes_to_read);
                    i2c_req.finish()
                }
                Operation::Write(bytes_to_write) => {
                    // TODO: address shouldn't be reissued with adjacent operations.
                    let write_data = create_write_vector!(
                        builder,
                        i2c_write_address(address),
                        data = bytes_to_write
                    );

                    let mut i2c_req = bpio::DataRequestBuilder::new(&mut builder);
                    i2c_req.add_start_main(needs_start(previous_operation, operation));
                    i2c_req.add_stop_main(false);
                    i2c_req.add_bytes_read(0);
                    i2c_req.add_data_write(write_data);
                    i2c_req.finish()
                }
            };

            build_data_request_packet!(builder, i2c_req);

            eprintln!(
                "{:#?}",
                flatbuffers::root::<bpio::RequestPacket>(builder.finished_data()).unwrap()
            );

            // Update the previous operation for Repeated-Start purposes.
            previous_operation = Some(discriminant(operation));

            let Ok(response) = self.transfer(Request::encode(builder.finished_data())) else {
                // Does this also need I2C transaction cleanup (issue Stop)?
                todo!("Handle transfer error.");
            };

            let Ok(packet) = bpio::root_as_response_packet(&response.cobs_decoded) else {
                todo!("Handle errors from flatbuffer");
            };
            eprintln!("{packet:#?}");

            let Some(i2c_resp) = packet.contents_as_data_response() else {
                todo!("Confirm response is actually a data response or return an error.")
            };

            if let Some(error_message) = i2c_resp.error() {
                eprintln!("{error_message:#?}");
                todo!("Return an error with the message.")
            }

            if let Operation::Read(read_buffer) = operation {
                // if Some(read_data) = i2c_resp.data()
                let Some(read_data) = i2c_resp.data_read() else {
                    todo!("Missing data, return an error.");
                };
                read_buffer.copy_from_slice(read_data.bytes());
            }
        }

        // Send the final Stop condition.
        builder.reset();
        let _response = self.i2c_stop(&mut builder)?;
        // TODO: Check response for error.
        Ok(())
    }

    fn write_read(
        &mut self,
        address: u8,
        write: &[u8],
        read: &mut [u8],
    ) -> Result<(), Self::Error> {
        let mut builder = flatbuffers::FlatBufferBuilder::with_capacity(512);

        build_data_request_packet!(
            builder,
            single_data_request!(
                builder,
                addr = i2c_write_address(address),
                write = write,
                read = read,
            )
        );

        let response = self.transfer(Request::encode(builder.finished_data()))?;
        let response = bpio::root_as_response_packet(&response.cobs_decoded)?;
        // TODO: Handle error message
        if let Some(data_response) = response.contents_as_data_response() {
            if let Some(data) = data_response.data_read() {
                read.copy_from_slice(data.bytes());
            }
        } else {
            eprintln!("Couldn't get data response from contents.");
            return Err(Error::Other);
        }

        Ok(())
    }

    fn read(&mut self, address: u8, read: &mut [u8]) -> Result<(), Self::Error> {
        let mut builder = flatbuffers::FlatBufferBuilder::with_capacity(512);

        build_data_request_packet!(
            builder,
            single_data_request!(builder, addr = i2c_read_address(address), read = read)
        );

        println!(
            "{:#?}",
            flatbuffers::root::<bpio::RequestPacket>(builder.finished_data()).unwrap()
        );

        let response = self.transfer(Request::encode(builder.finished_data()))?;
        let response = bpio::root_as_response_packet(&response.cobs_decoded)?;
        // TODO: Handle error message
        // Also figure out how these properties interact. Is data always Some
        // if data_response is Some?
        if let Some(data_response) = response.contents_as_data_response() {
            if let Some(data) = data_response.data_read() {
                read.copy_from_slice(data.bytes());
                return Ok(());
            }
        }
        // TODO: Handle error properly.
        eprintln!("Couldn't get data response from contents.");
        Err(Error::Other)
    }

    fn write(&mut self, address: u8, write: &[u8]) -> Result<(), Self::Error> {
        let mut builder = flatbuffers::FlatBufferBuilder::with_capacity(512);

        build_data_request_packet!(
            builder,
            single_data_request!(builder, addr = i2c_write_address(address), write = write)
        );

        println!(
            "{:#?}",
            flatbuffers::root::<bpio::RequestPacket>(builder.finished_data()).unwrap()
        );

        let response_bytes = self.transfer(Request::encode(builder.finished_data()))?;
        let _response = bpio::root_as_response_packet(&response_bytes.cobs_decoded)?;
        // TODO: Handle error message
        Ok(())
    }
}
