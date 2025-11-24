use crate::{
    error::{err, Error},
    util::escape::{Byte, Bytes},
};

/// Parses an `i64` number from the beginning to the end of the given slice of
/// ASCII digit characters.
///
/// If any byte in the given slice is not `[0-9]`, then this returns an error.
/// Similarly, if the number parsed does not fit into a `i64`, then this
/// returns an error. Notably, this routine does not permit parsing a negative
/// integer. (We use `i64` because everything in this crate uses signed
/// integers, and because a higher level routine might want to parse the sign
/// and then apply it to the result of this routine.)
#[cfg_attr(feature = "perf-inline", inline(always))]
pub(crate) fn i64(bytes: &[u8]) -> Result<i64, Error> {
    if bytes.is_empty() {
        return Err(err!("invalid number, no digits found"));
    }
    let mut n: i64 = 0;
    for &byte in bytes {
        let digit = match byte.checked_sub(b'0') {
            None => {
                return Err(err!(
                    "invalid digit, expected 0-9 but got {}",
                    Byte(byte),
                ));
            }
            Some(digit) if digit > 9 => {
                return Err(err!(
                    "invalid digit, expected 0-9 but got {}",
                    Byte(byte),
                ))
            }
            Some(digit) => {
                debug_assert!((0..=9).contains(&digit));
                i64::from(digit)
            }
        };
        n = n.checked_mul(10).and_then(|n| n.checked_add(digit)).ok_or_else(
            || {
                err!(
                    "number '{}' too big to parse into 64-bit integer",
                    Bytes(bytes),
                )
            },
        )?;
    }
    Ok(n)
}

/// Parses an `i64` fractional number from the beginning to the end of the
/// given slice of ASCII digit characters.
///
/// The fraction's maximum precision must be provided. The returned integer
/// will always be in units of `10^{max_precision}`. For example, to parse a
/// fractional amount of seconds with a maximum precision of nanoseconds, then
/// use `max_precision=9`.
///
/// If any byte in the given slice is not `[0-9]`, then this returns an error.
/// Similarly, if the fraction parsed does not fit into a `i64`, then this
/// returns an error. Notably, this routine does not permit parsing a negative
/// integer. (We use `i64` because everything in this crate uses signed
/// integers, and because a higher level routine might want to parse the sign
/// and then apply it to the result of this routine.)
pub(crate) fn fraction(
    bytes: &[u8],
    max_precision: usize,
) -> Result<i64, Error> {
    if bytes.is_empty() {
        return Err(err!("invalid fraction, no digits found"));
    } else if bytes.len() > max_precision {
        return Err(err!(
            "invalid fraction, too many digits \
             (at most {max_precision} are allowed"
        ));
    }
    let mut n: i64 = 0;
    for &byte in bytes {
        let digit = match byte.checked_sub(b'0') {
            None => {
                return Err(err!(
                    "invalid fractional digit, expected 0-9 but got {}",
                    Byte(byte),
                ));
            }
            Some(digit) if digit > 9 => {
                return Err(err!(
                    "invalid fractional digit, expected 0-9 but got {}",
                    Byte(byte),
                ))
            }
            Some(digit) => {
                debug_assert!((0..=9).contains(&digit));
                i64::from(digit)
            }
        };
        n = n.checked_mul(10).and_then(|n| n.checked_add(digit)).ok_or_else(
            || {
                err!(
                    "fractional '{}' too big to parse into 64-bit integer",
                    Bytes(bytes),
                )
            },
        )?;
    }
    for _ in bytes.len()..max_precision {
        n = n.checked_mul(10).ok_or_else(|| {
            err!(
                "fractional '{}' too big to parse into 64-bit integer \
                 (too much precision supported)",
                Bytes(bytes)
            )
        })?;
    }
    Ok(n)
}

/// Parses an `OsStr` into a `&str` when `&[u8]` isn't easily available.
///
/// This is effectively `OsStr::to_str`, but with a slightly better error
/// message.
#[cfg(feature = "tzdb-zoneinfo")]
pub(crate) fn os_str_utf8<'o, O>(os_str: &'o O) -> Result<&'o str, Error>
where
    O: ?Sized + AsRef<std::ffi::OsStr>,
{
    let os_str = os_str.as_ref();
    os_str
        .to_str()
        .ok_or_else(|| err!("environment value {os_str:?} is not valid UTF-8"))
}

/// Parses an `OsStr` into a `&str` when `&[u8]` isn't easily available.
///
/// The main difference between this and `OsStr::to_str` is that this will
/// be a zero-cost conversion on Unix platforms to `&[u8]`. On Windows, this
/// will do UTF-8 validation and return an error if it's invalid UTF-8.
#[cfg(feature = "tz-system")]
pub(crate) fn os_str_bytes<'o, O>(os_str: &'o O) -> Result<&'o [u8], Error>
where
    O: ?Sized + AsRef<std::ffi::OsStr>,
{
    let os_str = os_str.as_ref();
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Ok(os_str.as_bytes())
    }
    #[cfg(not(unix))]
    {
        let string = os_str.to_str().ok_or_else(|| {
            err!("environment value {os_str:?} is not valid UTF-8")
        })?;
        // It is suspect that we're doing UTF-8 validation and then throwing
        // away the fact that we did UTF-8 validation. So this could lead
        // to an extra UTF-8 check if the caller ultimately needs UTF-8. If
        // that's important, we can add a new API that returns a `&str`. But it
        // probably won't matter because an `OsStr` in this crate is usually
        // just an environment variable.
        Ok(string.as_bytes())
    }
}

/// Splits the given input into two slices at the given position.
///
/// If the position is greater than the length of the slice given, then this
/// returns `None`.
#[cfg_attr(feature = "perf-inline", inline(always))]
pub(crate) fn split(input: &[u8], at: usize) -> Option<(&[u8], &[u8])> {
    if at > input.len() {
        None
    } else {
        Some(input.split_at(at))
    }
}

/// Returns a function that converts two slices to an offset.
///
/// It takes the starting point as input and returns a function that, when
/// given an ending point (greater than or equal to the starting point), then
/// the corresponding pointers are subtracted and an offset relative to the
/// starting point is returned.
///
/// This is useful as a helper function in parsing routines that use slices
/// but want to report offsets.
///
/// # Panics
///
/// This may panic if the ending point is not a suffix slice of `start`.
pub(crate) fn offseter<'a>(
    start: &'a [u8],
) -> impl Fn(&'a [u8]) -> usize + 'a {
    move |end| (end.as_ptr() as usize) - (start.as_ptr() as usize)
}

/// Returns a function that converts two slices to the slice between them.
///
/// This takes a starting point as input and returns a function that, when
/// given an ending point (greater than or equal to the starting point), it
/// returns a slice beginning at the starting point and ending just at the
/// ending point.
///
/// This is useful as a helper function in parsing routines.
///
/// # Panics
///
/// This may panic if the ending point is not a suffix slice of `start`.
pub(crate) fn slicer<'a>(
    start: &'a [u8],
) -> impl Fn(&'a [u8]) -> &'a [u8] + 'a {
    let mkoffset = offseter(start);
    move |end| {
        let offset = mkoffset(end);
        &start[..offset]
    }
}
