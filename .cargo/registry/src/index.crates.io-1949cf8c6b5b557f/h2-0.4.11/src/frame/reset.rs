use crate::frame::{self, Error, Head, Kind, Reason, StreamId};

use bytes::BufMut;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Reset {
    stream_id: StreamId,
    error_code: Reason,
}

impl Reset {
    pub fn new(stream_id: StreamId, error: Reason) -> Reset {
        Reset {
            stream_id,
            error_code: error,
        }
    }

    pub fn stream_id(&self) -> StreamId {
        self.stream_id
    }

    pub fn reason(&self) -> Reason {
        self.error_code
    }

    pub fn load(head: Head, payload: &[u8]) -> Result<Reset, Error> {
        if payload.len() != 4 {
            return Err(Error::InvalidPayloadLength);
        }

        let error_code = unpack_octets_4!(payload, 0, u32);

        Ok(Reset {
            stream_id: head.stream_id(),
            error_code: error_code.into(),
        })
    }

    pub fn encode<B: BufMut>(&self, dst: &mut B) {
        tracing::trace!(
            "encoding RESET; id={:?} code={:?}",
            self.stream_id,
            self.error_code
        );
        let head = Head::new(Kind::Reset, 0, self.stream_id);
        head.encode(4, dst);
        dst.put_u32(self.error_code.into());
    }
}

impl<B> From<Reset> for frame::Frame<B> {
    fn from(src: Reset) -> Self {
        frame::Frame::Reset(src)
    }
}
