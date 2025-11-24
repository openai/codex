use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;
use core::marker::PhantomData;
use core::ops::ControlFlow;
#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{self, ErrorKind};

use crate::base64;

/// Items that can be decoded from PEM data.
pub trait PemObject: Sized {
    /// Decode the first section of this type from PEM contained in
    /// a byte slice.
    ///
    /// [`Error::NoItemsFound`] is returned if no such items are found.
    fn from_pem_slice(pem: &[u8]) -> Result<Self, Error> {
        Self::pem_slice_iter(pem)
            .next()
            .unwrap_or(Err(Error::NoItemsFound))
    }

    /// Iterate over all sections of this type from PEM contained in
    /// a byte slice.
    fn pem_slice_iter(pem: &[u8]) -> SliceIter<'_, Self> {
        SliceIter {
            current: pem,
            _ty: PhantomData,
        }
    }

    /// Decode the first section of this type from the PEM contents of the named file.
    ///
    /// [`Error::NoItemsFound`] is returned if no such items are found.
    #[cfg(feature = "std")]
    fn from_pem_file(file_name: impl AsRef<std::path::Path>) -> Result<Self, Error> {
        Self::pem_file_iter(file_name)?
            .next()
            .unwrap_or(Err(Error::NoItemsFound))
    }

    /// Iterate over all sections of this type from the PEM contents of the named file.
    ///
    /// This reports errors in two phases:
    ///
    /// - errors opening the file are reported from this function directly,
    /// - errors reading from the file are reported from the returned iterator,
    #[cfg(feature = "std")]
    fn pem_file_iter(
        file_name: impl AsRef<std::path::Path>,
    ) -> Result<ReadIter<io::BufReader<File>, Self>, Error> {
        Ok(ReadIter::<_, Self> {
            rd: io::BufReader::new(File::open(file_name).map_err(Error::Io)?),
            _ty: PhantomData,
        })
    }

    /// Decode the first section of this type from PEM read from an [`io::Read`].
    #[cfg(feature = "std")]
    fn from_pem_reader(rd: impl std::io::Read) -> Result<Self, Error> {
        Self::pem_reader_iter(rd)
            .next()
            .unwrap_or(Err(Error::NoItemsFound))
    }

    /// Iterate over all sections of this type from PEM present in an [`io::Read`].
    #[cfg(feature = "std")]
    fn pem_reader_iter<R: std::io::Read>(rd: R) -> ReadIter<io::BufReader<R>, Self> {
        ReadIter::<_, Self> {
            rd: io::BufReader::new(rd),
            _ty: PhantomData,
        }
    }

    /// Conversion from a PEM [`SectionKind`] and body data.
    ///
    /// This inspects `kind`, and if it matches this type's PEM section kind,
    /// converts `der` into this type.
    fn from_pem(kind: SectionKind, der: Vec<u8>) -> Option<Self>;
}

pub(crate) trait PemObjectFilter: PemObject + From<Vec<u8>> {
    const KIND: SectionKind;
}

impl<T: PemObjectFilter + From<Vec<u8>>> PemObject for T {
    fn from_pem(kind: SectionKind, der: Vec<u8>) -> Option<Self> {
        match Self::KIND == kind {
            true => Some(Self::from(der)),
            false => None,
        }
    }
}

/// Extract and return all PEM sections by reading `rd`.
#[cfg(feature = "std")]
pub struct ReadIter<R, T> {
    rd: R,
    _ty: PhantomData<T>,
}

#[cfg(feature = "std")]
impl<R: io::BufRead, T: PemObject> ReadIter<R, T> {
    /// Create a new iterator.
    pub fn new(rd: R) -> Self {
        Self {
            rd,
            _ty: PhantomData,
        }
    }
}

#[cfg(feature = "std")]
impl<R: io::BufRead, T: PemObject> Iterator for ReadIter<R, T> {
    type Item = Result<T, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            return match from_buf(&mut self.rd) {
                Ok(Some((sec, item))) => match T::from_pem(sec, item) {
                    Some(res) => Some(Ok(res)),
                    None => continue,
                },
                Ok(None) => return None,
                Err(err) => Some(Err(err)),
            };
        }
    }
}

/// Iterator over all PEM sections in a `&[u8]` slice.
pub struct SliceIter<'a, T> {
    current: &'a [u8],
    _ty: PhantomData<T>,
}

impl<'a, T: PemObject> SliceIter<'a, T> {
    /// Create a new iterator.
    pub fn new(current: &'a [u8]) -> Self {
        Self {
            current,
            _ty: PhantomData,
        }
    }

    /// Returns the rest of the unparsed data.
    ///
    /// This is the slice immediately following the most
    /// recently returned item from `next()`.
    #[doc(hidden)]
    pub fn remainder(&self) -> &'a [u8] {
        self.current
    }
}

impl<T: PemObject> Iterator for SliceIter<'_, T> {
    type Item = Result<T, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            return match from_slice(self.current) {
                Ok(Some(((sec, item), rest))) => {
                    self.current = rest;
                    match T::from_pem(sec, item) {
                        Some(res) => Some(Ok(res)),
                        None => continue,
                    }
                }
                Ok(None) => return None,
                Err(err) => Some(Err(err)),
            };
        }
    }
}

impl PemObject for (SectionKind, Vec<u8>) {
    fn from_pem(kind: SectionKind, der: Vec<u8>) -> Option<Self> {
        Some((kind, der))
    }
}

/// Extract and decode the next supported PEM section from `input`
///
/// - `Ok(None)` is returned if there is no PEM section to read from `input`
/// - Syntax errors and decoding errors produce a `Err(...)`
/// - Otherwise each decoded section is returned with a `Ok(Some((..., remainder)))` where
///   `remainder` is the part of the `input` that follows the returned section
#[allow(clippy::type_complexity)]
fn from_slice(mut input: &[u8]) -> Result<Option<((SectionKind, Vec<u8>), &[u8])>, Error> {
    let mut b64buf = Vec::with_capacity(1024);
    let mut section = None::<(Vec<_>, Vec<_>)>;

    loop {
        let next_line = if let Some(index) = input
            .iter()
            .position(|byte| *byte == b'\n' || *byte == b'\r')
        {
            let (line, newline_plus_remainder) = input.split_at(index);
            input = &newline_plus_remainder[1..];
            Some(line)
        } else if !input.is_empty() {
            let next_line = input;
            input = &[];
            Some(next_line)
        } else {
            None
        };

        match read(next_line, &mut section, &mut b64buf)? {
            ControlFlow::Continue(()) => continue,
            ControlFlow::Break(item) => return Ok(item.map(|item| (item, input))),
        }
    }
}

/// Extract and decode the next supported PEM section from `rd`.
///
/// - Ok(None) is returned if there is no PEM section read from `rd`.
/// - Underlying IO errors produce a `Err(...)`
/// - Otherwise each decoded section is returned with a `Ok(Some(...))`
#[cfg(feature = "std")]
pub fn from_buf(rd: &mut dyn io::BufRead) -> Result<Option<(SectionKind, Vec<u8>)>, Error> {
    let mut b64buf = Vec::with_capacity(1024);
    let mut section = None::<(Vec<_>, Vec<_>)>;
    let mut line = Vec::with_capacity(80);

    loop {
        line.clear();
        let len = read_until_newline(rd, &mut line).map_err(Error::Io)?;

        let next_line = if len == 0 {
            None
        } else {
            Some(line.as_slice())
        };

        match read(next_line, &mut section, &mut b64buf) {
            Ok(ControlFlow::Break(opt)) => return Ok(opt),
            Ok(ControlFlow::Continue(())) => continue,
            Err(e) => return Err(e),
        }
    }
}

#[allow(clippy::type_complexity)]
fn read(
    next_line: Option<&[u8]>,
    section: &mut Option<(Vec<u8>, Vec<u8>)>,
    b64buf: &mut Vec<u8>,
) -> Result<ControlFlow<Option<(SectionKind, Vec<u8>)>, ()>, Error> {
    let line = if let Some(line) = next_line {
        line
    } else {
        // EOF
        return match section.take() {
            Some((_, end_marker)) => Err(Error::MissingSectionEnd { end_marker }),
            None => Ok(ControlFlow::Break(None)),
        };
    };

    if line.starts_with(b"-----BEGIN ") {
        let (mut trailer, mut pos) = (0, line.len());
        for (i, &b) in line.iter().enumerate().rev() {
            match b {
                b'-' => {
                    trailer += 1;
                    pos = i;
                }
                b'\n' | b'\r' | b' ' => continue,
                _ => break,
            }
        }

        if trailer != 5 {
            return Err(Error::IllegalSectionStart {
                line: line.to_vec(),
            });
        }

        let ty = &line[11..pos];
        let mut end = Vec::with_capacity(10 + 4 + ty.len());
        end.extend_from_slice(b"-----END ");
        end.extend_from_slice(ty);
        end.extend_from_slice(b"-----");
        *section = Some((ty.to_owned(), end));
        return Ok(ControlFlow::Continue(()));
    }

    if let Some((section_label, end_marker)) = section.as_ref() {
        if line.starts_with(end_marker) {
            let kind = match SectionKind::try_from(&section_label[..]) {
                Ok(kind) => kind,
                // unhandled section: have caller try again
                Err(()) => {
                    *section = None;
                    b64buf.clear();
                    return Ok(ControlFlow::Continue(()));
                }
            };

            let mut der = vec![0u8; base64::decoded_length(b64buf.len())];
            let der_len = match kind.secret() {
                true => base64::decode_secret(b64buf, &mut der),
                false => base64::decode_public(b64buf, &mut der),
            }
            .map_err(|err| Error::Base64Decode(format!("{err:?}")))?
            .len();

            der.truncate(der_len);

            return Ok(ControlFlow::Break(Some((kind, der))));
        }
    }

    if section.is_some() {
        b64buf.extend(line);
    }

    Ok(ControlFlow::Continue(()))
}

/// A single recognised section in a PEM file.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SectionKind {
    /// A DER-encoded x509 certificate.
    ///
    /// Appears as "CERTIFICATE" in PEM files.
    Certificate,

    /// A DER-encoded Subject Public Key Info; as specified in RFC 7468.
    ///
    /// Appears as "PUBLIC KEY" in PEM files.
    PublicKey,

    /// A DER-encoded plaintext RSA private key; as specified in PKCS #1/RFC 3447
    ///
    /// Appears as "RSA PRIVATE KEY" in PEM files.
    RsaPrivateKey,

    /// A DER-encoded plaintext private key; as specified in PKCS #8/RFC 5958
    ///
    /// Appears as "PRIVATE KEY" in PEM files.
    PrivateKey,

    /// A Sec1-encoded plaintext private key; as specified in RFC 5915
    ///
    /// Appears as "EC PRIVATE KEY" in PEM files.
    EcPrivateKey,

    /// A Certificate Revocation List; as specified in RFC 5280
    ///
    /// Appears as "X509 CRL" in PEM files.
    Crl,

    /// A Certificate Signing Request; as specified in RFC 2986
    ///
    /// Appears as "CERTIFICATE REQUEST" in PEM files.
    Csr,

    /// An EchConfigList structure, as specified in
    /// <https://www.ietf.org/archive/id/draft-farrell-tls-pemesni-05.html>.
    ///
    /// Appears as "ECHCONFIG" in PEM files.
    EchConfigList,
}

impl SectionKind {
    fn secret(&self) -> bool {
        match self {
            Self::RsaPrivateKey | Self::PrivateKey | Self::EcPrivateKey => true,
            Self::Certificate | Self::PublicKey | Self::Crl | Self::Csr | Self::EchConfigList => {
                false
            }
        }
    }
}

impl TryFrom<&[u8]> for SectionKind {
    type Error = ();

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        Ok(match value {
            b"CERTIFICATE" => Self::Certificate,
            b"PUBLIC KEY" => Self::PublicKey,
            b"RSA PRIVATE KEY" => Self::RsaPrivateKey,
            b"PRIVATE KEY" => Self::PrivateKey,
            b"EC PRIVATE KEY" => Self::EcPrivateKey,
            b"X509 CRL" => Self::Crl,
            b"CERTIFICATE REQUEST" => Self::Csr,
            b"ECHCONFIG" => Self::EchConfigList,
            _ => return Err(()),
        })
    }
}

/// Errors that may arise when parsing the contents of a PEM file
#[non_exhaustive]
#[derive(Debug)]
pub enum Error {
    /// a section is missing its "END marker" line
    MissingSectionEnd {
        /// the expected "END marker" line that was not found
        end_marker: Vec<u8>,
    },

    /// syntax error found in the line that starts a new section
    IllegalSectionStart {
        /// line that contains the syntax error
        line: Vec<u8>,
    },

    /// base64 decode error
    Base64Decode(String),

    /// I/O errors, from APIs that accept `std::io` types.
    #[cfg(feature = "std")]
    Io(io::Error),

    /// No items found of desired type
    NoItemsFound,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSectionEnd { end_marker } => {
                write!(f, "missing section end marker: {end_marker:?}")
            }
            Self::IllegalSectionStart { line } => {
                write!(f, "illegal section start: {line:?}")
            }
            Self::Base64Decode(e) => write!(f, "base64 decode error: {e}"),
            #[cfg(feature = "std")]
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::NoItemsFound => write!(f, "no items found"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

// Ported from https://github.com/rust-lang/rust/blob/91cfcb021935853caa06698b759c293c09d1e96a/library/std/src/io/mod.rs#L1990 and
// modified to look for our accepted newlines.
#[cfg(feature = "std")]
fn read_until_newline<R: io::BufRead + ?Sized>(r: &mut R, buf: &mut Vec<u8>) -> io::Result<usize> {
    let mut read = 0;
    loop {
        let (done, used) = {
            let available = match r.fill_buf() {
                Ok(n) => n,
                Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };
            match available
                .iter()
                .copied()
                .position(|b| b == b'\n' || b == b'\r')
            {
                Some(i) => {
                    buf.extend_from_slice(&available[..=i]);
                    (true, i + 1)
                }
                None => {
                    buf.extend_from_slice(available);
                    (false, available.len())
                }
            }
        };
        r.consume(used);
        read += used;
        if done || used == 0 {
            return Ok(read);
        }
    }
}
