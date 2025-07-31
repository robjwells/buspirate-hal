pub(crate) struct Request {
    pub(crate) cobs_encoded: Vec<u8>,
}

impl Request {
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
