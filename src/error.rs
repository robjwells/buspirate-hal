#[derive(Debug)]
pub enum Error {
    SerialPort(serialport::Error),
    Io(std::io::Error),
    Flatbuffer(flatbuffers::InvalidFlatbuffer),
    Cobs(cobs::DecodeError),
    FlatbufferUnexpectedContents,
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

impl From<cobs::DecodeError> for Error {
    fn from(value: cobs::DecodeError) -> Self {
        Self::Cobs(value)
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
