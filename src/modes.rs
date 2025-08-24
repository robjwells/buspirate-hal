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

pub struct HiZ;
impl_mode!(HiZ);

pub struct I2c;
impl_mode!(I2c);

pub struct Spi;
impl_mode!(Spi);

#[derive(Debug, Clone, Copy)]
pub enum Modes {
    HiZ,
    I2c,
    Spi,
}

impl Modes {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Modes::HiZ => "HiZ",
            Modes::I2c => "I2C",
            Modes::Spi => "SPI",
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
            "SPI" => Self::Spi,
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

impl std::fmt::Display for Modes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Modes::HiZ => "HiZ",
            Modes::I2c => "I2C",
            Modes::Spi => "SPI",
        };
        write!(f, "{name}")
    }
}
