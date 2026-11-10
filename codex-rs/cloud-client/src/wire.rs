//! Minimal ThreadService protobuf envelopes; field numbers are the wire contract.
//! Length-delimited nested events remain intact, including unknown protobuf fields.

use bytes::Buf;
use bytes::Bytes;
use prost::Message;
use std::marker::PhantomData;
use tonic::codec::Codec;

#[derive(Clone, prost::Message)]
pub(crate) struct EmptyResponse {}

#[derive(Clone, prost::Message)]
pub(crate) struct ThreadIdRequest {
    #[prost(string, tag = "1")]
    pub thread_id: String,
}

#[derive(Clone, prost::Message)]
pub(crate) struct UnarchiveResponse {
    #[prost(bytes = "bytes", optional, tag = "1")]
    pub thread: Option<Bytes>,
}

#[derive(Clone, prost::Message)]
pub(crate) struct AttachResponse {
    #[prost(oneof = "Payload", tags = "1, 2")]
    pub event: Option<Payload>,
}

#[derive(Clone, prost::Oneof)]
pub(crate) enum Payload {
    #[prost(bytes = "bytes", tag = "1")]
    Notification(bytes::Bytes),
    #[prost(bytes = "bytes", tag = "2")]
    ServerRequest(bytes::Bytes),
}

// Tonic enforces message bounds and status trailers while callers decode the raw payload.
pub(super) struct RawResponseCodec<T>(pub PhantomData<T>);
impl<T: Message + Send + 'static> Codec for RawResponseCodec<T> {
    type Encode = T;
    type Decode = Bytes;
    type Encoder = tonic_prost::ProstEncoder<T>;
    type Decoder = RawDecoder;
    fn encoder(&mut self) -> Self::Encoder {
        tonic_prost::ProstCodec::<T, EmptyResponse>::default().encoder()
    }
    fn decoder(&mut self) -> Self::Decoder {
        RawDecoder
    }
}
pub(super) struct RawDecoder;
impl tonic::codec::Decoder for RawDecoder {
    type Item = Bytes;
    type Error = tonic::Status;
    fn decode(
        &mut self,
        buffer: &mut tonic::codec::DecodeBuf<'_>,
    ) -> Result<Option<Bytes>, tonic::Status> {
        Ok(Some(buffer.copy_to_bytes(buffer.remaining())))
    }
}
