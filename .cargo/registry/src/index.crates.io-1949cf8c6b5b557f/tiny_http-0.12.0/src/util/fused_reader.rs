use std::io::{IoSliceMut, Read, Result as IoResult};

/// Wraps another reader and provides "fused" behavior.
/// When the underlying reader reaches EOF, it is dropped
/// and the fused reader becomes an empty stub.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FusedReader<R: Read> {
    inner: Option<R>,
}

impl<R: Read> FusedReader<R> {
    pub fn new(inner: R) -> Self {
        Self { inner: Some(inner) }
    }

    #[allow(dead_code)]
    pub fn into_inner(self) -> Option<R> {
        self.inner
    }
}

impl<R: Read> Read for FusedReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        match &mut self.inner {
            Some(r) => {
                let l = r.read(buf)?;
                if l == 0 {
                    self.inner = None;
                }
                Ok(l)
            }
            None => Ok(0),
        }
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> IoResult<usize> {
        match &mut self.inner {
            Some(r) => {
                let l = r.read_vectored(bufs)?;
                if l == 0 {
                    self.inner = None;
                }
                Ok(l)
            }
            None => Ok(0),
        }
    }
}
