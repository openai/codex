use std::io::{Read, Seek};

use crate::{tags::Tag, Directory};
use crate::{TiffError, TiffFormatError, TiffResult};

use super::ifd::{Entry, Value};
use super::ValueReader;

pub(crate) struct TagReader<'lt, R: EntryDecoder + ?Sized> {
    pub(crate) decoder: &'lt mut R,
    pub(crate) ifd: &'lt Directory,
}

pub(crate) trait EntryDecoder {
    /// Turn an entry into a value by fetching the required bytes in the stream.
    fn entry_val(&mut self, entry: &Entry) -> TiffResult<Value>;
}

impl<R: Seek + Read> EntryDecoder for ValueReader<R> {
    fn entry_val(&mut self, entry: &Entry) -> TiffResult<Value> {
        entry.val(&self.limits, self.bigtiff, &mut self.reader)
    }
}

impl<'a, R: EntryDecoder + ?Sized> TagReader<'a, R> {
    pub(crate) fn find_tag(&mut self, tag: Tag) -> TiffResult<Option<Value>> {
        let entry = match self.ifd.get(tag) {
            None => return Ok(None),
            Some(entry) => entry.clone(),
        };

        self.decoder.entry_val(&entry).map(Some)
    }

    pub(crate) fn require_tag(&mut self, tag: Tag) -> TiffResult<Value> {
        match self.find_tag(tag)? {
            Some(val) => Ok(val),
            None => Err(TiffError::FormatError(
                TiffFormatError::RequiredTagNotFound(tag),
            )),
        }
    }

    pub fn find_tag_uint_vec<T: TryFrom<u64>>(&mut self, tag: Tag) -> TiffResult<Option<Vec<T>>> {
        self.find_tag(tag)?
            .map(|v| v.into_u64_vec())
            .transpose()?
            .map(|v| {
                v.into_iter()
                    .map(|u| {
                        T::try_from(u).map_err(|_| TiffFormatError::InvalidTagValueType(tag).into())
                    })
                    .collect()
            })
            .transpose()
    }
}
