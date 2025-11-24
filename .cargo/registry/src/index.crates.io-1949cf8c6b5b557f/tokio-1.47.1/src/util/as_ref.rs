use super::typeid;

#[derive(Debug)]
pub(crate) enum OwnedBuf {
    Vec(Vec<u8>),
    #[cfg(feature = "io-util")]
    Bytes(bytes::Bytes),
}

impl AsRef<[u8]> for OwnedBuf {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Vec(vec) => vec,
            #[cfg(feature = "io-util")]
            Self::Bytes(bytes) => bytes,
        }
    }
}

pub(crate) fn upgrade<B: AsRef<[u8]>>(buf: B) -> OwnedBuf {
    let buf = match unsafe { typeid::try_transmute::<B, Vec<u8>>(buf) } {
        Ok(vec) => return OwnedBuf::Vec(vec),
        Err(original_buf) => original_buf,
    };

    let buf = match unsafe { typeid::try_transmute::<B, String>(buf) } {
        Ok(string) => return OwnedBuf::Vec(string.into_bytes()),
        Err(original_buf) => original_buf,
    };

    #[cfg(feature = "io-util")]
    let buf = match unsafe { typeid::try_transmute::<B, bytes::Bytes>(buf) } {
        Ok(bytes) => return OwnedBuf::Bytes(bytes),
        Err(original_buf) => original_buf,
    };

    OwnedBuf::Vec(buf.as_ref().to_owned())
}
