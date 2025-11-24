use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use crate::{
    error::{err, Error, ErrorContext},
    tz::TimeZone,
    util::{array_str::ArrayStr, escape, utf8},
};

/// An abstraction for reading data from Android's concatenated TZif data file.
///
/// This abstraction is designed in a way that the data is reads from is
/// largely untrusted. This means that, no matter what sequence of bytes is
/// given, this should never panic (or else there is a bug). Moreover, there is
/// some guarding against disproportionate allocation. While big allocations
/// can still happen, they require a proportionally large data file. (Thus,
/// callers can guard against this by considering the size of the data.) What
/// this implementation prevents against is accidentally OOMing or panicking as
/// a result of naively doing `Vec::with_capacity(rdr.decode_integer())`.
///
/// This is also designed to work in alloc-only contexts mostly out of "good
/// sense." Technically we don't (currently) use this outside of `std`, since
/// it's only used for reading tzdb on Android from the file system. But we do
/// things this way in case we end up wanting to use it for something else.
/// If we needed this for no-alloc environments, then that's a much bigger
/// change, if only because it would require making the TZif parser no-alloc
/// compatible, and it's not quite clear what the best way to do that is. We
/// achieve the alloc-only API be introducing a trait that abstracts over a
/// `File` for random access to bytes.
#[derive(Debug)]
pub(crate) struct ConcatenatedTzif<R> {
    rdr: R,
    header: Header,
}

impl<R: Read> ConcatenatedTzif<R> {
    /// Open the concatenated TZif file using the reader given.
    ///
    /// This reads the header and will return an error if the header is
    /// invalid.
    pub(crate) fn open(rdr: R) -> Result<ConcatenatedTzif<R>, Error> {
        let header = Header::read(&rdr)?;
        Ok(ConcatenatedTzif { rdr, header })
    }

    /// Returns the version of this `tzdata` database.
    pub(crate) fn version(&self) -> ArrayStr<5> {
        self.header.version
    }

    /// Returns a `TimeZone` extracted from this concatenated TZif data.
    ///
    /// This is only successful if an index entry with the corresponding
    /// IANA time zone identifier could be found.
    ///
    /// Callers must provide two scratch buffers that are used for temporary
    /// allocation internally. Callers can create a new buffer for each call,
    /// but it's likely faster to reuse them if possible.
    ///
    /// If a `TimeZone` is returned, it is guaranteed to have a present IANA
    /// name (accessible via `TimeZone::iana_name`).
    pub(crate) fn get(
        &self,
        query: &str,
        scratch1: &mut Vec<u8>,
        scratch2: &mut Vec<u8>,
    ) -> Result<Option<TimeZone>, Error> {
        scratch1.clear();
        alloc(scratch1, self.header.index_len())?;
        self.rdr
            .read_exact_at(scratch1, self.header.index_offset)
            .context("failed to read index block")?;

        let mut index = &**scratch1;
        while !index.is_empty() {
            let entry = IndexEntry::new(&index[..IndexEntry::LEN]);
            index = &index[IndexEntry::LEN..];
            let ordering = utf8::cmp_ignore_ascii_case_bytes(
                entry.name_bytes(),
                query.as_bytes(),
            );
            if ordering.is_ne() {
                continue;
            }

            // OK because `entry.name_bytes()` is equal to `query`,
            // ignoring ASCII case. The only way this can be true is is
            // `entry.name_bytes()` is itself valid UTF-8.
            let name = entry.name().unwrap();
            scratch2.clear();
            alloc(scratch2, entry.len())?;
            let start = self.header.data_offset.saturating_add(entry.start());
            self.rdr
                .read_exact_at(scratch2, start)
                .context("failed to read TZif data block")?;
            return TimeZone::tzif(name, scratch2).map(Some);
        }
        Ok(None)
    }

    /// Returns a list of all IANA time zone identifiers in this concatenated
    /// TZif data.
    ///
    /// Callers must provide a scratch buffer that is used for temporary
    /// allocation internally. Callers can create a new buffer for each call,
    /// but it's likely faster to reuse them if possible.
    pub(crate) fn available(
        &self,
        scratch: &mut Vec<u8>,
    ) -> Result<Vec<String>, Error> {
        scratch.clear();
        alloc(scratch, self.header.index_len())?;
        self.rdr
            .read_exact_at(scratch, self.header.index_offset)
            .context("failed to read index block")?;

        let names_len = self.header.index_len() / IndexEntry::LEN;
        // Why are we careless with this alloc? Well, its size is proportional
        // to the actual amount of data in the file. So the only way to get a
        // big alloc is to create a huge file. This seems... fine... I guess.
        // Where as the `alloc` above is done on the basis of an arbitrary
        // 32-bit integer.
        let mut names = Vec::with_capacity(names_len);
        let mut index = &**scratch;
        while !index.is_empty() {
            let entry = IndexEntry::new(&index[..IndexEntry::LEN]);
            index = &index[IndexEntry::LEN..];
            names.push(entry.name()?.to_string());
        }
        Ok(names)
    }
}

/// The header of Android concatenated TZif data.
///
/// The header has the version and some offsets indicating the location of
/// the index entry (a list of IANA time zone identifiers and offsets into
/// the data block) and the actual TZif data.
#[derive(Debug)]
struct Header {
    version: ArrayStr<5>,
    index_offset: u64,
    data_offset: u64,
}

impl Header {
    /// Reads the header from Android's concatenated TZif concatenated data
    /// file.
    ///
    /// Basically, this gives us the version and some offsets for where to find
    /// data.
    fn read<R: Read + ?Sized>(rdr: &R) -> Result<Header, Error> {
        // 12 bytes plus 3 4-byte big endian integers.
        let mut buf = [0; 12 + 3 * 4];
        rdr.read_exact_at(&mut buf, 0)
            .context("failed to read concatenated TZif header")?;
        if &buf[..6] != b"tzdata" {
            return Err(err!(
                "expected first 6 bytes of concatenated TZif header \
                 to be `tzdata`, but found `{found}`",
                found = escape::Bytes(&buf[..6]),
            ));
        }
        if buf[11] != 0 {
            return Err(err!(
                "expected last byte of concatenated TZif header \
                 to be NUL, but found `{found}`",
                found = escape::Bytes(&buf[..12]),
            ));
        }

        let version = {
            let version = core::str::from_utf8(&buf[6..11]).map_err(|_| {
                err!(
                    "expected version in concatenated TZif header to \
                     be valid UTF-8, but found `{found}`",
                    found = escape::Bytes(&buf[6..11]),
                )
            })?;
            // OK because `version` is exactly 5 bytes, by construction.
            ArrayStr::new(version).unwrap()
        };
        // OK because the sub-slice is sized to exactly 4 bytes.
        let index_offset = u64::from(read_be32(&buf[12..16]));
        // OK because the sub-slice is sized to exactly 4 bytes.
        let data_offset = u64::from(read_be32(&buf[16..20]));
        if index_offset > data_offset {
            return Err(err!(
                "invalid index ({index_offset}) and data ({data_offset}) \
                 offsets, expected index offset to be less than or equal \
                 to data offset",
            ));
        }
        // we don't read 20..24 since we don't care about zonetab (yet)
        let header = Header { version, index_offset, data_offset };
        if header.index_len() % IndexEntry::LEN != 0 {
            return Err(err!(
                "length of index block is not a multiple {len}",
                len = IndexEntry::LEN,
            ));
        }
        Ok(header)
    }

    /// Returns the length of the index section of the concatenated tzdb.
    ///
    /// Beware of using this to create allocations. In theory, this should be
    /// trusted data, but the length can be any 32-bit integer. If it's used to
    /// create an allocation, it could potentially be up to 4GB.
    fn index_len(&self) -> usize {
        // OK because `Header` parsing returns an error if this overflows.
        let len = self.data_offset.checked_sub(self.index_offset).unwrap();
        // N.B. Overflow only occurs here on 16-bit (or smaller) platforms,
        // which at the time of writing, is not supported by Jiff. Instead,
        // a `usize::MAX` will trigger an allocation error.
        usize::try_from(len).unwrap_or(usize::MAX)
    }
}

/// A view into a single index entry in the index block of concatenated TZif
/// data.
///
/// If we had safe transmute, it would be much nicer to define this as
///
/// ```text
/// #[derive(Clone, Copy)]
/// #[repr(transparent, align(1))]
/// struct IndexEntry {
///     name: [u8; 40],
///     start: u32,
///     len: u32,
///     _raw_utc_offset: u32, // we don't use this here
/// }
/// ```
///
/// And probably implement a trait asserting that this is plain old data (or
/// derive it safely). And then we could cast `&[u8]` to `&[IndexEntry]`
/// safely and access the individual fields as is. We could do this today,
/// but not in safe code. And since this isn't performance critical, it's just
/// not worth flagging this code as potentially containing undefined behavior.
#[derive(Clone, Copy)]
struct IndexEntry<'a>(&'a [u8]);

impl<'a> IndexEntry<'a> {
    /// The length of an index entry. It's fixed size. 40 bytes for the IANA
    /// time zone identifier. 4 bytes for each of 3 big-endian integers. The
    /// first is the start of the corresponding TZif data within the data
    /// block. The second is the length of said TZif data. And the third is
    /// the "raw UTC offset" of the time zone. (I'm unclear on the semantics
    /// of this third, since some time zones have more than one because of
    /// DST. And of course, it can change over time. Since I don't know what
    /// Android uses this for, I'm not sure how I'm supposed to interpret it.)
    const LEN: usize = 40 + 3 * 4;

    /// Creates a new view into an entry in the concatenated TZif index.
    ///
    /// # Panics
    ///
    /// When `slice` does not have the expected length (`IndexEntry::LEN`).
    fn new(slice: &'a [u8]) -> IndexEntry<'a> {
        assert_eq!(slice.len(), IndexEntry::LEN, "invalid index entry length");
        IndexEntry(slice)
    }

    /// Like `name_bytes`, but as a `&str`.
    ///
    /// This returns an error if the name isn't valid UTF-8.
    fn name(&self) -> Result<&str, Error> {
        core::str::from_utf8(self.name_bytes()).map_err(|_| {
            err!(
                "IANA time zone identifier `{name}` is not valid UTF-8",
                name = escape::Bytes(self.name_bytes()),
            )
        })
    }

    /// Returns the IANA time zone identifier as a byte slice.
    ///
    /// In theory, an empty slice could be returned. But if that happens,
    /// then there is probably a bug in this code somewhere, the format
    /// changed or the source data is corrupt somehow.
    fn name_bytes(&self) -> &'a [u8] {
        let mut block = &self.0[..40];
        while block.last().copied() == Some(0) {
            block = &block[..block.len() - 1];
        }
        block
    }

    /// Returns the starting offset (relative to the beginning of the TZif
    /// data block) of the corresponding TZif data.
    fn start(&self) -> u64 {
        u64::from(read_be32(&self.0[40..44]))
    }

    /// Returns the length of the TZif data block.
    ///
    /// Beware of using this to create allocations. In theory, this should be
    /// trusted data, but the length can be any 32-bit integer. If it's used to
    /// create an allocation, it could potentially be up to 4GB.
    fn len(&self) -> usize {
        // N.B. Overflow only occurs here on 16-bit (or smaller) platforms,
        // which at the time of writing, is not supported by Jiff. Instead,
        // a `usize::MAX` will trigger an allocation error.
        usize::try_from(read_be32(&self.0[44..48])).unwrap_or(usize::MAX)
    }
}

impl<'a> core::fmt::Debug for IndexEntry<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("IndexEntry")
            .field("name", &escape::Bytes(self.name_bytes()))
            .field("start", &self.start())
            .field("len", &self.len())
            .finish()
    }
}

/// A crate-internal trait defining the source of concatenated TZif data.
///
/// Basically, this just provides a way to read a fixed amount of data at a
/// particular offset. This is obviously trivial to implement on `&[u8]` (and
/// indeed, we do so for testing), but we use it to abstract over platform
/// differences when reading from a `File`.
///
/// The intent is that on Unix, this will use `pread`, which avoids a file
/// seek followed by a `read` call.
pub(crate) trait Read {
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> Result<(), Error>;
}

impl<'a, R: Read + ?Sized> Read for &'a R {
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> Result<(), Error> {
        (**self).read_exact_at(buf, offset)
    }
}

/// Reads a 32-bit big endian encoded integer from `bytes`.
///
/// # Panics
///
/// If `bytes.len() != 4`.
fn read_be32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes(bytes.try_into().expect("slice of length 4"))
}

#[cfg(test)]
impl Read for [u8] {
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> Result<(), Error> {
        let offset = usize::try_from(offset)
            .map_err(|_| err!("offset `{offset}` overflowed `usize`"))?;
        let Some(slice) = self.get(offset..) else {
            return Err(err!(
                "given offset `{offset}` is not valid \
                 (only {len} bytes are available)",
                len = self.len(),
            ));
        };
        if buf.len() > slice.len() {
            return Err(err!(
                "unexpected EOF, expected {len} bytes but only have {have}",
                len = buf.len(),
                have = slice.len()
            ));
        }
        buf.copy_from_slice(&slice[..buf.len()]);
        Ok(())
    }
}

#[cfg(all(feature = "std", unix))]
impl Read for std::fs::File {
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> Result<(), Error> {
        use std::os::unix::fs::FileExt;
        FileExt::read_exact_at(self, buf, offset).map_err(Error::io)
    }
}

#[cfg(all(feature = "std", windows))]
impl Read for std::fs::File {
    fn read_exact_at(
        &self,
        mut buf: &mut [u8],
        mut offset: u64,
    ) -> Result<(), Error> {
        use std::{io, os::windows::fs::FileExt};

        while !buf.is_empty() {
            match self.seek_read(buf, offset) {
                Ok(0) => break,
                Ok(n) => {
                    buf = &mut buf[n..];
                    offset = u64::try_from(n)
                        .ok()
                        .and_then(|n| n.checked_add(offset))
                        .ok_or_else(|| {
                            err!("offset overflow when reading from `File`")
                        })?;
                }
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(Error::io(e)),
            }
        }
        if !buf.is_empty() {
            Err(Error::io(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "failed to fill whole buffer",
            )))
        } else {
            Ok(())
        }
    }
}

#[cfg(all(feature = "std", all(not(unix), not(windows))))]
impl Read for std::fs::File {
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> Result<(), Error> {
        use std::io::{Read as _, Seek as _, SeekFrom};
        let mut file = self;
        file.seek(SeekFrom::Start(offset)).map_err(Error::io).with_context(
            || err!("failed to seek to offset {offset} in `File`"),
        )?;
        file.read_exact(buf).map_err(Error::io)
    }
}

/// Allocates `additional` extra bytes on the `Vec` given and set them to `0`.
///
/// This specifically will never do an "OOM panic" and will instead return an
/// error (courtesy of `Vec::try_reserve_exact`). It will also return an error
/// without even trying the allocation if it's deemed to be "too big."
///
/// This is used so that we are extra careful about creating allocations based
/// on integers parsed from concatenated TZif data. Generally speaking, the
/// data we parse should be "trusted" (since it's probably not writable by
/// anyone other than `root`), but who knows where this code will ultimately be
/// used. So we try pretty hard to avoid panicking (even for OOM).
///
/// To be clear, we probably could panic on the error path. The goal here
/// isn't to avoid OOM because you can't allocate 10 bytes---Jiff isn't robust
/// enough in that kind of environment by far. The goal is to avoid OOM for
/// exorbitantly large allocations through some kind of attack vector.
fn alloc(bytes: &mut Vec<u8>, additional: usize) -> Result<(), Error> {
    // At time of writing, the biggest TZif data file is a few KB. And the
    // index block is tens of KB. So impose a limit that is a couple of orders
    // of magnitude bigger, but still overall pretty small for... some systems.
    // Anyway, I welcome improvements to this heuristic!
    const LIMIT: usize = 10 * 1 << 20;

    if additional > LIMIT {
        return Err(err!(
            "attempted to allocate more than {LIMIT} bytes \
             while reading concatenated TZif data, which \
             exceeds a heuristic limit to prevent huge allocations \
             (please file a bug if this error is inappropriate)",
        ));
    }
    bytes.try_reserve_exact(additional).map_err(|_| {
        err!(
            "failed to allocation {additional} bytes \
             for reading concatenated TZif data"
        )
    })?;
    // This... can't actually happen right?
    let new_len = bytes
        .len()
        .checked_add(additional)
        .ok_or_else(|| err!("total allocation length overflowed `usize`"))?;
    bytes.resize(new_len, 0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{
        civil::date,
        tz::{
            offset, testdata::ANDROID_CONCATENATED_TZIF, AmbiguousOffset,
            Offset,
        },
        Timestamp,
    };

    use super::*;

    fn unambiguous(offset_hours: i8) -> AmbiguousOffset {
        let offset = offset(offset_hours);
        o_unambiguous(offset)
    }

    fn gap(
        earlier_offset_hours: i8,
        later_offset_hours: i8,
    ) -> AmbiguousOffset {
        let earlier = offset(earlier_offset_hours);
        let later = offset(later_offset_hours);
        o_gap(earlier, later)
    }

    fn fold(
        earlier_offset_hours: i8,
        later_offset_hours: i8,
    ) -> AmbiguousOffset {
        let earlier = offset(earlier_offset_hours);
        let later = offset(later_offset_hours);
        o_fold(earlier, later)
    }

    fn o_unambiguous(offset: Offset) -> AmbiguousOffset {
        AmbiguousOffset::Unambiguous { offset }
    }

    fn o_gap(earlier: Offset, later: Offset) -> AmbiguousOffset {
        AmbiguousOffset::Gap { before: earlier, after: later }
    }

    fn o_fold(earlier: Offset, later: Offset) -> AmbiguousOffset {
        AmbiguousOffset::Fold { before: earlier, after: later }
    }

    // Copied from src/tz/mod.rs.
    #[test]
    fn time_zone_tzif_to_ambiguous_timestamp() {
        let tests: &[(&str, &[_])] = &[
            (
                "America/New_York",
                &[
                    ((1969, 12, 31, 19, 0, 0, 0), unambiguous(-5)),
                    ((2024, 3, 10, 1, 59, 59, 999_999_999), unambiguous(-5)),
                    ((2024, 3, 10, 2, 0, 0, 0), gap(-5, -4)),
                    ((2024, 3, 10, 2, 59, 59, 999_999_999), gap(-5, -4)),
                    ((2024, 3, 10, 3, 0, 0, 0), unambiguous(-4)),
                    ((2024, 11, 3, 0, 59, 59, 999_999_999), unambiguous(-4)),
                    ((2024, 11, 3, 1, 0, 0, 0), fold(-4, -5)),
                    ((2024, 11, 3, 1, 59, 59, 999_999_999), fold(-4, -5)),
                    ((2024, 11, 3, 2, 0, 0, 0), unambiguous(-5)),
                ],
            ),
            (
                "Europe/Dublin",
                &[
                    ((1970, 1, 1, 0, 0, 0, 0), unambiguous(1)),
                    ((2024, 3, 31, 0, 59, 59, 999_999_999), unambiguous(0)),
                    ((2024, 3, 31, 1, 0, 0, 0), gap(0, 1)),
                    ((2024, 3, 31, 1, 59, 59, 999_999_999), gap(0, 1)),
                    ((2024, 3, 31, 2, 0, 0, 0), unambiguous(1)),
                    ((2024, 10, 27, 0, 59, 59, 999_999_999), unambiguous(1)),
                    ((2024, 10, 27, 1, 0, 0, 0), fold(1, 0)),
                    ((2024, 10, 27, 1, 59, 59, 999_999_999), fold(1, 0)),
                    ((2024, 10, 27, 2, 0, 0, 0), unambiguous(0)),
                ],
            ),
            (
                "Australia/Tasmania",
                &[
                    ((1970, 1, 1, 11, 0, 0, 0), unambiguous(11)),
                    ((2024, 4, 7, 1, 59, 59, 999_999_999), unambiguous(11)),
                    ((2024, 4, 7, 2, 0, 0, 0), fold(11, 10)),
                    ((2024, 4, 7, 2, 59, 59, 999_999_999), fold(11, 10)),
                    ((2024, 4, 7, 3, 0, 0, 0), unambiguous(10)),
                    ((2024, 10, 6, 1, 59, 59, 999_999_999), unambiguous(10)),
                    ((2024, 10, 6, 2, 0, 0, 0), gap(10, 11)),
                    ((2024, 10, 6, 2, 59, 59, 999_999_999), gap(10, 11)),
                    ((2024, 10, 6, 3, 0, 0, 0), unambiguous(11)),
                ],
            ),
            (
                "Antarctica/Troll",
                &[
                    ((1970, 1, 1, 0, 0, 0, 0), unambiguous(0)),
                    // test the gap
                    ((2024, 3, 31, 0, 59, 59, 999_999_999), unambiguous(0)),
                    ((2024, 3, 31, 1, 0, 0, 0), gap(0, 2)),
                    ((2024, 3, 31, 1, 59, 59, 999_999_999), gap(0, 2)),
                    // still in the gap!
                    ((2024, 3, 31, 2, 0, 0, 0), gap(0, 2)),
                    ((2024, 3, 31, 2, 59, 59, 999_999_999), gap(0, 2)),
                    // finally out
                    ((2024, 3, 31, 3, 0, 0, 0), unambiguous(2)),
                    // test the fold
                    ((2024, 10, 27, 0, 59, 59, 999_999_999), unambiguous(2)),
                    ((2024, 10, 27, 1, 0, 0, 0), fold(2, 0)),
                    ((2024, 10, 27, 1, 59, 59, 999_999_999), fold(2, 0)),
                    // still in the fold!
                    ((2024, 10, 27, 2, 0, 0, 0), fold(2, 0)),
                    ((2024, 10, 27, 2, 59, 59, 999_999_999), fold(2, 0)),
                    // finally out
                    ((2024, 10, 27, 3, 0, 0, 0), unambiguous(0)),
                ],
            ),
            (
                "America/St_Johns",
                &[
                    (
                        (1969, 12, 31, 20, 30, 0, 0),
                        o_unambiguous(-Offset::hms(3, 30, 0)),
                    ),
                    (
                        (2024, 3, 10, 1, 59, 59, 999_999_999),
                        o_unambiguous(-Offset::hms(3, 30, 0)),
                    ),
                    (
                        (2024, 3, 10, 2, 0, 0, 0),
                        o_gap(-Offset::hms(3, 30, 0), -Offset::hms(2, 30, 0)),
                    ),
                    (
                        (2024, 3, 10, 2, 59, 59, 999_999_999),
                        o_gap(-Offset::hms(3, 30, 0), -Offset::hms(2, 30, 0)),
                    ),
                    (
                        (2024, 3, 10, 3, 0, 0, 0),
                        o_unambiguous(-Offset::hms(2, 30, 0)),
                    ),
                    (
                        (2024, 11, 3, 0, 59, 59, 999_999_999),
                        o_unambiguous(-Offset::hms(2, 30, 0)),
                    ),
                    (
                        (2024, 11, 3, 1, 0, 0, 0),
                        o_fold(-Offset::hms(2, 30, 0), -Offset::hms(3, 30, 0)),
                    ),
                    (
                        (2024, 11, 3, 1, 59, 59, 999_999_999),
                        o_fold(-Offset::hms(2, 30, 0), -Offset::hms(3, 30, 0)),
                    ),
                    (
                        (2024, 11, 3, 2, 0, 0, 0),
                        o_unambiguous(-Offset::hms(3, 30, 0)),
                    ),
                ],
            ),
            // This time zone has an interesting transition where it jumps
            // backwards a full day at 1867-10-19T15:30:00.
            (
                "America/Sitka",
                &[
                    ((1969, 12, 31, 16, 0, 0, 0), unambiguous(-8)),
                    (
                        (-9999, 1, 2, 16, 58, 46, 0),
                        o_unambiguous(Offset::hms(14, 58, 47)),
                    ),
                    (
                        (1867, 10, 18, 15, 29, 59, 0),
                        o_unambiguous(Offset::hms(14, 58, 47)),
                    ),
                    (
                        (1867, 10, 18, 15, 30, 0, 0),
                        // A fold of 24 hours!!!
                        o_fold(
                            Offset::hms(14, 58, 47),
                            -Offset::hms(9, 1, 13),
                        ),
                    ),
                    (
                        (1867, 10, 19, 15, 29, 59, 999_999_999),
                        // Still in the fold...
                        o_fold(
                            Offset::hms(14, 58, 47),
                            -Offset::hms(9, 1, 13),
                        ),
                    ),
                    (
                        (1867, 10, 19, 15, 30, 0, 0),
                        // Finally out.
                        o_unambiguous(-Offset::hms(9, 1, 13)),
                    ),
                ],
            ),
            // As with to_datetime, we test every possible transition
            // point here since this time zone has a small number of them.
            (
                "Pacific/Honolulu",
                &[
                    (
                        (1896, 1, 13, 11, 59, 59, 0),
                        o_unambiguous(-Offset::hms(10, 31, 26)),
                    ),
                    (
                        (1896, 1, 13, 12, 0, 0, 0),
                        o_gap(
                            -Offset::hms(10, 31, 26),
                            -Offset::hms(10, 30, 0),
                        ),
                    ),
                    (
                        (1896, 1, 13, 12, 1, 25, 0),
                        o_gap(
                            -Offset::hms(10, 31, 26),
                            -Offset::hms(10, 30, 0),
                        ),
                    ),
                    (
                        (1896, 1, 13, 12, 1, 26, 0),
                        o_unambiguous(-Offset::hms(10, 30, 0)),
                    ),
                    (
                        (1933, 4, 30, 1, 59, 59, 0),
                        o_unambiguous(-Offset::hms(10, 30, 0)),
                    ),
                    (
                        (1933, 4, 30, 2, 0, 0, 0),
                        o_gap(-Offset::hms(10, 30, 0), -Offset::hms(9, 30, 0)),
                    ),
                    (
                        (1933, 4, 30, 2, 59, 59, 0),
                        o_gap(-Offset::hms(10, 30, 0), -Offset::hms(9, 30, 0)),
                    ),
                    (
                        (1933, 4, 30, 3, 0, 0, 0),
                        o_unambiguous(-Offset::hms(9, 30, 0)),
                    ),
                    (
                        (1933, 5, 21, 10, 59, 59, 0),
                        o_unambiguous(-Offset::hms(9, 30, 0)),
                    ),
                    (
                        (1933, 5, 21, 11, 0, 0, 0),
                        o_fold(
                            -Offset::hms(9, 30, 0),
                            -Offset::hms(10, 30, 0),
                        ),
                    ),
                    (
                        (1933, 5, 21, 11, 59, 59, 0),
                        o_fold(
                            -Offset::hms(9, 30, 0),
                            -Offset::hms(10, 30, 0),
                        ),
                    ),
                    (
                        (1933, 5, 21, 12, 0, 0, 0),
                        o_unambiguous(-Offset::hms(10, 30, 0)),
                    ),
                    (
                        (1942, 2, 9, 1, 59, 59, 0),
                        o_unambiguous(-Offset::hms(10, 30, 0)),
                    ),
                    (
                        (1942, 2, 9, 2, 0, 0, 0),
                        o_gap(-Offset::hms(10, 30, 0), -Offset::hms(9, 30, 0)),
                    ),
                    (
                        (1942, 2, 9, 2, 59, 59, 0),
                        o_gap(-Offset::hms(10, 30, 0), -Offset::hms(9, 30, 0)),
                    ),
                    (
                        (1942, 2, 9, 3, 0, 0, 0),
                        o_unambiguous(-Offset::hms(9, 30, 0)),
                    ),
                    (
                        (1945, 8, 14, 13, 29, 59, 0),
                        o_unambiguous(-Offset::hms(9, 30, 0)),
                    ),
                    (
                        (1945, 8, 14, 13, 30, 0, 0),
                        o_unambiguous(-Offset::hms(9, 30, 0)),
                    ),
                    (
                        (1945, 8, 14, 13, 30, 1, 0),
                        o_unambiguous(-Offset::hms(9, 30, 0)),
                    ),
                    (
                        (1945, 9, 30, 0, 59, 59, 0),
                        o_unambiguous(-Offset::hms(9, 30, 0)),
                    ),
                    (
                        (1945, 9, 30, 1, 0, 0, 0),
                        o_fold(
                            -Offset::hms(9, 30, 0),
                            -Offset::hms(10, 30, 0),
                        ),
                    ),
                    (
                        (1945, 9, 30, 1, 59, 59, 0),
                        o_fold(
                            -Offset::hms(9, 30, 0),
                            -Offset::hms(10, 30, 0),
                        ),
                    ),
                    (
                        (1945, 9, 30, 2, 0, 0, 0),
                        o_unambiguous(-Offset::hms(10, 30, 0)),
                    ),
                    (
                        (1947, 6, 8, 1, 59, 59, 0),
                        o_unambiguous(-Offset::hms(10, 30, 0)),
                    ),
                    (
                        (1947, 6, 8, 2, 0, 0, 0),
                        o_gap(-Offset::hms(10, 30, 0), -offset(10)),
                    ),
                    (
                        (1947, 6, 8, 2, 29, 59, 0),
                        o_gap(-Offset::hms(10, 30, 0), -offset(10)),
                    ),
                    ((1947, 6, 8, 2, 30, 0, 0), unambiguous(-10)),
                ],
            ),
        ];
        let db = ConcatenatedTzif::open(ANDROID_CONCATENATED_TZIF).unwrap();
        let (mut buf1, mut buf2) = (alloc::vec![], alloc::vec![]);
        for &(tzname, datetimes_to_ambiguous) in tests {
            let tz = db.get(tzname, &mut buf1, &mut buf2).unwrap().unwrap();
            for &(datetime, ambiguous_kind) in datetimes_to_ambiguous {
                let (year, month, day, hour, min, sec, nano) = datetime;
                let dt = date(year, month, day).at(hour, min, sec, nano);
                let got = tz.to_ambiguous_zoned(dt);
                assert_eq!(
                    got.offset(),
                    ambiguous_kind,
                    "\nTZ: {tzname}\ndatetime: \
                     {year:04}-{month:02}-{day:02}T\
                     {hour:02}:{min:02}:{sec:02}.{nano:09}",
                );
            }
        }
    }

    // Copied from src/tz/mod.rs.
    #[test]
    fn time_zone_tzif_to_datetime() {
        let o = |hours| offset(hours);
        let tests: &[(&str, &[_])] = &[
            (
                "America/New_York",
                &[
                    ((0, 0), o(-5), "EST", (1969, 12, 31, 19, 0, 0, 0)),
                    (
                        (1710052200, 0),
                        o(-5),
                        "EST",
                        (2024, 3, 10, 1, 30, 0, 0),
                    ),
                    (
                        (1710053999, 999_999_999),
                        o(-5),
                        "EST",
                        (2024, 3, 10, 1, 59, 59, 999_999_999),
                    ),
                    ((1710054000, 0), o(-4), "EDT", (2024, 3, 10, 3, 0, 0, 0)),
                    (
                        (1710055800, 0),
                        o(-4),
                        "EDT",
                        (2024, 3, 10, 3, 30, 0, 0),
                    ),
                    ((1730610000, 0), o(-4), "EDT", (2024, 11, 3, 1, 0, 0, 0)),
                    (
                        (1730611800, 0),
                        o(-4),
                        "EDT",
                        (2024, 11, 3, 1, 30, 0, 0),
                    ),
                    (
                        (1730613599, 999_999_999),
                        o(-4),
                        "EDT",
                        (2024, 11, 3, 1, 59, 59, 999_999_999),
                    ),
                    ((1730613600, 0), o(-5), "EST", (2024, 11, 3, 1, 0, 0, 0)),
                    (
                        (1730615400, 0),
                        o(-5),
                        "EST",
                        (2024, 11, 3, 1, 30, 0, 0),
                    ),
                ],
            ),
            (
                "Australia/Tasmania",
                &[
                    ((0, 0), o(11), "AEDT", (1970, 1, 1, 11, 0, 0, 0)),
                    (
                        (1728142200, 0),
                        o(10),
                        "AEST",
                        (2024, 10, 6, 1, 30, 0, 0),
                    ),
                    (
                        (1728143999, 999_999_999),
                        o(10),
                        "AEST",
                        (2024, 10, 6, 1, 59, 59, 999_999_999),
                    ),
                    (
                        (1728144000, 0),
                        o(11),
                        "AEDT",
                        (2024, 10, 6, 3, 0, 0, 0),
                    ),
                    (
                        (1728145800, 0),
                        o(11),
                        "AEDT",
                        (2024, 10, 6, 3, 30, 0, 0),
                    ),
                    ((1712415600, 0), o(11), "AEDT", (2024, 4, 7, 2, 0, 0, 0)),
                    (
                        (1712417400, 0),
                        o(11),
                        "AEDT",
                        (2024, 4, 7, 2, 30, 0, 0),
                    ),
                    (
                        (1712419199, 999_999_999),
                        o(11),
                        "AEDT",
                        (2024, 4, 7, 2, 59, 59, 999_999_999),
                    ),
                    ((1712419200, 0), o(10), "AEST", (2024, 4, 7, 2, 0, 0, 0)),
                    (
                        (1712421000, 0),
                        o(10),
                        "AEST",
                        (2024, 4, 7, 2, 30, 0, 0),
                    ),
                ],
            ),
            // Pacific/Honolulu is small eough that we just test every
            // possible instant before, at and after each transition.
            (
                "Pacific/Honolulu",
                &[
                    (
                        (-2334101315, 0),
                        -Offset::hms(10, 31, 26),
                        "LMT",
                        (1896, 1, 13, 11, 59, 59, 0),
                    ),
                    (
                        (-2334101314, 0),
                        -Offset::hms(10, 30, 0),
                        "HST",
                        (1896, 1, 13, 12, 1, 26, 0),
                    ),
                    (
                        (-2334101313, 0),
                        -Offset::hms(10, 30, 0),
                        "HST",
                        (1896, 1, 13, 12, 1, 27, 0),
                    ),
                    (
                        (-1157283001, 0),
                        -Offset::hms(10, 30, 0),
                        "HST",
                        (1933, 4, 30, 1, 59, 59, 0),
                    ),
                    (
                        (-1157283000, 0),
                        -Offset::hms(9, 30, 0),
                        "HDT",
                        (1933, 4, 30, 3, 0, 0, 0),
                    ),
                    (
                        (-1157282999, 0),
                        -Offset::hms(9, 30, 0),
                        "HDT",
                        (1933, 4, 30, 3, 0, 1, 0),
                    ),
                    (
                        (-1155436201, 0),
                        -Offset::hms(9, 30, 0),
                        "HDT",
                        (1933, 5, 21, 11, 59, 59, 0),
                    ),
                    (
                        (-1155436200, 0),
                        -Offset::hms(10, 30, 0),
                        "HST",
                        (1933, 5, 21, 11, 0, 0, 0),
                    ),
                    (
                        (-1155436199, 0),
                        -Offset::hms(10, 30, 0),
                        "HST",
                        (1933, 5, 21, 11, 0, 1, 0),
                    ),
                    (
                        (-880198201, 0),
                        -Offset::hms(10, 30, 0),
                        "HST",
                        (1942, 2, 9, 1, 59, 59, 0),
                    ),
                    (
                        (-880198200, 0),
                        -Offset::hms(9, 30, 0),
                        "HWT",
                        (1942, 2, 9, 3, 0, 0, 0),
                    ),
                    (
                        (-880198199, 0),
                        -Offset::hms(9, 30, 0),
                        "HWT",
                        (1942, 2, 9, 3, 0, 1, 0),
                    ),
                    (
                        (-769395601, 0),
                        -Offset::hms(9, 30, 0),
                        "HWT",
                        (1945, 8, 14, 13, 29, 59, 0),
                    ),
                    (
                        (-769395600, 0),
                        -Offset::hms(9, 30, 0),
                        "HPT",
                        (1945, 8, 14, 13, 30, 0, 0),
                    ),
                    (
                        (-769395599, 0),
                        -Offset::hms(9, 30, 0),
                        "HPT",
                        (1945, 8, 14, 13, 30, 1, 0),
                    ),
                    (
                        (-765376201, 0),
                        -Offset::hms(9, 30, 0),
                        "HPT",
                        (1945, 9, 30, 1, 59, 59, 0),
                    ),
                    (
                        (-765376200, 0),
                        -Offset::hms(10, 30, 0),
                        "HST",
                        (1945, 9, 30, 1, 0, 0, 0),
                    ),
                    (
                        (-765376199, 0),
                        -Offset::hms(10, 30, 0),
                        "HST",
                        (1945, 9, 30, 1, 0, 1, 0),
                    ),
                    (
                        (-712150201, 0),
                        -Offset::hms(10, 30, 0),
                        "HST",
                        (1947, 6, 8, 1, 59, 59, 0),
                    ),
                    // At this point, we hit the last transition and the POSIX
                    // TZ string takes over.
                    (
                        (-712150200, 0),
                        -Offset::hms(10, 0, 0),
                        "HST",
                        (1947, 6, 8, 2, 30, 0, 0),
                    ),
                    (
                        (-712150199, 0),
                        -Offset::hms(10, 0, 0),
                        "HST",
                        (1947, 6, 8, 2, 30, 1, 0),
                    ),
                ],
            ),
            // This time zone has an interesting transition where it jumps
            // backwards a full day at 1867-10-19T15:30:00.
            (
                "America/Sitka",
                &[
                    ((0, 0), o(-8), "PST", (1969, 12, 31, 16, 0, 0, 0)),
                    (
                        (-377705023201, 0),
                        Offset::hms(14, 58, 47),
                        "LMT",
                        (-9999, 1, 2, 16, 58, 46, 0),
                    ),
                    (
                        (-3225223728, 0),
                        Offset::hms(14, 58, 47),
                        "LMT",
                        (1867, 10, 19, 15, 29, 59, 0),
                    ),
                    // Notice the 24 hour time jump backwards a whole day!
                    (
                        (-3225223727, 0),
                        -Offset::hms(9, 1, 13),
                        "LMT",
                        (1867, 10, 18, 15, 30, 0, 0),
                    ),
                    (
                        (-3225223726, 0),
                        -Offset::hms(9, 1, 13),
                        "LMT",
                        (1867, 10, 18, 15, 30, 1, 0),
                    ),
                ],
            ),
        ];
        let db = ConcatenatedTzif::open(ANDROID_CONCATENATED_TZIF).unwrap();
        let (mut buf1, mut buf2) = (alloc::vec![], alloc::vec![]);
        for &(tzname, timestamps_to_datetimes) in tests {
            let tz = db.get(tzname, &mut buf1, &mut buf2).unwrap().unwrap();
            for &((unix_sec, unix_nano), offset, abbrev, datetime) in
                timestamps_to_datetimes
            {
                let (year, month, day, hour, min, sec, nano) = datetime;
                let timestamp = Timestamp::new(unix_sec, unix_nano).unwrap();
                let info = tz.to_offset_info(timestamp);
                assert_eq!(
                    info.offset(),
                    offset,
                    "\nTZ={tzname}, timestamp({unix_sec}, {unix_nano})",
                );
                assert_eq!(
                    info.abbreviation(),
                    abbrev,
                    "\nTZ={tzname}, timestamp({unix_sec}, {unix_nano})",
                );
                assert_eq!(
                    info.offset().to_datetime(timestamp),
                    date(year, month, day).at(hour, min, sec, nano),
                    "\nTZ={tzname}, timestamp({unix_sec}, {unix_nano})",
                );
            }
        }
    }

    #[test]
    #[cfg(not(miri))]
    fn read_all_time_zones() {
        let db = ConcatenatedTzif::open(ANDROID_CONCATENATED_TZIF).unwrap();
        let available = db.available(&mut alloc::vec![]).unwrap();
        let (mut buf1, mut buf2) = (alloc::vec![], alloc::vec![]);
        for tzname in available.iter() {
            let tz = db.get(tzname, &mut buf1, &mut buf2).unwrap().unwrap();
            assert_eq!(tzname, tz.iana_name().unwrap());
        }
    }

    #[test]
    fn available_len() {
        let db = ConcatenatedTzif::open(ANDROID_CONCATENATED_TZIF).unwrap();
        let available = db.available(&mut alloc::vec![]).unwrap();
        assert_eq!(596, available.len());
        for window in available.windows(2) {
            let (x1, x2) = (&window[0], &window[1]);
            assert!(x1 < x2, "{x1} is not less than {x2}");
        }
    }
}
