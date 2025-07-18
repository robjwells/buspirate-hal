mod bpio2_generated;

pub use bpio2_generated::bpio;
use flatbuffers::FlatBufferBuilder;

/// HAL wrapper
pub struct BusPirate {
    i2c_speed: u32,
}

#[allow(dead_code)]
struct Request<'a>(&'a [u8]);

struct Response<'a>(&'a [u8]);

impl BusPirate {
    fn transfer(&mut self, _req: Request) -> Result<Response, crate::error::Error> {
        todo!()
    }

    /// Put the Bus Pirate into I2C mode.
    fn enter_i2c_mode(
        &mut self,
        builder: &mut FlatBufferBuilder,
    ) -> Result<(), crate::error::Error> {
        let i2c_string = builder.create_string("I2C");
        let i2c_config =
            bpio::I2CConfig::create(builder, &bpio::I2CConfigArgs { speed: self.i2c_speed });

        let mut status_request = bpio::StatusRequestBuilder::new(builder);
        status_request.add_name(i2c_string);
        status_request.add_configuration_type(bpio::ModeConfiguration::I2CConfig);
        status_request.add_configuration(i2c_config.as_union_value());
        let status_request = status_request.finish();

        let mut packet = bpio::RequestPacketBuilder::new(builder);
        packet.add_contents_type(bpio::RequestPacketContents::StatusRequest);
        packet.add_contents(status_request.as_union_value());
        let packet = packet.finish();

        eprintln!("Status request packet: {packet:#?}");
        builder.finish(packet, None);

        self.transfer(Request(builder.finished_data()))?;
        Ok(())
    }
}

mod eh_i2c {
    use std::mem::{discriminant, Discriminant};

    use embedded_hal::i2c::{ErrorType, I2c, Operation};

    use crate::{
        bpio::{self, RequestPacketContents},
        error::Error,
        Request,
    };

    use super::BusPirate;

    impl ErrorType for BusPirate {
        type Error = Error;
    }

    impl I2c for BusPirate {
        fn transaction(
            &mut self,
            address: u8,
            operations: &mut [Operation<'_>],
        ) -> Result<(), Self::Error> {
            // TODO: Audit error points for I2C transaction cleanup.

            // TODO: Choose a sensible capacity for the flatbuffer builder.
            let mut builder = flatbuffers::FlatBufferBuilder::with_capacity(1024);

            // Put the Bus Pirate into I2C mode.
            self.enter_i2c_mode(&mut builder)?;

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

                        let mut i2c_req = bpio::DataRequestBuilder::new(&mut builder);
                        i2c_req.add_dstart(needs_start(previous_operation, operation));
                        i2c_req.add_dstop(false);
                        i2c_req.add_daddr(address);
                        i2c_req.add_dreadbytes(bytes_to_read);
                        i2c_req.finish()
                    }
                    Operation::Write(bytes_to_write) => {
                        let write_data = builder.create_vector(bytes_to_write);
                        let mut i2c_req = bpio::DataRequestBuilder::new(&mut builder);
                        i2c_req.add_dstart(needs_start(previous_operation, operation));
                        i2c_req.add_dstop(false);
                        i2c_req.add_daddr(address);
                        i2c_req.add_dreadbytes(0);
                        i2c_req.add_ddata(write_data);
                        i2c_req.finish()
                    }
                };

                let mut packet = bpio::RequestPacketBuilder::new(&mut builder);
                packet.add_contents_type(bpio::RequestPacketContents::DataRequest);
                packet.add_contents(i2c_req.as_union_value());
                let packet = packet.finish();

                eprintln!("Request packet: {packet:#?}");
                builder.finish(packet, None);

                // Update the previous operation for Repeated-Start purposes.
                previous_operation = Some(discriminant(operation));

                let Ok(response) = self.transfer(Request(builder.finished_data())) else {
                    // Does this also need I2C transaction cleanup (issue Stop)?
                    todo!("Handle transfer error.");
                };

                let Ok(packet) = bpio::root_as_response_packet(response.0) else {
                    todo!("Handle errors from flatbuffer");
                };
                eprintln!("Response packet: {packet:#?}");

                let Some(i2c_resp) = packet.contents_as_data_response() else {
                    todo!("Confirm response is actually a data response or return an error.")
                };

                if let Some(_error_message) = i2c_resp.derror_message() {
                    todo!("Return an error with the message.")
                }

                // TODO: Question: is ack information accessible elsewhere now (2025-07-18)
                // that the response structure has changed to remove it?
                // if !i2c_resp.ack() {
                //     todo!("Return a nack error");
                // }

                if let Operation::Read(read_buffer) = operation {
                    // if Some(read_data) = i2c_resp.data()
                    let Some(read_data) = i2c_resp.ddata() else {
                        todo!("Missing data, return an error.");
                    };
                    read_buffer.copy_from_slice(read_data.bytes());
                }
            }

            // Send the final Stop condition.
            builder.reset();
            let mut i2c_req = bpio::DataRequestBuilder::new(&mut builder);
            i2c_req.add_dstart(false);
            i2c_req.add_dstop(true);
            i2c_req.add_dreadbytes(0);
            let i2c_req = i2c_req.finish();

            let mut packet = bpio::RequestPacketBuilder::new(&mut builder);
            packet.add_contents_type(RequestPacketContents::DataRequest);
            packet.add_contents(i2c_req.as_union_value());
            let packet = packet.finish();

            builder.finish(packet, None);
            eprintln!("Final stop request: {packet:#?}");

            self.transfer(Request(builder.finished_data()))?;
            Ok(())
        }
    }
}

mod error {
    #[derive(Debug)]
    pub enum Error {
        Other,
    }

    impl embedded_hal::i2c::Error for Error {
        fn kind(&self) -> embedded_hal::i2c::ErrorKind {
            // TODO: This is a dummy implementation.
            match self {
                Error::Other => embedded_hal::i2c::ErrorKind::Other,
            }
        }
    }
}
