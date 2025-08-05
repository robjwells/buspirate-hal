use crate::{bpio::DataRequest, modes::Spi, BusPirate, Error};

impl embedded_hal::spi::Error for Error {
    fn kind(&self) -> embedded_hal::spi::ErrorKind {
        // TODO: This is a dummy implementation.
        embedded_hal::spi::ErrorKind::Other
    }
}

impl embedded_hal::spi::ErrorType for BusPirate<Spi> {
    type Error = Error;
}

fn copy(received: Option<Vec<u8>>, buf: &mut [u8]) -> Result<(), Error> {
    if let Some(data) = received {
        buf.copy_from_slice(&data);
        Ok(())
    } else if buf.is_empty() {
        // No data received, but we didn't expect any.
        Ok(())
    } else {
        Err(Error::NoDataReceived)
    }
}

impl embedded_hal::spi::SpiBus for BusPirate<Spi> {
    fn read(&mut self, words: &mut [u8]) -> Result<(), Self::Error> {
        let request = DataRequest::builder()
            .start(true)
            .stop(true)
            .bytes_to_read(words.len())
            .build();

        self.send_data_request(request)
            .map(|received| copy(received, words))?
    }

    fn write(&mut self, words: &[u8]) -> Result<(), Self::Error> {
        let request = DataRequest::builder()
            .start(true)
            .stop(true)
            .bytes_to_write(words)
            .build();
        self.send_data_request(request).map(drop)
    }

    fn transfer(&mut self, read: &mut [u8], write: &[u8]) -> Result<(), Self::Error> {
        let request = DataRequest::builder()
            .start(true)
            .stop(true)
            .bytes_to_read(read.len())
            .bytes_to_write(write)
            .build();

        self.send_data_request(request)
            .map(|received| copy(received, read))?
    }

    fn transfer_in_place(&mut self, words: &mut [u8]) -> Result<(), Self::Error> {
        let request = DataRequest::builder()
            .start(true)
            .stop(true)
            .bytes_to_read(words.len())
            .bytes_to_write(words)
            .build();

        self.send_data_request(request)
            .map(|received| copy(received, words))?
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        // Flush is a no-op because communication with the Bus Pirate is synchronous.
        Ok(())
    }
}
