#![allow(dead_code)]

use crate::bpio_generated::bpio;
use crate::modes::Modes;
use crate::{Error, Request, Response};

use std::io::{Read, Write};

pub(crate) fn send(mut port: impl Read + Write, request: Request) -> Result<Response, Error> {
    port.write_all(&request.cobs_encoded)?;

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

// TODO: This should take mode configuration and general configuration.
pub(crate) fn change_mode(port: impl Read + Write, mode: Modes) -> Result<(), Error> {
    let mut builder = flatbuffers::FlatBufferBuilder::with_capacity(128);
    let mode_string = builder.create_string(mode.name());
    // TODO: Actually configure the mode.
    let mode_config = bpio::ModeConfigurationBuilder::new(&mut builder).finish();

    let mut config_request = bpio::ConfigurationRequestBuilder::new(&mut builder);
    config_request.add_mode(mode_string);
    config_request.add_mode_configuration(mode_config);
    let config_request = config_request.finish();

    let mut packet = bpio::RequestPacketBuilder::new(&mut builder);
    packet.add_contents_type(bpio::RequestPacketContents::ConfigurationRequest);
    packet.add_contents(config_request.as_union_value());
    let packet = packet.finish();

    builder.finish_minimal(packet);

    let response_bytes = send(port, Request::encode(builder.finished_data()))?;
    let root = bpio::root_as_response_packet(&response_bytes.cobs_decoded)?;
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
