#[allow(clippy::all)]
#[allow(unused_imports)]
#[allow(mismatched_lifetime_syntaxes)]
mod bpio_generated;

use std::io::{Read, Write};

use bit_field::BitField;
use flatbuffers::FlatBufferBuilder;

use crate::modes::Modes;
use crate::{EncodedRequest, Error, Response};
use bpio_generated::bpio as generated;

// BPIO interface major version
const VERSION_MAJOR: u8 = 2;
// BPIO interface minor version
const VERSION_MINOR: u8 = 0;

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

impl From<generated::ErrorResponse<'_>> for crate::Error {
    fn from(value: generated::ErrorResponse<'_>) -> Self {
        Self::BpioErrorMessage(value.error().unwrap_or_default().to_owned())
    }
}

impl From<generated::ResponsePacketContents> for Error {
    fn from(value: generated::ResponsePacketContents) -> Self {
        Self::UnexpectedResponseType(
            value
                .variant_name()
                .expect("Variants must have defined names."),
        )
    }
}

/// Handle the ways in which a ResponsePacket can be erroneous.
///
/// This should be used *after* handling an error from the flatbuffer
/// decoding process.
macro_rules! check_response {
    ($packet:ident, $e:expr) => {{
        match $e {
            Some(v) => {
                if let Some(error_message) = v.error() {
                    // Correct response type, but contains an error message.
                    Err(Error::BpioErrorMessage(error_message.to_owned()))
                } else {
                    // Correct response type, no error.
                    Ok(v)
                }
            }
            None => {
                if let Some(error_response) = $packet.contents_as_error_response() {
                    // Response type is an error response.
                    Err(error_response.into())
                } else {
                    // Response type is not what was expected.
                    Err($packet.contents_type().into())
                }
            }
        }
    }};
}

pub(crate) fn send_data_request(
    port: impl Read + Write,
    req: EncodedRequest,
) -> Result<Option<Vec<u8>>, Error> {
    let response = send(port, req)?;
    let packet = generated::root_as_response_packet(&response.cobs_decoded)?;
    let data_response = check_response!(packet, packet.contents_as_data_response())?;
    Ok(data_response
        .data_read()
        .filter(|v| !v.is_empty())
        .map(|v| v.iter().collect()))
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
        let write_vector = Self::add_i2c_write_vector(builder, self.address, self.bytes_to_write);

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

    fn add_i2c_write_vector<'a>(
        builder: &mut FlatBufferBuilder<'a>,
        address: Option<u8>,
        data: Option<&[u8]>,
    ) -> Option<flatbuffers::WIPOffset<flatbuffers::Vector<'a, u8>>> {
        // TODO: Clean this up. It's horrible.
        let num_bytes @ 1.. = (if address.is_some() {
            1
        } else {
            Default::default()
        }) + data.map(|d| d.len()).unwrap_or_default() else {
            return None;
        };

        builder.start_vector::<u8>(num_bytes);
        if let Some(bytes) = data {
            for &byte in bytes.iter().rev() {
                builder.push(byte);
            }
        }
        if let Some(address) = address {
            builder.push(address);
        }
        Some(builder.end_vector(num_bytes))
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

#[derive(Debug, bon::Builder)]
pub(crate) struct DataRequest<'a> {
    start: bool,
    stop: bool,
    bytes_to_write: Option<&'a [u8]>,
    #[builder(with = |n: usize| n as u16)]
    bytes_to_read: Option<u16>,
}

impl<'a> From<DataRequest<'a>> for EncodedRequest {
    fn from(request: DataRequest<'a>) -> Self {
        let mut builder = FlatBufferBuilder::new();
        let write_vector = request
            .bytes_to_write
            .map(|data| builder.create_vector(data));

        let mut data_request = generated::DataRequestBuilder::new(&mut builder);
        data_request.add_start_main(request.start);
        data_request.add_stop_main(request.stop);
        if let Some(bytes_read) = request.bytes_to_read {
            data_request.add_bytes_read(bytes_read);
        }
        if let Some(wv) = write_vector {
            data_request.add_data_write(wv);
        }
        let data_request = data_request.finish();
        let packet = build_data_request_packet(&mut builder, data_request);

        builder.finish_minimal(packet);
        EncodedRequest::encode(builder.finished_data())
    }
}

fn build_data_request_packet<'a>(
    builder: &mut FlatBufferBuilder<'a>,
    data_request: flatbuffers::WIPOffset<generated::DataRequest<'a>>,
) -> flatbuffers::WIPOffset<generated::RequestPacket<'a>> {
    let mut packet = generated::RequestPacketBuilder::new(builder);
    packet.add_version_major(VERSION_MAJOR);
    packet.add_version_minor(VERSION_MINOR);
    packet.add_contents_type(generated::RequestPacketContents::DataRequest);
    packet.add_contents(data_request.as_union_value());
    packet.finish()
}

#[derive(Debug, Clone, Copy)]
pub enum BitOrder {
    Msb,
    Lsb,
}

#[derive(Debug, Clone, Copy)]
pub enum IoDirection {
    Output,
    Input,
}

impl IoDirection {
    /// Returns `true` if the io direction is [`Output`].
    ///
    /// [`Output`]: IoDirection::Output
    #[must_use]
    fn is_output(&self) -> bool {
        matches!(self, Self::Output)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum LogicLevel {
    High,
    Low,
}

impl LogicLevel {
    /// Returns `true` if the logic level is [`High`].
    ///
    /// [`High`]: LogicLevel::High
    #[must_use]
    fn is_high(&self) -> bool {
        matches!(self, Self::High)
    }
}

#[derive(Debug, bon::Builder)]
pub struct PsuConfig {
    enable: Option<bool>,
    millivolts: Option<u32>,
    milliamps: Option<u16>,
}

impl PsuConfig {
    pub fn enable(millivolts: u32, milliamps: u16) -> Self {
        Self {
            enable: Some(true),
            millivolts: Some(millivolts),
            milliamps: Some(milliamps),
        }
    }

    pub fn disable() -> Self {
        Self {
            enable: Some(false),
            millivolts: None,
            milliamps: None,
        }
    }

    fn apply<T>(&self, cfg: &mut generated::ConfigurationRequestBuilder<T>)
    where
        T: flatbuffers::Allocator,
    {
        if let Some(enable_psu) = self.enable {
            if enable_psu {
                cfg.add_psu_enable(true);
            } else {
                cfg.add_psu_disable(true);
            }
        }
        if let Some(mv) = self.millivolts {
            cfg.add_psu_set_mv(mv);
        }
        if let Some(ma) = self.milliamps {
            cfg.add_psu_set_ma(ma);
        }
    }
}

#[derive(Debug)]
pub struct IoConfig {
    direction_mask: u8,
    direction: u8,
    value_mask: u8,
    value: u8,
}

impl IoConfig {
    pub fn set_direction(&mut self, pin: usize, direction: IoDirection) {
        assert!(matches!(pin, 0..8), "Pin must be in range 0..8");
        self.direction_mask.set_bit(pin, true);
        self.direction.set_bit(pin, direction.is_output());
    }

    pub fn set_level(&mut self, pin: usize, level: LogicLevel) {
        assert!(matches!(pin, 0..8), "Pin must be in range 0..8");
        self.value_mask.set_bit(pin, true);
        self.value.set_bit(pin, level.is_high());
    }

    fn apply<T>(&self, cfg: &mut generated::ConfigurationRequestBuilder<T>)
    where
        T: flatbuffers::Allocator,
    {
        cfg.add_io_direction_mask(self.direction_mask);
        cfg.add_io_direction(self.direction);
        cfg.add_io_value_mask(self.value_mask);
        cfg.add_io_value(self.value);
    }
}

#[derive(Debug, bon::Builder)]
pub struct Configuration<'a> {
    mode_bit_order: Option<BitOrder>,
    psu: Option<PsuConfig>,
    pullup: Option<bool>,
    io: Option<IoConfig>,
    led_resume: Option<bool>,
    led_color: Option<&'a [u32]>,
    print_string: Option<&'a str>,
    hardware_bootloader: Option<bool>,
    hardware_reset: Option<bool>,
}

impl<'a> Configuration<'a> {
    fn empty() -> Self {
        Self::builder().build()
    }
}

struct FullConfiguration<'a> {
    config: Configuration<'a>,
    mode: Option<(Modes, ModeConfiguration)>,
}

impl<'a> From<FullConfiguration<'a>> for EncodedRequest {
    fn from(request: FullConfiguration<'a>) -> Self {
        let mut fbb = FlatBufferBuilder::with_capacity(256);

        let FullConfiguration { config, mode } = request;

        // Create nested items first to avoid a borrowing conflict with the config builder.
        let mode = mode.map(|(mode, mode_config)| {
            let mode = fbb.create_string(mode.name());
            let mode_config = mode_config.apply(&mut fbb);
            (mode, mode_config)
        });
        let print_string = config.print_string.map(|s| fbb.create_string(s));
        let led_color = config.led_color.map(|colors| fbb.create_vector(colors));

        let mut builder = generated::ConfigurationRequestBuilder::new(&mut fbb);
        if let Some((mode, mode_config)) = mode {
            builder.add_mode(mode);
            builder.add_mode_configuration(mode_config);
        }
        if let Some(bit_order) = config.mode_bit_order {
            match bit_order {
                BitOrder::Msb => builder.add_mode_bitorder_msb(true),
                BitOrder::Lsb => builder.add_mode_bitorder_lsb(true),
            };
        }
        if let Some(psu) = config.psu {
            psu.apply(&mut builder);
        }
        if let Some(turn_pullup_on) = config.pullup {
            if turn_pullup_on {
                builder.add_pullup_enable(true);
            } else {
                builder.add_pullup_disable(true);
            }
        }
        if let Some(io_config) = config.io {
            io_config.apply(&mut builder);
        }
        if let Some(led_resume) = config.led_resume {
            builder.add_led_resume(led_resume);
        }
        if let Some(led_color) = led_color {
            builder.add_led_color(led_color);
        }
        if let Some(print_string) = print_string {
            builder.add_print_string(print_string);
        }
        if let Some(hardware_bootloader) = config.hardware_bootloader {
            builder.add_hardware_bootloader(hardware_bootloader);
        }
        if let Some(hardware_reset) = config.hardware_reset {
            builder.add_hardware_reset(hardware_reset);
        }
        let cfg = builder.finish();

        let mut packet = generated::RequestPacketBuilder::new(&mut fbb);
        packet.add_version_major(VERSION_MAJOR);
        packet.add_version_minor(VERSION_MINOR);
        packet.add_contents_type(generated::RequestPacketContents::ConfigurationRequest);
        packet.add_contents(cfg.as_union_value());
        let packet = packet.finish();
        fbb.finish_minimal(packet);
        EncodedRequest::encode(fbb.finished_data())
    }
}

// TODO: Turn primitives into meaningful types, where appropriate.
#[derive(Debug, bon::Builder)]
pub struct ModeConfiguration {
    speed: Option<u32>,
    data_bits: Option<u8>,
    parity: Option<bool>,
    stop_bits: Option<u8>,
    flow_control: Option<bool>,
    signal_inversion: Option<bool>,
    clock_stretch: Option<bool>,
    clock_polarity: Option<bool>,
    clock_phase: Option<bool>,
    chip_select_idle: Option<bool>,
    submode: Option<u8>,
    tx_modulation: Option<u32>,
    rx_sensor: Option<u8>,
}

impl ModeConfiguration {
    pub fn empty() -> Self {
        Self::builder().build()
    }
}

impl ModeConfiguration {
    fn apply<'a>(
        &self,
        builder: &mut FlatBufferBuilder<'a>,
    ) -> flatbuffers::WIPOffset<generated::ModeConfiguration<'a>> {
        let mut cfg = generated::ModeConfigurationBuilder::new(builder);

        if let Some(speed) = self.speed {
            cfg.add_speed(speed);
        }
        if let Some(data_bits) = self.data_bits {
            cfg.add_data_bits(data_bits);
        }
        if let Some(parity) = self.parity {
            cfg.add_parity(parity);
        }
        if let Some(stop_bits) = self.stop_bits {
            cfg.add_stop_bits(stop_bits);
        }
        if let Some(flow_control) = self.flow_control {
            cfg.add_flow_control(flow_control);
        }
        if let Some(signal_inversion) = self.signal_inversion {
            cfg.add_signal_inversion(signal_inversion);
        }
        if let Some(clock_stretch) = self.clock_stretch {
            cfg.add_clock_stretch(clock_stretch);
        }
        if let Some(clock_polarity) = self.clock_polarity {
            cfg.add_clock_polarity(clock_polarity);
        }
        if let Some(clock_phase) = self.clock_phase {
            cfg.add_clock_phase(clock_phase);
        }
        if let Some(chip_select_idle) = self.chip_select_idle {
            cfg.add_chip_select_idle(chip_select_idle);
        }
        if let Some(submode) = self.submode {
            cfg.add_submode(submode);
        }
        if let Some(tx_modulation) = self.tx_modulation {
            cfg.add_tx_modulation(tx_modulation);
        }
        if let Some(rx_sensor) = self.rx_sensor {
            cfg.add_rx_sensor(rx_sensor);
        }

        cfg.finish()
    }
}

pub(crate) fn send_configuration_request(
    port: impl Read + Write,
    config: Configuration,
) -> Result<(), Error> {
    send_full_configuration_request(port, config, None)
}

// TODO: Method to change configuration of current mode, without changing the mode itself(?)

fn send_full_configuration_request(
    port: impl Read + Write,
    config: Configuration,
    mode: Option<(Modes, ModeConfiguration)>,
) -> Result<(), Error> {
    let full_config = FullConfiguration { config, mode };
    let response_bytes = send(port, full_config.into())?;
    let packet = generated::root_as_response_packet(&response_bytes.cobs_decoded)?;
    check_response!(packet, packet.contents_as_configuration_response()).map(drop)
}

pub(crate) fn change_mode(
    port: impl Read + Write,
    mode: Modes,
    mode_config: ModeConfiguration,
    extra_config: Option<Configuration<'_>>,
) -> Result<(), Error> {
    let config = extra_config.unwrap_or_else(Configuration::empty);
    send_full_configuration_request(port, config, Some((mode, mode_config)))
}
