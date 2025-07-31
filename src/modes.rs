mod sealed {
    pub trait Sealed {}
}

pub trait ActiveMode: sealed::Sealed {
    fn mode_name(&self) -> &'static str;
}

macro_rules! impl_mode {
    ($mode:ident) => {
        impl sealed::Sealed for $mode {}
        impl ActiveMode for $mode {
            fn mode_name(&self) -> &'static str {
                ::std::stringify!($mode)
            }
        }
    };
}

pub struct I2c;
impl_mode!(I2c);

pub struct HiZ;
impl_mode!(HiZ);

#[derive(Debug, Clone, Copy)]
pub enum Modes {
    HiZ,
    I2c,
}

impl Modes {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Modes::HiZ => "HiZ",
            Modes::I2c => "I2C",
        }
    }
}

impl From<Modes> for Box<dyn ActiveMode> {
    fn from(value: Modes) -> Self {
        match value {
            Modes::HiZ => Box::new(HiZ),
            Modes::I2c => Box::new(I2c),
        }
    }
}

impl std::str::FromStr for Modes {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "HiZ" => Self::HiZ,
            "I2C" => Self::I2c,
            "1WIRE" => todo!("1WIRE"),
            "UART" => todo!("UART"),
            "HDUART" => todo!("HDUART"),
            "SPI" => todo!("SPI"),
            "2WIRE" => todo!("2WIRE"),
            "3WIRE" => todo!("3WIRE"),
            "DIO" => todo!("DIO"),
            "LED" => todo!("LED"),
            "INFRARED" => todo!("INFRARED"),
            "JTAG" => todo!("JTAG"),
            other => todo!("unexpected mode {other:?}"),
        })
    }
}
