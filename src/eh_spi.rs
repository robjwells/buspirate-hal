use std::time::Duration;

use embedded_hal::spi::{Operation, SpiBus, SpiDevice};
use log::debug;

use crate::{BusPirate, Error, bpio::DataRequest, modes::Spi};

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

impl SpiBus for BusPirate<Spi> {
    fn read(&mut self, words: &mut [u8]) -> Result<(), Self::Error> {
        debug!("SPI Read r:{}", words.len());

        let request = DataRequest::builder()
            .start(true)
            .stop(true)
            .bytes_to_read(words.len())
            .build();

        self.send_data_request(request)
            .and_then(|received| copy(received, words))
    }

    fn write(&mut self, words: &[u8]) -> Result<(), Self::Error> {
        debug!("SPI Write w:{}", words.len());

        let request = DataRequest::builder()
            .start(true)
            .stop(true)
            .bytes_to_write(words)
            .build();
        self.send_data_request(request).map(drop)
    }

    fn transfer(&mut self, read: &mut [u8], write: &[u8]) -> Result<(), Self::Error> {
        debug!("SPI Transfer w:{} r:{}", write.len(), read.len());

        // start_alt or { reads bytes as a byte is written (full-duplex).
        let request = DataRequest::builder()
            .start(false)
            .start_alt(true)
            .stop(true)
            .bytes_to_read(read.len())
            .bytes_to_write(write)
            .build();

        self.send_data_request(request)
            .and_then(|received| copy(received, read))
    }

    fn transfer_in_place(&mut self, words: &mut [u8]) -> Result<(), Self::Error> {
        debug!("SPI Transfer in place w/r:{}", words.len());

        // start_alt or { reads bytes as a byte is written (full-duplex).
        let request = DataRequest::builder()
            .start(false)
            .start_alt(true)
            .stop(true)
            .bytes_to_read(words.len())
            .bytes_to_write(words)
            .build();

        self.send_data_request(request)
            .and_then(|received| copy(received, words))
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        // Flush is a no-op because communication with the Bus Pirate is synchronous.
        Ok(())
    }
}

impl SpiDevice for BusPirate<Spi> {
    fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), Self::Error> {
        if operations.is_empty() {
            return Ok(());
        }

        let stop_request = DataRequest::builder().start(false).stop(true).build();

        for op in operations {
            let res = match op {
                Operation::Read(read) => {
                    let req = DataRequest::builder()
                        .start(true)
                        .stop(false)
                        .bytes_to_read(read.len())
                        .build();
                    self.send_data_request(req).and_then(|r| copy(r, read))
                }
                Operation::Write(write) => {
                    let req = DataRequest::builder()
                        .start(true)
                        .stop(false)
                        .bytes_to_write(write)
                        .build();
                    self.send_data_request(req).map(drop)
                }
                Operation::Transfer(read, write) => {
                    let req = DataRequest::builder()
                        .start(false)
                        .start_alt(true)
                        .stop(false)
                        .bytes_to_read(read.len())
                        .bytes_to_write(write)
                        .build();
                    self.send_data_request(req).and_then(|r| copy(r, read))
                }
                Operation::TransferInPlace(words) => {
                    let req = DataRequest::builder()
                        .start(false)
                        .start_alt(true)
                        .stop(false)
                        .bytes_to_read(words.len())
                        .bytes_to_write(words)
                        .build();
                    self.send_data_request(req).and_then(|r| copy(r, words))
                }
                Operation::DelayNs(ns) => {
                    std::thread::sleep(Duration::from_nanos(*ns as u64));
                    Ok(())
                }
            };

            // Try to clean up if there was an error.
            if let error @ Err(..) = res {
                // Attempt to release the chip select line.
                let _ = self.send_data_request(stop_request);
                // If that fails, ignore it as we're already in an error state.
                return error;
            }
        }

        // Release the chip select line.
        self.send_data_request(stop_request).map(drop)
    }

    // For the single-operation methods, just use the SpiBus methods as the implementation
    // would be identical.
    fn read(&mut self, buf: &mut [u8]) -> Result<(), Self::Error> {
        <Self as SpiBus>::read(self, buf)
    }

    fn write(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        <Self as SpiBus>::write(self, buf)
    }

    fn transfer(&mut self, read: &mut [u8], write: &[u8]) -> Result<(), Self::Error> {
        <Self as SpiBus>::transfer(self, read, write)
    }

    fn transfer_in_place(&mut self, buf: &mut [u8]) -> Result<(), Self::Error> {
        <Self as SpiBus>::transfer_in_place(self, buf)
    }
}
