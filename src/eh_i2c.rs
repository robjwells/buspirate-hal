use std::mem::{discriminant, Discriminant};

use embedded_hal::i2c::{ErrorType, I2c, Operation};

use crate::{bpio::I2cRequest, error::Error, modes, BusPirate};

trait I2cAddress {
    fn for_reading(&self) -> u8;
    fn for_writing(&self) -> u8;
    fn for_operation(&self, op: &Operation<'_>) -> u8;
}

impl I2cAddress for u8 {
    fn for_reading(&self) -> u8 {
        (self << 1) + 1
    }

    fn for_writing(&self) -> u8 {
        self << 1
    }

    fn for_operation(&self, op: &Operation<'_>) -> u8 {
        match op {
            Operation::Read(_) => self.for_reading(),
            Operation::Write(_) => self.for_writing(),
        }
    }
}

impl ErrorType for BusPirate<modes::I2c> {
    type Error = Error;
}

impl I2c for BusPirate<modes::I2c> {
    fn transaction(
        &mut self,
        address: u8,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        // TODO: Audit error points for I2C transaction cleanup.

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
            // Build I2C request.
            // Performing the same builder steps, in the same order, even where the
            // values are redundant, is important to keep the types the same.
            let request = match operation {
                Operation::Read(read_buffer) => I2cRequest::builder()
                    .stop(false)
                    .bytes_to_read(read_buffer.len())
                    .bytes_to_write(&[]),
                Operation::Write(bytes_to_write) => I2cRequest::builder()
                    .stop(false)
                    .bytes_to_read(0)
                    .bytes_to_write(bytes_to_write),
            };
            // If we're issuing a start, we also need to supply the address. If we're not,
            // the address was already sent on the bus. This logic is the same for
            // both reading and writing.
            let request = if needs_start(previous_operation, operation) {
                // If we're issuing a start, we also need to supply the address.
                // If we're not, then the address was already sent.
                request
                    .start(true)
                    .address(address.for_operation(operation))
                    .build()
            } else {
                request.start(false).build()
            };

            // Update the previous operation for Repeated-Start purposes.
            previous_operation = Some(discriminant(operation));

            let Ok(read_data) = self.send_data_request(request) else {
                // Does this also need I2C transaction cleanup (issue Stop)?
                todo!("Handle transfer error.");
            };

            if let Operation::Read(read_buffer) = operation {
                // if Some(read_data) = i2c_resp.data()
                let Some(data) = read_data else {
                    todo!("Missing data, return an error.");
                };
                read_buffer.copy_from_slice(&data);
            }
        }

        self.i2c_stop()
    }

    fn write_read(
        &mut self,
        address: u8,
        write: &[u8],
        read: &mut [u8],
    ) -> Result<(), Self::Error> {
        // TODO: What if `read` is longer than u16::MAX?

        let request = I2cRequest::builder()
            .start(true)
            .stop(true)
            .address(address.for_writing())
            .bytes_to_write(write)
            .bytes_to_read(read.len())
            .build();

        // TODO: Handle mismatched amounts of read data
        if let Some(data) = self.send_data_request(request)? {
            read.copy_from_slice(&data);
            Ok(())
        } else {
            eprintln!("Couldn't get data response from contents.");
            Err(Error::Other)
        }
    }

    fn read(&mut self, address: u8, read: &mut [u8]) -> Result<(), Self::Error> {
        let request = I2cRequest::builder()
            .start(true)
            .stop(true)
            .address(address.for_reading())
            .bytes_to_read(read.len())
            .build();

        if let Some(data) = self.send_data_request(request)? {
            read.copy_from_slice(&data);
            Ok(())
        } else {
            // TODO: Handle error properly.
            eprintln!("Couldn't get data response from contents.");
            Err(Error::Other)
        }
    }

    fn write(&mut self, address: u8, write: &[u8]) -> Result<(), Self::Error> {
        let request = I2cRequest::builder()
            .start(true)
            .stop(true)
            .address(address.for_writing())
            .bytes_to_write(write)
            .build();

        let _ = self.send_data_request(request)?;
        Ok(())
    }
}
