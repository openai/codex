use super::StreamId;

use bytes::BufMut;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Head {
    kind: Kind,
    flag: u8,
    stream_id: StreamId,
}

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Kind {
    Data = 0,
    Headers = 1,
    Priority = 2,
    Reset = 3,
    Settings = 4,
    PushPromise = 5,
    Ping = 6,
    GoAway = 7,
    WindowUpdate = 8,
    Continuation = 9,
    Unknown,
}

// ===== impl Head =====

impl Head {
    pub fn new(kind: Kind, flag: u8, stream_id: StreamId) -> Head {
        Head {
            kind,
            flag,
            stream_id,
        }
    }

    /// Parse an HTTP/2 frame header
    pub fn parse(header: &[u8]) -> Head {
        let (stream_id, _) = StreamId::parse(&header[5..]);

        Head {
            kind: Kind::new(header[3]),
            flag: header[4],
            stream_id,
        }
    }

    pub fn stream_id(&self) -> StreamId {
        self.stream_id
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    pub fn flag(&self) -> u8 {
        self.flag
    }

    pub fn encode_len(&self) -> usize {
        super::HEADER_LEN
    }

    pub fn encode<T: BufMut>(&self, payload_len: usize, dst: &mut T) {
        debug_assert!(self.encode_len() <= dst.remaining_mut());

        dst.put_uint(payload_len as u64, 3);
        dst.put_u8(self.kind as u8);
        dst.put_u8(self.flag);
        dst.put_u32(self.stream_id.into());
    }
}

// ===== impl Kind =====

impl Kind {
    pub fn new(byte: u8) -> Kind {
        match byte {
            0 => Kind::Data,
            1 => Kind::Headers,
            2 => Kind::Priority,
            3 => Kind::Reset,
            4 => Kind::Settings,
            5 => Kind::PushPromise,
            6 => Kind::Ping,
            7 => Kind::GoAway,
            8 => Kind::WindowUpdate,
            9 => Kind::Continuation,
            _ => Kind::Unknown,
        }
    }
}
