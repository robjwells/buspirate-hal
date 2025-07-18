mod bpio2_generated;

pub use bpio2_generated::bpio2;

/// HAL wrapper
pub struct BusPirate;

#[allow(dead_code)]
struct Request<'a>(&'a [u8]);

struct Response<'a>(&'a [u8]);

impl BusPirate {
    #[allow(dead_code)]
    fn transfer(&mut self, _req: Request) -> Result<Response, crate::error::Error> {
        todo!()
    }
}

mod eh_i2c {
    use std::mem::{discriminant, Discriminant};

    use embedded_hal::i2c::{ErrorType, I2c, Operation};

    use crate::{
        bpio2::{self, PacketContents, PacketType},
        error::Error,
        Request,
    };

    use super::BusPirate;

    impl ErrorType for BusPirate {
        type Error = Error;
    }

    impl I2c for BusPirate {
        #[allow(unused)]
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

                        let mut i2c_req = bpio2::I2CRWRequestBuilder::new(&mut builder);
                        i2c_req.add_i2cstart(needs_start(previous_operation, operation));
                        i2c_req.add_i2cstart(false);
                        i2c_req.add_i2cstop(false);
                        i2c_req.add_i2caddr(address);
                        i2c_req.add_i2creadbytes(bytes_to_read);
                        i2c_req.finish()
                    }
                    Operation::Write(bytes_to_write) => {
                        let write_data = builder.create_vector(bytes_to_write);
                        let mut i2c_req = bpio2::I2CRWRequestBuilder::new(&mut builder);
                        i2c_req.add_i2cstart(needs_start(previous_operation, operation));
                        i2c_req.add_i2cstop(false);
                        i2c_req.add_i2caddr(address);
                        i2c_req.add_i2creadbytes(0);
                        i2c_req.add_i2cdata(write_data);
                        i2c_req.finish()
                    }
                };

                let mut packet = bpio2::PacketBuilder::new(&mut builder);
                // TODO: Question: is this duplication necessary?
                packet.add_type_(bpio2::PacketType::I2CRWRequest);
                packet.add_contents_type(bpio2::PacketContents::I2CRWRequest);
                packet.add_contents(i2c_req.as_union_value());
                let packet = packet.finish();

                eprintln!("Request packet: {packet:#?}");
                builder.finish(packet, None);

                // Update the previous operation for Repeated-Start purposes.
                previous_operation = Some(discriminant(operation));

                // TODO: Handle the error here properly. Does it need cleaning up the
                // open I2C transaction?
                let response = self.transfer(Request(builder.finished_data())).unwrap();

                // TODO: Handle flatbuffer errors (what are they?)
                let packet = bpio2::root_as_packet(response.0).unwrap();
                eprintln!("Response packet: {packet:#?}");

                // TODO: Confirm the packet is actually an i2c response or return
                // an error (maybe UnexpectedPacketType or something like that).
                // TODO: Question: why is the method named like that? (i2_cresponse)
                let i2c_resp = packet.contents_as_i2_cresponse().unwrap();

                if let Some(error_message) = i2c_resp.error_message() {
                    todo!("Return an error with the message.")
                }

                if !i2c_resp.ack() {
                    todo!("Return a nack error");
                }

                if let Operation::Read(read_buffer) = operation {
                    // if Some(read_data) = i2c_resp.data()
                    let Some(read_data) = i2c_resp.data() else {
                        todo!("Missing data, return an error.");
                    };
                    read_buffer.copy_from_slice(read_data.bytes());
                }
            }

            // Send the final Stop condition.
            builder.reset();
            let mut i2c_req = bpio2::I2CRWRequestBuilder::new(&mut builder);
            i2c_req.add_i2cstart(false);
            i2c_req.add_i2cstop(true);
            i2c_req.add_i2creadbytes(0);
            let i2c_req = i2c_req.finish();

            let mut packet = bpio2::PacketBuilder::new(&mut builder);
            packet.add_type_(PacketType::I2CRWRequest);
            packet.add_contents_type(PacketContents::I2CRWRequest);
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
