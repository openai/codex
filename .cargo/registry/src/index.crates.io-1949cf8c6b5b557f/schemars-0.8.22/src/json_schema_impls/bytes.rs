use crate::r#gen::SchemaGenerator;
use crate::schema::*;
use crate::JsonSchema;
use bytes::{Bytes, BytesMut};

forward_impl!((JsonSchema for Bytes) => Vec<u8>);
forward_impl!((JsonSchema for BytesMut) => Vec<u8>);
