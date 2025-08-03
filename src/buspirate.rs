use std::{marker::PhantomData, time::Duration};

use serialport::SerialPort;

use crate::bpio::{self, ConfigurationRequest};
use crate::modes::{self, ActiveMode, I2c, Modes};
use crate::{EncodedRequest, Error, ModeConfiguration};

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
    bpio::change_mode(
        &mut serial_port,
        Modes::HiZ,
        ModeConfiguration::empty(),
        None,
    )?;
    Ok(BusPirate::<modes::HiZ> {
        _mode: PhantomData,
        serial_port,
    })
}

impl<M: ActiveMode> BusPirate<M> {
    pub(crate) fn send_data_request(
        &mut self,
        request: impl Into<EncodedRequest>,
    ) -> Result<Option<Vec<u8>>, Error> {
        bpio::send_data_request(&mut self.serial_port, request.into())
    }

    pub fn configure(&mut self, request: ConfigurationRequest) -> Result<(), Error> {
        bpio::send_configuration_request(&mut self.serial_port, request)
    }

    fn set_mode(
        &mut self,
        mode: Modes,
        mode_config: ModeConfiguration,
        extra_config: Option<ConfigurationRequest>,
    ) -> Result<(), Error> {
        bpio::change_mode(&mut self.serial_port, mode, mode_config, extra_config)
    }

    /// Put the Bus Pirate into I2C mode.
    pub fn enter_i2c_mode(
        mut self,
        speed: u32,
        clock_stretching: bool,
        extra_config: Option<ConfigurationRequest>,
    ) -> Result<BusPirate<modes::I2c>, crate::error::Error> {
        self.set_mode(
            Modes::I2c,
            ModeConfiguration::builder()
                .speed(speed)
                .clock_stretch(clock_stretching)
                .build(),
            extra_config,
        )?;
        Ok(with_mode!(self, I2c))
    }
}

impl BusPirate<modes::I2c> {
    pub(crate) fn i2c_stop(&mut self) -> Result<(), Error> {
        let request = bpio::I2cRequest::builder().start(false).stop(true).build();
        self.send_data_request(request)?;
        Ok(())
    }
}
