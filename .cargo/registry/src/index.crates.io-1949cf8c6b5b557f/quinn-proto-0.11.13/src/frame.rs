use std::{
    fmt::{self, Write},
    mem,
    ops::{Range, RangeInclusive},
};

use bytes::{Buf, BufMut, Bytes};
use tinyvec::TinyVec;

use crate::{
    Dir, MAX_CID_SIZE, RESET_TOKEN_SIZE, ResetToken, StreamId, TransportError, TransportErrorCode,
    VarInt,
    coding::{self, BufExt, BufMutExt, UnexpectedEnd},
    range_set::ArrayRangeSet,
    shared::{ConnectionId, EcnCodepoint},
};

#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;

/// A QUIC frame type
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct FrameType(u64);

impl FrameType {
    fn stream(self) -> Option<StreamInfo> {
        if STREAM_TYS.contains(&self.0) {
            Some(StreamInfo(self.0 as u8))
        } else {
            None
        }
    }
    fn datagram(self) -> Option<DatagramInfo> {
        if DATAGRAM_TYS.contains(&self.0) {
            Some(DatagramInfo(self.0 as u8))
        } else {
            None
        }
    }
}

impl coding::Codec for FrameType {
    fn decode<B: Buf>(buf: &mut B) -> coding::Result<Self> {
        Ok(Self(buf.get_var()?))
    }
    fn encode<B: BufMut>(&self, buf: &mut B) {
        buf.write_var(self.0);
    }
}

pub(crate) trait FrameStruct {
    /// Smallest number of bytes this type of frame is guaranteed to fit within.
    const SIZE_BOUND: usize;
}

macro_rules! frame_types {
    {$($name:ident = $val:expr,)*} => {
        impl FrameType {
            $(pub(crate) const $name: FrameType = FrameType($val);)*
        }

        impl fmt::Debug for FrameType {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self.0 {
                    $($val => f.write_str(stringify!($name)),)*
                    _ => write!(f, "Type({:02x})", self.0)
                }
            }
        }

        impl fmt::Display for FrameType {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self.0 {
                    $($val => f.write_str(stringify!($name)),)*
                    x if STREAM_TYS.contains(&x) => f.write_str("STREAM"),
                    x if DATAGRAM_TYS.contains(&x) => f.write_str("DATAGRAM"),
                    _ => write!(f, "<unknown {:02x}>", self.0),
                }
            }
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
struct StreamInfo(u8);

impl StreamInfo {
    fn fin(self) -> bool {
        self.0 & 0x01 != 0
    }
    fn len(self) -> bool {
        self.0 & 0x02 != 0
    }
    fn off(self) -> bool {
        self.0 & 0x04 != 0
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
struct DatagramInfo(u8);

impl DatagramInfo {
    fn len(self) -> bool {
        self.0 & 0x01 != 0
    }
}

frame_types! {
    PADDING = 0x00,
    PING = 0x01,
    ACK = 0x02,
    ACK_ECN = 0x03,
    RESET_STREAM = 0x04,
    STOP_SENDING = 0x05,
    CRYPTO = 0x06,
    NEW_TOKEN = 0x07,
    // STREAM
    MAX_DATA = 0x10,
    MAX_STREAM_DATA = 0x11,
    MAX_STREAMS_BIDI = 0x12,
    MAX_STREAMS_UNI = 0x13,
    DATA_BLOCKED = 0x14,
    STREAM_DATA_BLOCKED = 0x15,
    STREAMS_BLOCKED_BIDI = 0x16,
    STREAMS_BLOCKED_UNI = 0x17,
    NEW_CONNECTION_ID = 0x18,
    RETIRE_CONNECTION_ID = 0x19,
    PATH_CHALLENGE = 0x1a,
    PATH_RESPONSE = 0x1b,
    CONNECTION_CLOSE = 0x1c,
    APPLICATION_CLOSE = 0x1d,
    HANDSHAKE_DONE = 0x1e,
    // ACK Frequency
    ACK_FREQUENCY = 0xaf,
    IMMEDIATE_ACK = 0x1f,
    // DATAGRAM
}

const STREAM_TYS: RangeInclusive<u64> = RangeInclusive::new(0x08, 0x0f);
const DATAGRAM_TYS: RangeInclusive<u64> = RangeInclusive::new(0x30, 0x31);

#[derive(Debug)]
pub(crate) enum Frame {
    Padding,
    Ping,
    Ack(Ack),
    ResetStream(ResetStream),
    StopSending(StopSending),
    Crypto(Crypto),
    NewToken(NewToken),
    Stream(Stream),
    MaxData(VarInt),
    MaxStreamData { id: StreamId, offset: u64 },
    MaxStreams { dir: Dir, count: u64 },
    DataBlocked { offset: u64 },
    StreamDataBlocked { id: StreamId, offset: u64 },
    StreamsBlocked { dir: Dir, limit: u64 },
    NewConnectionId(NewConnectionId),
    RetireConnectionId { sequence: u64 },
    PathChallenge(u64),
    PathResponse(u64),
    Close(Close),
    Datagram(Datagram),
    AckFrequency(AckFrequency),
    ImmediateAck,
    HandshakeDone,
}

impl Frame {
    pub(crate) fn ty(&self) -> FrameType {
        use Frame::*;
        match *self {
            Padding => FrameType::PADDING,
            ResetStream(_) => FrameType::RESET_STREAM,
            Close(self::Close::Connection(_)) => FrameType::CONNECTION_CLOSE,
            Close(self::Close::Application(_)) => FrameType::APPLICATION_CLOSE,
            MaxData(_) => FrameType::MAX_DATA,
            MaxStreamData { .. } => FrameType::MAX_STREAM_DATA,
            MaxStreams { dir: Dir::Bi, .. } => FrameType::MAX_STREAMS_BIDI,
            MaxStreams { dir: Dir::Uni, .. } => FrameType::MAX_STREAMS_UNI,
            Ping => FrameType::PING,
            DataBlocked { .. } => FrameType::DATA_BLOCKED,
            StreamDataBlocked { .. } => FrameType::STREAM_DATA_BLOCKED,
            StreamsBlocked { dir: Dir::Bi, .. } => FrameType::STREAMS_BLOCKED_BIDI,
            StreamsBlocked { dir: Dir::Uni, .. } => FrameType::STREAMS_BLOCKED_UNI,
            StopSending { .. } => FrameType::STOP_SENDING,
            RetireConnectionId { .. } => FrameType::RETIRE_CONNECTION_ID,
            Ack(_) => FrameType::ACK,
            Stream(ref x) => {
                let mut ty = *STREAM_TYS.start();
                if x.fin {
                    ty |= 0x01;
                }
                if x.offset != 0 {
                    ty |= 0x04;
                }
                FrameType(ty)
            }
            PathChallenge(_) => FrameType::PATH_CHALLENGE,
            PathResponse(_) => FrameType::PATH_RESPONSE,
            NewConnectionId { .. } => FrameType::NEW_CONNECTION_ID,
            Crypto(_) => FrameType::CRYPTO,
            NewToken(_) => FrameType::NEW_TOKEN,
            Datagram(_) => FrameType(*DATAGRAM_TYS.start()),
            AckFrequency(_) => FrameType::ACK_FREQUENCY,
            ImmediateAck => FrameType::IMMEDIATE_ACK,
            HandshakeDone => FrameType::HANDSHAKE_DONE,
        }
    }

    pub(crate) fn is_ack_eliciting(&self) -> bool {
        !matches!(*self, Self::Ack(_) | Self::Padding | Self::Close(_))
    }
}

#[derive(Clone, Debug)]
pub enum Close {
    Connection(ConnectionClose),
    Application(ApplicationClose),
}

impl Close {
    pub(crate) fn encode<W: BufMut>(&self, out: &mut W, max_len: usize) {
        match *self {
            Self::Connection(ref x) => x.encode(out, max_len),
            Self::Application(ref x) => x.encode(out, max_len),
        }
    }

    pub(crate) fn is_transport_layer(&self) -> bool {
        matches!(*self, Self::Connection(_))
    }
}

impl From<TransportError> for Close {
    fn from(x: TransportError) -> Self {
        Self::Connection(x.into())
    }
}
impl From<ConnectionClose> for Close {
    fn from(x: ConnectionClose) -> Self {
        Self::Connection(x)
    }
}
impl From<ApplicationClose> for Close {
    fn from(x: ApplicationClose) -> Self {
        Self::Application(x)
    }
}

/// Reason given by the transport for closing the connection
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionClose {
    /// Class of error as encoded in the specification
    pub error_code: TransportErrorCode,
    /// Type of frame that caused the close
    pub frame_type: Option<FrameType>,
    /// Human-readable reason for the close
    pub reason: Bytes,
}

impl fmt::Display for ConnectionClose {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error_code.fmt(f)?;
        if !self.reason.as_ref().is_empty() {
            f.write_str(": ")?;
            f.write_str(&String::from_utf8_lossy(&self.reason))?;
        }
        Ok(())
    }
}

impl From<TransportError> for ConnectionClose {
    fn from(x: TransportError) -> Self {
        Self {
            error_code: x.code,
            frame_type: x.frame,
            reason: x.reason.into(),
        }
    }
}

impl FrameStruct for ConnectionClose {
    const SIZE_BOUND: usize = 1 + 8 + 8 + 8;
}

impl ConnectionClose {
    pub(crate) fn encode<W: BufMut>(&self, out: &mut W, max_len: usize) {
        out.write(FrameType::CONNECTION_CLOSE); // 1 byte
        out.write(self.error_code); // <= 8 bytes
        let ty = self.frame_type.map_or(0, |x| x.0);
        out.write_var(ty); // <= 8 bytes
        let max_len = max_len
            - 3
            - VarInt::from_u64(ty).unwrap().size()
            - VarInt::from_u64(self.reason.len() as u64).unwrap().size();
        let actual_len = self.reason.len().min(max_len);
        out.write_var(actual_len as u64); // <= 8 bytes
        out.put_slice(&self.reason[0..actual_len]); // whatever's left
    }
}

/// Reason given by an application for closing the connection
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationClose {
    /// Application-specific reason code
    pub error_code: VarInt,
    /// Human-readable reason for the close
    pub reason: Bytes,
}

impl fmt::Display for ApplicationClose {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.reason.as_ref().is_empty() {
            f.write_str(&String::from_utf8_lossy(&self.reason))?;
            f.write_str(" (code ")?;
            self.error_code.fmt(f)?;
            f.write_str(")")?;
        } else {
            self.error_code.fmt(f)?;
        }
        Ok(())
    }
}

impl FrameStruct for ApplicationClose {
    const SIZE_BOUND: usize = 1 + 8 + 8;
}

impl ApplicationClose {
    pub(crate) fn encode<W: BufMut>(&self, out: &mut W, max_len: usize) {
        out.write(FrameType::APPLICATION_CLOSE); // 1 byte
        out.write(self.error_code); // <= 8 bytes
        let max_len = max_len - 3 - VarInt::from_u64(self.reason.len() as u64).unwrap().size();
        let actual_len = self.reason.len().min(max_len);
        out.write_var(actual_len as u64); // <= 8 bytes
        out.put_slice(&self.reason[0..actual_len]); // whatever's left
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct Ack {
    pub largest: u64,
    pub delay: u64,
    pub additional: Bytes,
    pub ecn: Option<EcnCounts>,
}

impl fmt::Debug for Ack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut ranges = "[".to_string();
        let mut first = true;
        for range in self.iter() {
            if !first {
                ranges.push(',');
            }
            write!(ranges, "{range:?}").unwrap();
            first = false;
        }
        ranges.push(']');

        f.debug_struct("Ack")
            .field("largest", &self.largest)
            .field("delay", &self.delay)
            .field("ecn", &self.ecn)
            .field("ranges", &ranges)
            .finish()
    }
}

impl<'a> IntoIterator for &'a Ack {
    type Item = RangeInclusive<u64>;
    type IntoIter = AckIter<'a>;

    fn into_iter(self) -> AckIter<'a> {
        AckIter::new(self.largest, &self.additional[..])
    }
}

impl Ack {
    pub fn encode<W: BufMut>(
        delay: u64,
        ranges: &ArrayRangeSet,
        ecn: Option<&EcnCounts>,
        buf: &mut W,
    ) {
        let mut rest = ranges.iter().rev();
        let first = rest.next().unwrap();
        let largest = first.end - 1;
        let first_size = first.end - first.start;
        buf.write(if ecn.is_some() {
            FrameType::ACK_ECN
        } else {
            FrameType::ACK
        });
        buf.write_var(largest);
        buf.write_var(delay);
        buf.write_var(ranges.len() as u64 - 1);
        buf.write_var(first_size - 1);
        let mut prev = first.start;
        for block in rest {
            let size = block.end - block.start;
            buf.write_var(prev - block.end - 1);
            buf.write_var(size - 1);
            prev = block.start;
        }
        if let Some(x) = ecn {
            x.encode(buf)
        }
    }

    pub fn iter(&self) -> AckIter<'_> {
        self.into_iter()
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct EcnCounts {
    pub ect0: u64,
    pub ect1: u64,
    pub ce: u64,
}

impl std::ops::AddAssign<EcnCodepoint> for EcnCounts {
    fn add_assign(&mut self, rhs: EcnCodepoint) {
        match rhs {
            EcnCodepoint::Ect0 => {
                self.ect0 += 1;
            }
            EcnCodepoint::Ect1 => {
                self.ect1 += 1;
            }
            EcnCodepoint::Ce => {
                self.ce += 1;
            }
        }
    }
}

impl EcnCounts {
    pub const ZERO: Self = Self {
        ect0: 0,
        ect1: 0,
        ce: 0,
    };

    pub fn encode<W: BufMut>(&self, out: &mut W) {
        out.write_var(self.ect0);
        out.write_var(self.ect1);
        out.write_var(self.ce);
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Stream {
    pub(crate) id: StreamId,
    pub(crate) offset: u64,
    pub(crate) fin: bool,
    pub(crate) data: Bytes,
}

impl FrameStruct for Stream {
    const SIZE_BOUND: usize = 1 + 8 + 8 + 8;
}

/// Metadata from a stream frame
#[derive(Debug, Clone)]
pub(crate) struct StreamMeta {
    pub(crate) id: StreamId,
    pub(crate) offsets: Range<u64>,
    pub(crate) fin: bool,
}

// This manual implementation exists because `Default` is not implemented for `StreamId`
impl Default for StreamMeta {
    fn default() -> Self {
        Self {
            id: StreamId(0),
            offsets: 0..0,
            fin: false,
        }
    }
}

impl StreamMeta {
    pub(crate) fn encode<W: BufMut>(&self, length: bool, out: &mut W) {
        let mut ty = *STREAM_TYS.start();
        if self.offsets.start != 0 {
            ty |= 0x04;
        }
        if length {
            ty |= 0x02;
        }
        if self.fin {
            ty |= 0x01;
        }
        out.write_var(ty); // 1 byte
        out.write(self.id); // <=8 bytes
        if self.offsets.start != 0 {
            out.write_var(self.offsets.start); // <=8 bytes
        }
        if length {
            out.write_var(self.offsets.end - self.offsets.start); // <=8 bytes
        }
    }
}

/// A vector of [`StreamMeta`] with optimization for the single element case
pub(crate) type StreamMetaVec = TinyVec<[StreamMeta; 1]>;

#[derive(Debug, Clone)]
pub(crate) struct Crypto {
    pub(crate) offset: u64,
    pub(crate) data: Bytes,
}

impl Crypto {
    pub(crate) const SIZE_BOUND: usize = 17;

    pub(crate) fn encode<W: BufMut>(&self, out: &mut W) {
        out.write(FrameType::CRYPTO);
        out.write_var(self.offset);
        out.write_var(self.data.len() as u64);
        out.put_slice(&self.data);
    }
}

#[derive(Debug, Clone)]
pub(crate) struct NewToken {
    pub(crate) token: Bytes,
}

impl NewToken {
    pub(crate) fn encode<W: BufMut>(&self, out: &mut W) {
        out.write(FrameType::NEW_TOKEN);
        out.write_var(self.token.len() as u64);
        out.put_slice(&self.token);
    }

    pub(crate) fn size(&self) -> usize {
        1 + VarInt::from_u64(self.token.len() as u64).unwrap().size() + self.token.len()
    }
}

pub(crate) struct Iter {
    bytes: Bytes,
    last_ty: Option<FrameType>,
}

impl Iter {
    pub(crate) fn new(payload: Bytes) -> Result<Self, TransportError> {
        if payload.is_empty() {
            // "An endpoint MUST treat receipt of a packet containing no frames as a
            // connection error of type PROTOCOL_VIOLATION."
            // https://www.rfc-editor.org/rfc/rfc9000.html#name-frames-and-frame-types
            return Err(TransportError::PROTOCOL_VIOLATION(
                "packet payload is empty",
            ));
        }

        Ok(Self {
            bytes: payload,
            last_ty: None,
        })
    }

    fn take_len(&mut self) -> Result<Bytes, UnexpectedEnd> {
        let len = self.bytes.get_var()?;
        if len > self.bytes.remaining() as u64 {
            return Err(UnexpectedEnd);
        }
        Ok(self.bytes.split_to(len as usize))
    }

    fn try_next(&mut self) -> Result<Frame, IterErr> {
        let ty = self.bytes.get::<FrameType>()?;
        self.last_ty = Some(ty);
        Ok(match ty {
            FrameType::PADDING => Frame::Padding,
            FrameType::RESET_STREAM => Frame::ResetStream(ResetStream {
                id: self.bytes.get()?,
                error_code: self.bytes.get()?,
                final_offset: self.bytes.get()?,
            }),
            FrameType::CONNECTION_CLOSE => Frame::Close(Close::Connection(ConnectionClose {
                error_code: self.bytes.get()?,
                frame_type: {
                    let x = self.bytes.get_var()?;
                    if x == 0 { None } else { Some(FrameType(x)) }
                },
                reason: self.take_len()?,
            })),
            FrameType::APPLICATION_CLOSE => Frame::Close(Close::Application(ApplicationClose {
                error_code: self.bytes.get()?,
                reason: self.take_len()?,
            })),
            FrameType::MAX_DATA => Frame::MaxData(self.bytes.get()?),
            FrameType::MAX_STREAM_DATA => Frame::MaxStreamData {
                id: self.bytes.get()?,
                offset: self.bytes.get_var()?,
            },
            FrameType::MAX_STREAMS_BIDI => Frame::MaxStreams {
                dir: Dir::Bi,
                count: self.bytes.get_var()?,
            },
            FrameType::MAX_STREAMS_UNI => Frame::MaxStreams {
                dir: Dir::Uni,
                count: self.bytes.get_var()?,
            },
            FrameType::PING => Frame::Ping,
            FrameType::DATA_BLOCKED => Frame::DataBlocked {
                offset: self.bytes.get_var()?,
            },
            FrameType::STREAM_DATA_BLOCKED => Frame::StreamDataBlocked {
                id: self.bytes.get()?,
                offset: self.bytes.get_var()?,
            },
            FrameType::STREAMS_BLOCKED_BIDI => Frame::StreamsBlocked {
                dir: Dir::Bi,
                limit: self.bytes.get_var()?,
            },
            FrameType::STREAMS_BLOCKED_UNI => Frame::StreamsBlocked {
                dir: Dir::Uni,
                limit: self.bytes.get_var()?,
            },
            FrameType::STOP_SENDING => Frame::StopSending(StopSending {
                id: self.bytes.get()?,
                error_code: self.bytes.get()?,
            }),
            FrameType::RETIRE_CONNECTION_ID => Frame::RetireConnectionId {
                sequence: self.bytes.get_var()?,
            },
            FrameType::ACK | FrameType::ACK_ECN => {
                let largest = self.bytes.get_var()?;
                let delay = self.bytes.get_var()?;
                let extra_blocks = self.bytes.get_var()? as usize;
                let n = scan_ack_blocks(&self.bytes, largest, extra_blocks)?;
                Frame::Ack(Ack {
                    delay,
                    largest,
                    additional: self.bytes.split_to(n),
                    ecn: if ty != FrameType::ACK_ECN {
                        None
                    } else {
                        Some(EcnCounts {
                            ect0: self.bytes.get_var()?,
                            ect1: self.bytes.get_var()?,
                            ce: self.bytes.get_var()?,
                        })
                    },
                })
            }
            FrameType::PATH_CHALLENGE => Frame::PathChallenge(self.bytes.get()?),
            FrameType::PATH_RESPONSE => Frame::PathResponse(self.bytes.get()?),
            FrameType::NEW_CONNECTION_ID => {
                let sequence = self.bytes.get_var()?;
                let retire_prior_to = self.bytes.get_var()?;
                if retire_prior_to > sequence {
                    return Err(IterErr::Malformed);
                }
                let length = self.bytes.get::<u8>()? as usize;
                if length > MAX_CID_SIZE || length == 0 {
                    return Err(IterErr::Malformed);
                }
                if length > self.bytes.remaining() {
                    return Err(IterErr::UnexpectedEnd);
                }
                let mut stage = [0; MAX_CID_SIZE];
                self.bytes.copy_to_slice(&mut stage[0..length]);
                let id = ConnectionId::new(&stage[..length]);
                if self.bytes.remaining() < 16 {
                    return Err(IterErr::UnexpectedEnd);
                }
                let mut reset_token = [0; RESET_TOKEN_SIZE];
                self.bytes.copy_to_slice(&mut reset_token);
                Frame::NewConnectionId(NewConnectionId {
                    sequence,
                    retire_prior_to,
                    id,
                    reset_token: reset_token.into(),
                })
            }
            FrameType::CRYPTO => Frame::Crypto(Crypto {
                offset: self.bytes.get_var()?,
                data: self.take_len()?,
            }),
            FrameType::NEW_TOKEN => Frame::NewToken(NewToken {
                token: self.take_len()?,
            }),
            FrameType::HANDSHAKE_DONE => Frame::HandshakeDone,
            FrameType::ACK_FREQUENCY => Frame::AckFrequency(AckFrequency {
                sequence: self.bytes.get()?,
                ack_eliciting_threshold: self.bytes.get()?,
                request_max_ack_delay: self.bytes.get()?,
                reordering_threshold: self.bytes.get()?,
            }),
            FrameType::IMMEDIATE_ACK => Frame::ImmediateAck,
            _ => {
                if let Some(s) = ty.stream() {
                    Frame::Stream(Stream {
                        id: self.bytes.get()?,
                        offset: if s.off() { self.bytes.get_var()? } else { 0 },
                        fin: s.fin(),
                        data: if s.len() {
                            self.take_len()?
                        } else {
                            self.take_remaining()
                        },
                    })
                } else if let Some(d) = ty.datagram() {
                    Frame::Datagram(Datagram {
                        data: if d.len() {
                            self.take_len()?
                        } else {
                            self.take_remaining()
                        },
                    })
                } else {
                    return Err(IterErr::InvalidFrameId);
                }
            }
        })
    }

    fn take_remaining(&mut self) -> Bytes {
        mem::take(&mut self.bytes)
    }
}

impl Iterator for Iter {
    type Item = Result<Frame, InvalidFrame>;
    fn next(&mut self) -> Option<Self::Item> {
        if !self.bytes.has_remaining() {
            return None;
        }
        match self.try_next() {
            Ok(x) => Some(Ok(x)),
            Err(e) => {
                // Corrupt frame, skip it and everything that follows
                self.bytes.clear();
                Some(Err(InvalidFrame {
                    ty: self.last_ty,
                    reason: e.reason(),
                }))
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct InvalidFrame {
    pub(crate) ty: Option<FrameType>,
    pub(crate) reason: &'static str,
}

impl From<InvalidFrame> for TransportError {
    fn from(err: InvalidFrame) -> Self {
        let mut te = Self::FRAME_ENCODING_ERROR(err.reason);
        te.frame = err.ty;
        te
    }
}

/// Validate exactly `n` ACK ranges in `buf` and return the number of bytes they cover
fn scan_ack_blocks(mut buf: &[u8], largest: u64, n: usize) -> Result<usize, IterErr> {
    let total_len = buf.remaining();
    let first_block = buf.get_var()?;
    let mut smallest = largest.checked_sub(first_block).ok_or(IterErr::Malformed)?;
    for _ in 0..n {
        let gap = buf.get_var()?;
        smallest = smallest.checked_sub(gap + 2).ok_or(IterErr::Malformed)?;
        let block = buf.get_var()?;
        smallest = smallest.checked_sub(block).ok_or(IterErr::Malformed)?;
    }
    Ok(total_len - buf.remaining())
}

enum IterErr {
    UnexpectedEnd,
    InvalidFrameId,
    Malformed,
}

impl IterErr {
    fn reason(&self) -> &'static str {
        use IterErr::*;
        match *self {
            UnexpectedEnd => "unexpected end",
            InvalidFrameId => "invalid frame ID",
            Malformed => "malformed",
        }
    }
}

impl From<UnexpectedEnd> for IterErr {
    fn from(_: UnexpectedEnd) -> Self {
        Self::UnexpectedEnd
    }
}

#[derive(Debug, Clone)]
pub struct AckIter<'a> {
    largest: u64,
    data: &'a [u8],
}

impl<'a> AckIter<'a> {
    fn new(largest: u64, data: &'a [u8]) -> Self {
        Self { largest, data }
    }
}

impl Iterator for AckIter<'_> {
    type Item = RangeInclusive<u64>;
    fn next(&mut self) -> Option<RangeInclusive<u64>> {
        if !self.data.has_remaining() {
            return None;
        }
        let block = self.data.get_var().unwrap();
        let largest = self.largest;
        if let Ok(gap) = self.data.get_var() {
            self.largest -= block + gap + 2;
        }
        Some(largest - block..=largest)
    }
}

#[allow(unreachable_pub)] // fuzzing only
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
#[derive(Debug, Copy, Clone)]
pub struct ResetStream {
    pub(crate) id: StreamId,
    pub(crate) error_code: VarInt,
    pub(crate) final_offset: VarInt,
}

impl FrameStruct for ResetStream {
    const SIZE_BOUND: usize = 1 + 8 + 8 + 8;
}

impl ResetStream {
    pub(crate) fn encode<W: BufMut>(&self, out: &mut W) {
        out.write(FrameType::RESET_STREAM); // 1 byte
        out.write(self.id); // <= 8 bytes
        out.write(self.error_code); // <= 8 bytes
        out.write(self.final_offset); // <= 8 bytes
    }
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct StopSending {
    pub(crate) id: StreamId,
    pub(crate) error_code: VarInt,
}

impl FrameStruct for StopSending {
    const SIZE_BOUND: usize = 1 + 8 + 8;
}

impl StopSending {
    pub(crate) fn encode<W: BufMut>(&self, out: &mut W) {
        out.write(FrameType::STOP_SENDING); // 1 byte
        out.write(self.id); // <= 8 bytes
        out.write(self.error_code) // <= 8 bytes
    }
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct NewConnectionId {
    pub(crate) sequence: u64,
    pub(crate) retire_prior_to: u64,
    pub(crate) id: ConnectionId,
    pub(crate) reset_token: ResetToken,
}

impl NewConnectionId {
    pub(crate) fn encode<W: BufMut>(&self, out: &mut W) {
        out.write(FrameType::NEW_CONNECTION_ID);
        out.write_var(self.sequence);
        out.write_var(self.retire_prior_to);
        out.write(self.id.len() as u8);
        out.put_slice(&self.id);
        out.put_slice(&self.reset_token);
    }
}

impl FrameStruct for NewConnectionId {
    const SIZE_BOUND: usize = 1 + 8 + 8 + 1 + MAX_CID_SIZE + RESET_TOKEN_SIZE;
}

/// Smallest number of bytes this type of frame is guaranteed to fit within.
pub(crate) const RETIRE_CONNECTION_ID_SIZE_BOUND: usize = 9;

/// An unreliable datagram
#[derive(Debug, Clone)]
pub struct Datagram {
    /// Payload
    pub data: Bytes,
}

impl FrameStruct for Datagram {
    const SIZE_BOUND: usize = 1 + 8;
}

impl Datagram {
    pub(crate) fn encode(&self, length: bool, out: &mut Vec<u8>) {
        out.write(FrameType(*DATAGRAM_TYS.start() | u64::from(length))); // 1 byte
        if length {
            // Safe to unwrap because we check length sanity before queueing datagrams
            out.write(VarInt::from_u64(self.data.len() as u64).unwrap()); // <= 8 bytes
        }
        out.extend_from_slice(&self.data);
    }

    pub(crate) fn size(&self, length: bool) -> usize {
        1 + if length {
            VarInt::from_u64(self.data.len() as u64).unwrap().size()
        } else {
            0
        } + self.data.len()
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) struct AckFrequency {
    pub(crate) sequence: VarInt,
    pub(crate) ack_eliciting_threshold: VarInt,
    pub(crate) request_max_ack_delay: VarInt,
    pub(crate) reordering_threshold: VarInt,
}

impl AckFrequency {
    pub(crate) fn encode<W: BufMut>(&self, buf: &mut W) {
        buf.write(FrameType::ACK_FREQUENCY);
        buf.write(self.sequence);
        buf.write(self.ack_eliciting_threshold);
        buf.write(self.request_max_ack_delay);
        buf.write(self.reordering_threshold);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::coding::Codec;
    use assert_matches::assert_matches;

    fn frames(buf: Vec<u8>) -> Vec<Frame> {
        Iter::new(Bytes::from(buf))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    }

    #[test]
    fn ack_coding() {
        const PACKETS: &[u64] = &[1, 2, 3, 5, 10, 11, 14];
        let mut ranges = ArrayRangeSet::new();
        for &packet in PACKETS {
            ranges.insert(packet..packet + 1);
        }
        let mut buf = Vec::new();
        const ECN: EcnCounts = EcnCounts {
            ect0: 42,
            ect1: 24,
            ce: 12,
        };
        Ack::encode(42, &ranges, Some(&ECN), &mut buf);
        let frames = frames(buf);
        assert_eq!(frames.len(), 1);
        match frames[0] {
            Frame::Ack(ref ack) => {
                let mut packets = ack.iter().flatten().collect::<Vec<_>>();
                packets.sort_unstable();
                assert_eq!(&packets[..], PACKETS);
                assert_eq!(ack.ecn, Some(ECN));
            }
            ref x => panic!("incorrect frame {x:?}"),
        }
    }

    #[test]
    fn ack_frequency_coding() {
        let mut buf = Vec::new();
        let original = AckFrequency {
            sequence: VarInt(42),
            ack_eliciting_threshold: VarInt(20),
            request_max_ack_delay: VarInt(50_000),
            reordering_threshold: VarInt(1),
        };
        original.encode(&mut buf);
        let frames = frames(buf);
        assert_eq!(frames.len(), 1);
        match &frames[0] {
            Frame::AckFrequency(decoded) => assert_eq!(decoded, &original),
            x => panic!("incorrect frame {x:?}"),
        }
    }

    #[test]
    fn immediate_ack_coding() {
        let mut buf = Vec::new();
        FrameType::IMMEDIATE_ACK.encode(&mut buf);
        let frames = frames(buf);
        assert_eq!(frames.len(), 1);
        assert_matches!(&frames[0], Frame::ImmediateAck);
    }
}
