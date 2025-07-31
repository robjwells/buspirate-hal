use std::{marker::PhantomData, time::Duration};

use flatbuffers::FlatBufferBuilder;
use serialport::SerialPort;

use crate::bpio_generated::bpio;
use crate::modes::{self, ActiveMode, Modes};
use crate::{transfer, Error, Request, Response};

/// HAL wrapper
pub struct BusPirate<M: ActiveMode> {
    _mode: PhantomData<M>,
    serial_port: Box<dyn SerialPort>,
}

/// Consume $this and return it with the new mode type.
macro_rules! with_mode {
    ($this:ident, $mode:ty) => {{
        let Self { _mode, serial_port } = $this;
        BusPirate::<$mode> {
            _mode: PhantomData,
            serial_port,
        }
    }};
}

pub fn open(address: &str) -> Result<BusPirate<modes::HiZ>, Error> {
    let mut serial_port = serialport::new(address, 115_200)
        // TODO: choose a sensible timeout value.
        .timeout(Duration::from_secs(1))
        .open()?;
    // Put the Bus Pirate into high-impedance mode upon opening the serial port.
    transfer::change_mode(&mut serial_port, Modes::HiZ)?;
    Ok(BusPirate::<modes::HiZ> {
        _mode: PhantomData,
        serial_port,
    })
}

impl<M: ActiveMode> BusPirate<M> {
    pub(crate) fn transfer(&mut self, request: Request) -> Result<Response, Error> {
        crate::transfer::send(&mut self.serial_port, request)
    }

    /// Put the Bus Pirate into I2C mode.
    pub fn enter_i2c_mode(
        mut self,
        speed: u32,
        clock_stretching: bool,
    ) -> Result<BusPirate<modes::I2c>, crate::error::Error> {
        let mut builder = flatbuffers::FlatBufferBuilder::with_capacity(128);
        let i2c_string = builder.create_string("I2C");
        let mut i2c_config = bpio::ModeConfigurationBuilder::new(&mut builder);
        i2c_config.add_speed(speed);
        i2c_config.add_clock_stretch(clock_stretching);
        let i2c_config = i2c_config.finish();

        let mut config_request = bpio::ConfigurationRequestBuilder::new(&mut builder);
        config_request.add_mode(i2c_string);
        config_request.add_mode_configuration(i2c_config);
        let config_request = config_request.finish();

        let mut packet = bpio::RequestPacketBuilder::new(&mut builder);
        packet.add_contents_type(bpio::RequestPacketContents::ConfigurationRequest);
        packet.add_contents(config_request.as_union_value());
        let packet = packet.finish();

        builder.finish_minimal(packet);

        self.transfer(Request::encode(builder.finished_data()))?;
        builder.reset();

        Ok(with_mode!(self, modes::I2c))
    }
}

impl BusPirate<modes::I2c> {
    pub(crate) fn i2c_stop(&mut self, builder: &mut FlatBufferBuilder) -> Result<Response, Error> {
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

        let response = self.transfer(Request::encode(builder.finished_data()))?;
        builder.reset();
        Ok(response)
    }
}
