// TODO: Patch out the warning-generating code.
#[allow(clippy::all)]
#[allow(unused_imports)]
mod bpio_generated;

use std::io::{Read, Write};

use bpio_generated::bpio as generated;
use flatbuffers::FlatBufferBuilder;

use crate::{modes::Modes, EncodedRequest, Error, Response};

fn send(mut port: impl Read + Write, req: EncodedRequest) -> Result<Response, Error> {
    port.write_all(&req.cobs_encoded)?;

    // 1kB decode buffer
    let mut decoded_bytes = [0u8; 1024];
    let mut decoder = cobs::CobsDecoder::new(&mut decoded_bytes);
    let mut read_buf = [0u8; 256];
    // TODO: This should be bounded.
    loop {
        // TODO: Log n bytes read to get an idea of how large read_buf should be.
        let bytes_read = port.read(&mut read_buf)?;
        if let Some(report) = decoder.push(&read_buf[..bytes_read])? {
            let packet = decoded_bytes[..report.frame_size()].to_vec();
            return Ok(Response::new(packet));
        }
    }
}

pub(crate) fn send_data_request(
    port: impl Read + Write,
    req: EncodedRequest,
) -> Result<Option<Vec<u8>>, Error> {
    let response = send(port, req)?;
    let packet = generated::root_as_response_packet(&response.cobs_decoded)?;
    if let Some(data_response) = packet.contents_as_data_response() {
        if let Some(error_message) = data_response.error() {
            return Err(Error::BpioErrorMessage(error_message.to_owned()));
        }
        Ok(data_response
            .data_read()
            .filter(|v| !v.is_empty())
            .map(|v| v.iter().collect()))
    } else if let Some(error_response) = packet.contents_as_error_response() {
        Err(Error::BpioErrorMessage(
            error_response.error().unwrap_or_default().to_owned(),
        ))
    } else {
        Err(Error::UnexpectedResponseType(
            packet
                .contents_type()
                .variant_name()
                .expect("Variants must have defined names."),
        ))
    }
}

#[derive(Debug, bon::Builder)]
pub(crate) struct I2cRequest<'a> {
    start: bool,
    stop: bool,
    address: Option<u8>,
    bytes_to_write: Option<&'a [u8]>,
    #[builder(with = |n: usize| n as u16)]
    bytes_to_read: Option<u16>,
}

impl I2cRequest<'_> {
    fn build_packet<'a>(
        &self,
        builder: &mut FlatBufferBuilder<'a>,
    ) -> flatbuffers::WIPOffset<generated::RequestPacket<'a>> {
        let write_vector = self.create_i2c_write_vector(builder);

        let mut data_request = generated::DataRequestBuilder::new(builder);
        data_request.add_start_main(self.start);
        data_request.add_stop_main(self.stop);

        if let Some(bytes_read) = self.bytes_to_read {
            data_request.add_bytes_read(bytes_read);
        }

        if let Some(wv) = write_vector {
            data_request.add_data_write(wv);
        }

        let data_request = data_request.finish();
        build_data_request_packet(builder, data_request)
    }

    fn create_i2c_write_vector<'a>(
        &self,
        builder: &mut FlatBufferBuilder<'a>,
    ) -> Option<flatbuffers::WIPOffset<flatbuffers::Vector<'a, u8>>> {
        let bytes_to_write = {
            let a = if self.address.is_some() { 1 } else { 0 };
            let b = self.bytes_to_write.map_or(0, |s| s.len());
            a + b
        };
        if bytes_to_write == 0 {
            return None;
        }

        builder.start_vector::<u8>(bytes_to_write);
        if let Some(bytes) = self.bytes_to_write {
            for &byte in bytes.iter().rev() {
                builder.push(byte);
            }
        }
        if let Some(address) = self.address {
            builder.push(address);
        }

        Some(builder.end_vector(bytes_to_write))
    }
}

impl<'a> From<I2cRequest<'a>> for EncodedRequest {
    fn from(request: I2cRequest<'a>) -> Self {
        let mut builder = FlatBufferBuilder::new();
        let packet = request.build_packet(&mut builder);
        builder.finish_minimal(packet);
        EncodedRequest::encode(builder.finished_data())
    }
}

fn build_data_request_packet<'a>(
    builder: &mut FlatBufferBuilder<'a>,
    data_request: flatbuffers::WIPOffset<generated::DataRequest<'a>>,
) -> flatbuffers::WIPOffset<generated::RequestPacket<'a>> {
    let mut packet = generated::RequestPacketBuilder::new(builder);
    packet.add_contents_type(generated::RequestPacketContents::DataRequest);
    packet.add_contents(data_request.as_union_value());
    packet.finish()
}

// TODO: This should take mode configuration and general configuration.
pub(crate) fn change_mode(port: impl Read + Write, mode: Modes) -> Result<(), Error> {
    let mut builder = flatbuffers::FlatBufferBuilder::with_capacity(128);
    let mode_string = builder.create_string(mode.name());
    // TODO: Actually configure the mode.
    let mode_config = generated::ModeConfigurationBuilder::new(&mut builder).finish();

    let mut config_request = generated::ConfigurationRequestBuilder::new(&mut builder);
    config_request.add_mode(mode_string);
    config_request.add_mode_configuration(mode_config);
    let config_request = config_request.finish();

    let mut packet = generated::RequestPacketBuilder::new(&mut builder);
    packet.add_contents_type(generated::RequestPacketContents::ConfigurationRequest);
    packet.add_contents(config_request.as_union_value());
    let packet = packet.finish();

    builder.finish_minimal(packet);

    let response_bytes = send(port, EncodedRequest::encode(builder.finished_data()))?;
    let root = generated::root_as_response_packet(&response_bytes.cobs_decoded)?;
    if let Some(_error_response) = root.contents_as_error_response() {
        todo!("Return some error variant");
    }
    if let Some(_configuration_error_message) =
        root.contents_as_configuration_response().unwrap().error()
    {
        todo!(
            "Report configuration error {:?}",
            _configuration_error_message
        );
    }
    Ok(())
}
