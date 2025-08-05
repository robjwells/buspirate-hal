pub(crate) struct EncodedRequest {
    pub(crate) cobs_encoded: Vec<u8>,
}

impl EncodedRequest {
    pub(crate) fn encode(data: &[u8]) -> Self {
        let mut cobs_encoded = cobs::encode_vec(data);
        // Push terminal.
        cobs_encoded.push(0x00);
        Self { cobs_encoded }
    }
}

pub(crate) struct Response {
    pub(crate) cobs_decoded: Vec<u8>,
}

impl Response {
    pub(crate) fn new(data: Vec<u8>) -> Self {
        Self { cobs_decoded: data }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ChipSelectPolarity {
    ActiveLow,
    ActiveHigh,
}

impl ChipSelectPolarity {
    /// Convert into a bool for BPIO requests.
    ///
    /// BPIO uses the _idle_ state, not the active state. Active low means the
    /// line is high when idle (true).
    pub(crate) fn for_bpio(self) -> bool {
        match self {
            ChipSelectPolarity::ActiveLow => true,
            ChipSelectPolarity::ActiveHigh => false,
        }
    }
}


#[derive(Debug, Clone, Copy)]
pub enum ClockPolarity {
    ActiveLow,
    ActiveHigh,
}

impl ClockPolarity {
    /// Convert into a bool for BPIO requests.
    ///
    /// BPIO uses the _idle_ state, not the active state. Active low means the
    /// line is high when idle (true).
    pub(crate) fn for_bpio(self) -> bool {
        match self {
            ClockPolarity::ActiveLow => true,
            ClockPolarity::ActiveHigh => false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ClockPhase {
    LeadingEdge,
    TrailingEdge,
}

impl ClockPhase {
    /// Convert into bool for BPIO requests.
    ///
    /// BPIO uses false for leading edges, true for trailing edges. Note that they
    /// are not referred to as rising or falling because that depends on the
    /// clock polarity.
    pub(crate) fn for_bpio(self) -> bool {
        match self {
            ClockPhase::LeadingEdge => false,
            ClockPhase::TrailingEdge => true,
        }
    }
}
