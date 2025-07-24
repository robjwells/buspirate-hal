// TODO: Patch out the warning-generating code.
#[allow(clippy::all)]
#[allow(unused_imports)]
mod bpio_generated;

use std::time::Duration;

pub use crate::error::Error;
use bpio_generated::bpio;

use flatbuffers::FlatBufferBuilder;
use serialport::SerialPort;

/// HAL wrapper
pub struct BusPirate {
    i2c_speed: u32,
    i2c_clock_stretching: bool,
    i2c_power: bool,
    millivolts: u32,
    serial_port: Box<dyn SerialPort>,
}

impl BusPirate {
    pub fn open(address: &str) -> Result<Self, serialport::Error> {
        let serial_port = serialport::new(address, 115_200)
            // TODO: choose a sensible timeout value.
            .timeout(Duration::from_secs(1))
            .open()?;
        Ok(Self {
            i2c_speed: 100_000,
            i2c_clock_stretching: true,
            i2c_power: true,
            millivolts: 3_300,
            serial_port,
        })
    }
}

#[allow(dead_code)]
struct Request<'a>(&'a [u8]);

impl<'a> Request<'a> {
    fn length_prefix(&self) -> [u8; 2] {
        (self.0.len() as u16).to_le_bytes()
    }
}

struct Response(Vec<u8>);

impl BusPirate {
    fn transfer(&mut self, req: Request) -> Result<Response, crate::error::Error> {
        self.serial_port.write_all(&req.length_prefix())?;
        self.serial_port.write_all(req.0)?;

        let mut length_bytes = [0u8; 2];
        self.serial_port.read_exact(&mut length_bytes)?;

        let mut data = vec![0u8; u16::from_le_bytes(length_bytes) as usize];
        self.serial_port.read_exact(&mut data)?;
        Ok(Response(data.to_owned()))
    }

    /// Put the Bus Pirate into I2C mode.
    pub fn enter_i2c_mode(
        &mut self,
        builder: &mut FlatBufferBuilder,
    ) -> Result<(), crate::error::Error> {
        let i2c_string = builder.create_string("I2C");
        let mut i2c_config = bpio::ModeConfigurationBuilder::new(builder);
        i2c_config.add_speed(self.i2c_speed);
        i2c_config.add_clock_stretch(self.i2c_clock_stretching);
        let i2c_config = i2c_config.finish();

        let mut config_request = bpio::ConfigurationRequestBuilder::new(builder);
        config_request.add_mode(i2c_string);
        config_request.add_mode_configuration(i2c_config);
        config_request.add_pullup_enable(true);
        if self.i2c_power {
            config_request.add_psu_enable(true);
            config_request.add_psu_set_mv(self.millivolts);
        }
        let config_request = config_request.finish();

        let mut packet = bpio::RequestPacketBuilder::new(builder);
        packet.add_contents_type(bpio::RequestPacketContents::ConfigurationRequest);
        packet.add_contents(config_request.as_union_value());
        let packet = packet.finish();

        builder.finish_minimal(packet);

        // TODO: Add proper logging.
        // eprintln!(
        //     "{:#?}",
        //     flatbuffers::root::<bpio::RequestPacket>(builder.finished_data()).unwrap()
        // );

        self.transfer(Request(builder.finished_data()))?;
        builder.reset();

        Ok(())
    }

    fn i2c_stop(&mut self, builder: &mut FlatBufferBuilder) -> Result<Response, self::Error> {
        let mut i2c_req = bpio::DataRequestBuilder::new(builder);
        i2c_req.add_start_main(false);
        i2c_req.add_stop_main(true);
        i2c_req.add_bytes_read(0);
        let i2c_req = i2c_req.finish();

        let mut packet = bpio::RequestPacketBuilder::new(builder);
        packet.add_contents_type(bpio::RequestPacketContents::DataRequest);
        packet.add_contents(i2c_req.as_union_value());
        let packet = packet.finish();

        builder.finish(packet, None);

        let response = self.transfer(Request(builder.finished_data()))?;
        builder.reset();
        Ok(response)
    }
}

mod eh_i2c {
    use std::mem::{discriminant, Discriminant};

    use embedded_hal::i2c::{ErrorType, I2c, Operation};

    use crate::{
        bpio::{self, RequestPacketBuilder},
        error::Error,
        BusPirate, Request,
    };

    impl ErrorType for BusPirate {
        type Error = Error;
    }

    fn i2c_read_address(address: u8) -> u8 {
        (address << 1) + 1
    }

    fn i2c_write_address(address: u8) -> u8 {
        address << 1
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

                        // Read address in the write buffer
                        let write_data = builder.create_vector(&[i2c_read_address(address)]);

                        let mut i2c_req = bpio::DataRequestBuilder::new(&mut builder);
                        i2c_req.add_start_main(needs_start(previous_operation, operation));
                        i2c_req.add_stop_main(false);
                        // I2C address
                        i2c_req.add_data_write(write_data);
                        i2c_req.add_bytes_read(bytes_to_read);
                        i2c_req.finish()
                    }
                    Operation::Write(bytes_to_write) => {
                        // Address + data in the write buffer
                        // NOTE: These have to be added in *REVERSE* order.
                        builder.start_vector::<u8>(1 + bytes_to_write.len());
                        for byte in bytes_to_write.iter().rev() {
                            builder.push(*byte);
                        }
                        builder.push(i2c_write_address(address));
                        let write_data = builder.end_vector(1 + bytes_to_write.len());

                        let mut i2c_req = bpio::DataRequestBuilder::new(&mut builder);
                        i2c_req.add_start_main(needs_start(previous_operation, operation));
                        i2c_req.add_stop_main(false);
                        i2c_req.add_bytes_read(0);
                        i2c_req.add_data_write(write_data);
                        i2c_req.finish()
                    }
                };

                let mut packet = bpio::RequestPacketBuilder::new(&mut builder);
                packet.add_contents_type(bpio::RequestPacketContents::DataRequest);
                packet.add_contents(i2c_req.as_union_value());
                let packet = packet.finish();

                builder.finish(packet, None);
                eprintln!(
                    "{:#?}",
                    flatbuffers::root::<bpio::RequestPacket>(builder.finished_data()).unwrap()
                );

                // Update the previous operation for Repeated-Start purposes.
                previous_operation = Some(discriminant(operation));

                let Ok(response) = self.transfer(Request(builder.finished_data())) else {
                    // Does this also need I2C transaction cleanup (issue Stop)?
                    todo!("Handle transfer error.");
                };

                let Ok(packet) = bpio::root_as_response_packet(&response.0) else {
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

            // TODO: Make this conditional on some stored state.
            // Issuing the configuration request on each transaction is slow.
            self.enter_i2c_mode(&mut builder)?;

            builder.start_vector::<u8>(write.len() + 1);
            for byte in write.iter().rev() {
                builder.push(*byte);
            }
            builder.push(i2c_write_address(address));
            let write_vector = builder.end_vector(write.len() + 1);

            let mut data_request = bpio::DataRequestBuilder::new(&mut builder);
            data_request.add_start_main(true);
            data_request.add_stop_main(true);
            data_request.add_data_write(write_vector);
            data_request.add_bytes_read(read.len() as u16);
            let data_request = data_request.finish();

            let mut packet = RequestPacketBuilder::new(&mut builder);
            packet.add_contents_type(bpio::RequestPacketContents::DataRequest);
            packet.add_contents(data_request.as_union_value());
            let packet = packet.finish();

            builder.finish_minimal(packet);
            let response_bytes = self.transfer(Request(builder.finished_data()))?;
            let response = bpio::root_as_response_packet(&response_bytes.0)?;
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
    }
}

mod error {
    #[derive(Debug)]
    pub enum Error {
        SerialPort(serialport::Error),
        Io(std::io::Error),
        Flatbuffer(flatbuffers::InvalidFlatbuffer),
        Other,
    }

    impl From<std::io::Error> for Error {
        fn from(value: std::io::Error) -> Self {
            Self::Io(value)
        }
    }

    impl From<flatbuffers::InvalidFlatbuffer> for Error {
        fn from(value: flatbuffers::InvalidFlatbuffer) -> Self {
            Self::Flatbuffer(value)
        }
    }

    impl From<serialport::Error> for Error {
        fn from(value: serialport::Error) -> Self {
            Self::SerialPort(value)
        }
    }

    impl embedded_hal::i2c::Error for Error {
        fn kind(&self) -> embedded_hal::i2c::ErrorKind {
            // TODO: This is a dummy implementation.
            embedded_hal::i2c::ErrorKind::Other
        }
    }

    impl std::fmt::Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            // TODO: This is a dummy implementation.
            write!(f, "{:?}", self)
        }
    }

    impl std::error::Error for Error {}
}
