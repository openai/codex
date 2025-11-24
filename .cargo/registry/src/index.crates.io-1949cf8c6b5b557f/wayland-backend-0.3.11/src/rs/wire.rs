//! Types and routines used to manipulate arguments from the wire format

use std::collections::VecDeque;
use std::ffi::CStr;
use std::os::unix::io::{BorrowedFd, OwnedFd, RawFd};

use crate::protocol::{Argument, ArgumentType, Message};

use smallvec::SmallVec;

/// Error generated when trying to serialize a message into buffers
#[derive(Debug)]
pub enum MessageWriteError {
    /// The buffer is too small to hold the message contents
    BufferTooSmall,
    /// The message contains a FD that could not be dup-ed
    DupFdFailed(std::io::Error),
}

impl std::error::Error for MessageWriteError {}

impl std::fmt::Display for MessageWriteError {
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
        match self {
            Self::BufferTooSmall => {
                f.write_str("The provided buffer is too small to hold message content.")
            }
            Self::DupFdFailed(e) => {
                write!(
                    f,
                    "The message contains a file descriptor that could not be dup()-ed ({e})."
                )
            }
        }
    }
}

/// Error generated when trying to deserialize a message from buffers
#[derive(Debug, Clone)]
pub enum MessageParseError {
    /// The message references a FD but the buffer FD is empty
    MissingFD,
    /// More data is needed to deserialize the message
    MissingData,
    /// The message is malformed and cannot be parsed
    Malformed,
}

impl std::error::Error for MessageParseError {}

impl std::fmt::Display for MessageParseError {
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
        match *self {
            Self::MissingFD => {
                f.write_str("The message references a FD but the buffer FD is empty.")
            }
            Self::MissingData => f.write_str("More data is needed to deserialize the message"),
            Self::Malformed => f.write_str("The message is malformed and cannot be parsed"),
        }
    }
}

/// Serialize the contents of this message into provided buffers
///
/// Returns the number of elements written in each buffer
///
/// Any serialized Fd will be `dup()`-ed in the process
pub fn write_to_buffers(
    msg: &Message<u32, RawFd>,
    payload: &mut [u8],
    fds: &mut Vec<OwnedFd>,
) -> Result<usize, MessageWriteError> {
    let orig_payload_len = payload.len();
    // Helper function to write a u32 or a RawFd to its buffer
    fn write_buf(u: u32, payload: &mut [u8]) -> Result<&mut [u8], MessageWriteError> {
        if payload.len() >= 4 {
            let (head, tail) = payload.split_at_mut(4);
            head.copy_from_slice(&u.to_ne_bytes());
            Ok(tail)
        } else {
            Err(MessageWriteError::BufferTooSmall)
        }
    }

    // Helper function to write byte arrays in payload
    fn write_array_to_payload<'a>(
        array: &[u8],
        payload: &'a mut [u8],
    ) -> Result<&'a mut [u8], MessageWriteError> {
        // size header
        let payload = write_buf(array.len() as u32, payload)?;

        // Handle padding
        let len = next_multiple_of(array.len(), 4);

        if payload.len() < len {
            return Err(MessageWriteError::BufferTooSmall);
        }

        let (buffer_slice, rest) = payload.split_at_mut(len);
        buffer_slice[..array.len()].copy_from_slice(array);
        Ok(rest)
    }

    let free_size = payload.len();
    if free_size < 2 * 4 {
        return Err(MessageWriteError::BufferTooSmall);
    }

    let (header, mut payload) = payload.split_at_mut(2 * 4);

    // write the contents in the buffer
    for arg in &msg.args {
        payload = match *arg {
            Argument::Int(i) => write_buf(i as u32, payload)?,
            Argument::Uint(u) => write_buf(u, payload)?,
            Argument::Fixed(f) => write_buf(f as u32, payload)?,
            Argument::Str(Some(ref s)) => write_array_to_payload(s.as_bytes_with_nul(), payload)?,
            Argument::Str(None) => write_array_to_payload(&[], payload)?,
            Argument::Object(o) => write_buf(o, payload)?,
            Argument::NewId(n) => write_buf(n, payload)?,
            Argument::Array(ref a) => write_array_to_payload(a, payload)?,
            Argument::Fd(fd) => {
                let dup_fd = unsafe { BorrowedFd::borrow_raw(fd) }
                    .try_clone_to_owned()
                    .map_err(MessageWriteError::DupFdFailed)?;
                fds.push(dup_fd);
                payload
            }
        };
    }

    let wrote_size = free_size - payload.len();
    header[..4].copy_from_slice(&msg.sender_id.to_ne_bytes());
    header[4..]
        .copy_from_slice(&(((wrote_size as u32) << 16) | u32::from(msg.opcode)).to_ne_bytes());
    Ok(orig_payload_len - payload.len())
}

/// Attempts to parse a single wayland message with the given signature.
///
/// If the buffers contains several messages, only the first one will be parsed,
/// and the unused tail of the buffers is returned. If a single message was present,
/// the returned slices should thus be empty.
///
/// Errors if the message is malformed.
#[allow(clippy::type_complexity)]
pub fn parse_message<'a>(
    raw: &'a [u8],
    signature: &[ArgumentType],
    fds: &mut VecDeque<OwnedFd>,
) -> Result<(Message<u32, OwnedFd>, &'a [u8]), MessageParseError> {
    // helper function to read arrays
    fn read_array_from_payload(
        array_len: usize,
        payload: &[u8],
    ) -> Result<(&[u8], &[u8]), MessageParseError> {
        let len = next_multiple_of(array_len, 4);
        if len > payload.len() {
            return Err(MessageParseError::MissingData);
        }
        Ok((&payload[..array_len], &payload[len..]))
    }

    if raw.len() < 2 * 4 {
        return Err(MessageParseError::MissingData);
    }

    let sender_id = u32::from_ne_bytes([raw[0], raw[1], raw[2], raw[3]]);
    let word_2 = u32::from_ne_bytes([raw[4], raw[5], raw[6], raw[7]]);
    let opcode = (word_2 & 0x0000_FFFF) as u16;
    let len = (word_2 >> 16) as usize;

    if len < 2 * 4 {
        return Err(MessageParseError::Malformed);
    } else if len > raw.len() {
        return Err(MessageParseError::MissingData);
    }

    let fd_len = signature.iter().filter(|x| matches!(x, ArgumentType::Fd)).count();
    if fd_len > fds.len() {
        return Err(MessageParseError::MissingFD);
    }

    let (mut payload, rest) = raw.split_at(len);
    payload = &payload[2 * 4..];

    let arguments = signature
        .iter()
        .map(|argtype| {
            if let ArgumentType::Fd = *argtype {
                // don't consume input but fd
                if let Some(front) = fds.pop_front() {
                    Ok(Argument::Fd(front))
                } else {
                    Err(MessageParseError::MissingFD)
                }
            } else if payload.len() >= 4 {
                let (front, mut tail) = payload.split_at(4);
                let front = u32::from_ne_bytes(front.try_into().unwrap());
                let arg = match *argtype {
                    ArgumentType::Int => Ok(Argument::Int(front as i32)),
                    ArgumentType::Uint => Ok(Argument::Uint(front)),
                    ArgumentType::Fixed => Ok(Argument::Fixed(front as i32)),
                    ArgumentType::Str(_) => {
                        read_array_from_payload(front as usize, tail).and_then(|(v, rest)| {
                            tail = rest;
                            if !v.is_empty() {
                                match CStr::from_bytes_with_nul(v) {
                                    Ok(s) => Ok(Argument::Str(Some(Box::new(s.into())))),
                                    Err(_) => Err(MessageParseError::Malformed),
                                }
                            } else {
                                Ok(Argument::Str(None))
                            }
                        })
                    }
                    ArgumentType::Object(_) => Ok(Argument::Object(front)),
                    ArgumentType::NewId => Ok(Argument::NewId(front)),
                    ArgumentType::Array => {
                        read_array_from_payload(front as usize, tail).map(|(v, rest)| {
                            tail = rest;
                            Argument::Array(Box::new(v.into()))
                        })
                    }
                    ArgumentType::Fd => unreachable!(),
                };
                payload = tail;
                arg
            } else {
                Err(MessageParseError::MissingData)
            }
        })
        .collect::<Result<SmallVec<_>, MessageParseError>>()?;

    let msg = Message { sender_id, opcode, args: arguments };
    Ok((msg, rest))
}

// Stabalized in Rust 1.73
fn next_multiple_of(lhs: usize, rhs: usize) -> usize {
    match lhs % rhs {
        0 => lhs,
        r => lhs + (rhs - r),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::AllowNull;
    use smallvec::smallvec;
    use std::{ffi::CString, os::unix::io::IntoRawFd};

    #[test]
    fn into_from_raw_cycle() {
        let mut bytes_buffer = vec![0; 1024];
        let mut fd_buffer = Vec::new();

        let msg = Message {
            sender_id: 42,
            opcode: 7,
            args: smallvec![
                Argument::Uint(3),
                Argument::Fixed(-89),
                Argument::Str(Some(Box::new(CString::new(&b"I like trains!"[..]).unwrap()))),
                Argument::Array(vec![1, 2, 3, 4, 5, 6, 7, 8, 9].into()),
                Argument::Object(88),
                Argument::NewId(56),
                Argument::Int(-25),
            ],
        };
        // write the message to the buffers
        write_to_buffers(&msg, &mut bytes_buffer[..], &mut fd_buffer).unwrap();
        // read them back
        let mut fd_buffer = VecDeque::from(fd_buffer);
        let (rebuilt, _) = parse_message(
            &bytes_buffer[..],
            &[
                ArgumentType::Uint,
                ArgumentType::Fixed,
                ArgumentType::Str(AllowNull::No),
                ArgumentType::Array,
                ArgumentType::Object(AllowNull::No),
                ArgumentType::NewId,
                ArgumentType::Int,
            ],
            &mut fd_buffer,
        )
        .unwrap();
        assert_eq!(rebuilt.map_fd(IntoRawFd::into_raw_fd), msg);
    }
}
