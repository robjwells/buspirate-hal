// TODO: Patch out the warning-generating code.
#[allow(clippy::all)]
#[allow(unused_imports)]
mod bpio_generated;
mod eh_i2c;
mod error;

use std::time::Duration;

use bpio_generated::bpio;
pub use error::Error;

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
