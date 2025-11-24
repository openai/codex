//! A module for all encoding needs.
use crate::error::{BufferResult, LzwError, LzwStatus, VectorResult};
use crate::{BitOrder, Code, StreamBuf, MAX_CODESIZE, MAX_ENTRIES, STREAM_BUF_SIZE};

use crate::alloc::{boxed::Box, vec::Vec};
#[cfg(feature = "std")]
use crate::error::StreamResult;
#[cfg(feature = "std")]
use std::io::{self, BufRead, Write};

/// The state for encoding data with an LZW algorithm.
///
/// The same structure can be utilized with streams as well as your own buffers and driver logic.
/// It may even be possible to mix them if you are sufficiently careful not to lose any written
/// data in the process.
///
/// This is a sans-IO implementation, meaning that it only contains the state of the encoder and
/// the caller will provide buffers for input and output data when calling the basic
/// [`encode_bytes`] method. Nevertheless, a number of _adapters_ are provided in the `into_*`
/// methods for enoding with a particular style of common IO.
///
/// * [`encode`] for encoding once without any IO-loop.
/// * [`into_async`] for encoding with the `futures` traits for asynchronous IO.
/// * [`into_stream`] for encoding with the standard `io` traits.
/// * [`into_vec`] for in-memory encoding.
///
/// [`encode_bytes`]: #method.encode_bytes
/// [`encode`]: #method.encode
/// [`into_async`]: #method.into_async
/// [`into_stream`]: #method.into_stream
/// [`into_vec`]: #method.into_vec
pub struct Encoder {
    /// Internally dispatch via a dynamic trait object. This did not have any significant
    /// performance impact as we batch data internally and this pointer does not change after
    /// creation!
    state: Box<dyn Stateful + Send + 'static>,
}

/// A encoding stream sink.
///
/// See [`Encoder::into_stream`] on how to create this type.
///
/// [`Encoder::into_stream`]: struct.Encoder.html#method.into_stream
#[cfg_attr(
    not(feature = "std"),
    deprecated = "This type is only useful with the `std` feature."
)]
#[cfg_attr(not(feature = "std"), allow(dead_code))]
pub struct IntoStream<'d, W> {
    encoder: &'d mut Encoder,
    writer: W,
    buffer: Option<StreamBuf<'d>>,
    default_size: usize,
}

/// An async decoding sink.
///
/// See [`Encoder::into_async`] on how to create this type.
///
/// [`Encoder::into_async`]: struct.Encoder.html#method.into_async
#[cfg(feature = "async")]
pub struct IntoAsync<'d, W> {
    encoder: &'d mut Encoder,
    writer: W,
    buffer: Option<StreamBuf<'d>>,
    default_size: usize,
}

/// A encoding sink into a vector.
///
/// See [`Encoder::into_vec`] on how to create this type.
///
/// [`Encoder::into_vec`]: struct.Encoder.html#method.into_vec
pub struct IntoVec<'d> {
    encoder: &'d mut Encoder,
    vector: &'d mut Vec<u8>,
}

trait Stateful {
    fn advance(&mut self, inp: &[u8], out: &mut [u8]) -> BufferResult;
    fn mark_ended(&mut self) -> bool;
    /// Reset the state tracking if end code has been written.
    fn restart(&mut self);
    /// Reset the encoder to the beginning, dropping all buffers etc.
    fn reset(&mut self);
}

struct EncodeState<B: Buffer> {
    /// The configured minimal code size.
    min_size: u8,
    /// The current encoding symbol tree.
    tree: Tree,
    /// If we have pushed the end code.
    has_ended: bool,
    /// If tiff then bumps are a single code sooner.
    is_tiff: bool,
    /// The code corresponding to the currently read characters.
    current_code: Code,
    /// The clear code for resetting the dictionary.
    clear_code: Code,
    /// The bit buffer for encoding.
    buffer: B,
}

struct MsbBuffer {
    /// The current code length.
    code_size: u8,
    /// The buffer bits.
    buffer: u64,
    /// The number of valid buffer bits.
    bits_in_buffer: u8,
}

struct LsbBuffer {
    /// The current code length.
    code_size: u8,
    /// The buffer bits.
    buffer: u64,
    /// The number of valid buffer bits.
    bits_in_buffer: u8,
}

trait Buffer {
    fn new(size: u8) -> Self;
    /// Reset the code size in the buffer.
    fn reset(&mut self, min_size: u8);
    /// Apply effects of a Clear Code.
    fn clear(&mut self, min_size: u8);
    /// Insert a code into the buffer.
    fn buffer_code(&mut self, code: Code);
    /// Push bytes if the buffer space is getting small.
    fn push_out(&mut self, out: &mut &mut [u8]) -> bool;
    /// Flush all full bytes, returning if at least one more byte remains.
    fn flush_out(&mut self, out: &mut &mut [u8]) -> bool;
    /// Pad the buffer to a full byte.
    fn buffer_pad(&mut self);
    /// Increase the maximum code size.
    fn bump_code_size(&mut self);
    /// Return the maximum code with the current code size.
    fn max_code(&self) -> Code;
    /// Return the current code size in bits.
    fn code_size(&self) -> u8;
}

/// One tree node for at most each code.
/// To avoid using too much memory we keep nodes with few successors in optimized form. This form
/// doesn't offer lookup by indexing but instead does a linear search.
#[derive(Default)]
struct Tree {
    simples: Vec<Simple>,
    complex: Vec<Full>,
    keys: Vec<CompressedKey>,
}

#[derive(Clone, Copy)]
enum FullKey {
    NoSuccessor,
    Simple(u16),
    Full(u16),
}

#[derive(Clone, Copy)]
struct CompressedKey(u16);

const SHORT: usize = 16;

#[derive(Clone, Copy)]
struct Simple {
    codes: [Code; SHORT],
    chars: [u8; SHORT],
    count: u8,
}

#[derive(Clone, Copy)]
struct Full {
    char_continuation: [Code; 256],
}

/// Describes the static parameters for creating a decoder.
#[derive(Clone, Debug)]
pub struct Configuration {
    order: BitOrder,
    size: u8,
    tiff: bool,
}

impl Configuration {
    /// Create a configuration to decode with the specified bit order and symbol size.
    ///
    /// # Panics
    ///
    /// The `size` needs to be in the interval `2..=12`.
    pub fn new(order: BitOrder, size: u8) -> Self {
        super::assert_encode_size(size);
        Configuration {
            order,
            size,
            tiff: false,
        }
    }

    /// Create a configuration for a TIFF compatible decoder.
    ///
    /// # Panics
    ///
    /// The `size` needs to be in the interval `2..=12`.
    pub fn with_tiff_size_switch(order: BitOrder, size: u8) -> Self {
        super::assert_encode_size(size);
        Configuration {
            order,
            size,
            tiff: true,
        }
    }

    /// Create a new decoder with the define configuration.
    pub fn build(self) -> Encoder {
        Encoder {
            state: Encoder::from_configuration(&self),
        }
    }
}

impl Encoder {
    /// Create a new encoder with the specified bit order and symbol size.
    ///
    /// The algorithm for dynamically increasing the code symbol bit width is compatible with the
    /// original specification. In particular you will need to specify an `Lsb` bit oder to encode
    /// the data portion of a compressed `gif` image.
    ///
    /// # Panics
    ///
    /// The `size` needs to be in the interval `2..=12`.
    pub fn new(order: BitOrder, size: u8) -> Self {
        Configuration::new(order, size).build()
    }

    /// Create a TIFF compatible encoder with the specified bit order and symbol size.
    ///
    /// The algorithm for dynamically increasing the code symbol bit width is compatible with the
    /// TIFF specification, which is a misinterpretation of the original algorithm for increasing
    /// the code size. It switches one symbol sooner.
    ///
    /// # Panics
    ///
    /// The `size` needs to be in the interval `2..=12`.
    pub fn with_tiff_size_switch(order: BitOrder, size: u8) -> Self {
        Configuration::with_tiff_size_switch(order, size).build()
    }

    fn from_configuration(cfg: &Configuration) -> Box<dyn Stateful + Send + 'static> {
        match cfg.order {
            BitOrder::Lsb => {
                let mut state = EncodeState::<LsbBuffer>::new(cfg.size);
                state.is_tiff = cfg.tiff;
                Box::new(state)
            }
            BitOrder::Msb => {
                let mut state = EncodeState::<MsbBuffer>::new(cfg.size);
                state.is_tiff = cfg.tiff;
                Box::new(state)
            }
        }
    }

    /// Encode some bytes from `inp` into `out`.
    ///
    /// See [`into_stream`] for high-level functions (this interface is only available with the
    /// `std` feature) and [`finish`] for marking the input data as complete.
    ///
    /// When some input byte is invalid, i.e. is not smaller than `1 << size`, then that byte and
    /// all following ones will _not_ be consumed and the `status` of the result will signal an
    /// error. The result will also indicate that all bytes up to but not including the offending
    /// byte have been consumed. You may try again with a fixed byte.
    ///
    /// [`into_stream`]: #method.into_stream
    /// [`finish`]: #method.finish
    pub fn encode_bytes(&mut self, inp: &[u8], out: &mut [u8]) -> BufferResult {
        self.state.advance(inp, out)
    }

    /// Encode a single chunk of data.
    ///
    /// This method will add an end marker to the encoded chunk.
    ///
    /// This is a convenience wrapper around [`into_vec`]. Use the `into_vec` adapter to customize
    /// buffer size, to supply an existing vector, to control whether an end marker is required, or
    /// to preserve partial data in the case of a decoding error.
    ///
    /// [`into_vec`]: #into_vec
    ///
    /// # Example
    ///
    /// ```
    /// use weezl::{BitOrder, encode::Encoder};
    ///
    /// let data = b"Hello, world";
    /// let encoded = Encoder::new(BitOrder::Msb, 9)
    ///     .encode(data)
    ///     .expect("All bytes valid for code size");
    /// ```
    pub fn encode(&mut self, data: &[u8]) -> Result<Vec<u8>, LzwError> {
        let mut output = Vec::new();
        self.into_vec(&mut output).encode_all(data).status?;
        Ok(output)
    }

    /// Construct a encoder into a writer.
    #[cfg(feature = "std")]
    pub fn into_stream<W: Write>(&mut self, writer: W) -> IntoStream<'_, W> {
        IntoStream {
            encoder: self,
            writer,
            buffer: None,
            default_size: STREAM_BUF_SIZE,
        }
    }

    /// Construct a encoder into an async writer.
    #[cfg(feature = "async")]
    pub fn into_async<W: futures::io::AsyncWrite>(&mut self, writer: W) -> IntoAsync<'_, W> {
        IntoAsync {
            encoder: self,
            writer,
            buffer: None,
            default_size: STREAM_BUF_SIZE,
        }
    }

    /// Construct an encoder into a vector.
    ///
    /// All encoded data is appended and the vector is __not__ cleared.
    ///
    /// Compared to `into_stream` this interface allows a high-level access to encoding without
    /// requires the `std`-feature. Also, it can make full use of the extra buffer control that the
    /// special target exposes.
    pub fn into_vec<'lt>(&'lt mut self, vec: &'lt mut Vec<u8>) -> IntoVec<'lt> {
        IntoVec {
            encoder: self,
            vector: vec,
        }
    }

    /// Mark the encoding as in the process of finishing.
    ///
    /// The next following call to `encode_bytes` which is able to consume the complete input will
    /// also try to emit an end code. It's not recommended, but also not unsound, to use different
    /// byte slices in different calls from this point forward and thus to 'delay' the actual end
    /// of the data stream. The behaviour after the end marker has been written is unspecified but
    /// sound.
    pub fn finish(&mut self) {
        self.state.mark_ended();
    }

    /// Undo marking this data stream as ending.
    /// FIXME: clarify how this interacts with padding introduced after end code.
    #[allow(dead_code)]
    pub(crate) fn restart(&mut self) {
        self.state.restart()
    }

    /// Reset all internal state.
    ///
    /// This produce an encoder as if just constructed with `new` but taking slightly less work. In
    /// particular it will not deallocate any internal allocations. It will also avoid some
    /// duplicate setup work.
    pub fn reset(&mut self) {
        self.state.reset()
    }
}

#[cfg(feature = "std")]
impl<'d, W: Write> IntoStream<'d, W> {
    /// Encode data from a reader.
    ///
    /// This will drain the supplied reader. It will not encode an end marker after all data has
    /// been processed.
    pub fn encode(&mut self, read: impl BufRead) -> StreamResult {
        self.encode_part(read, false)
    }

    /// Encode data from a reader and an end marker.
    pub fn encode_all(mut self, read: impl BufRead) -> StreamResult {
        self.encode_part(read, true)
    }

    /// Set the size of the intermediate encode buffer.
    ///
    /// A buffer of this size is allocated to hold one part of the encoded stream when no buffer is
    /// available and any encoding method is called. No buffer is allocated if `set_buffer` has
    /// been called. The buffer is reused.
    ///
    /// # Panics
    /// This method panics if `size` is `0`.
    pub fn set_buffer_size(&mut self, size: usize) {
        assert_ne!(size, 0, "Attempted to set empty buffer");
        self.default_size = size;
    }

    /// Use a particular buffer as an intermediate encode buffer.
    ///
    /// Calling this sets or replaces the buffer. When a buffer has been set then it is used
    /// instead of a dynamically allocating a buffer. Note that the size of the buffer is relevant
    /// for efficient encoding as there is additional overhead from `write` calls each time the
    /// buffer has been filled.
    ///
    /// # Panics
    /// This method panics if the `buffer` is empty.
    pub fn set_buffer(&mut self, buffer: &'d mut [u8]) {
        assert_ne!(buffer.len(), 0, "Attempted to set empty buffer");
        self.buffer = Some(StreamBuf::Borrowed(buffer));
    }

    fn encode_part(&mut self, mut read: impl BufRead, finish: bool) -> StreamResult {
        let IntoStream {
            encoder,
            writer,
            buffer,
            default_size,
        } = self;
        enum Progress {
            Ok,
            Done,
        }

        let mut bytes_read = 0;
        let mut bytes_written = 0;

        let read_bytes = &mut bytes_read;
        let write_bytes = &mut bytes_written;

        let outbuf: &mut [u8] =
            match { buffer.get_or_insert_with(|| StreamBuf::Owned(vec![0u8; *default_size])) } {
                StreamBuf::Borrowed(slice) => &mut *slice,
                StreamBuf::Owned(vec) => &mut *vec,
            };
        assert!(!outbuf.is_empty());

        let once = move || {
            let data = read.fill_buf()?;

            if data.is_empty() {
                if finish {
                    encoder.finish();
                } else {
                    return Ok(Progress::Done);
                }
            }

            let result = encoder.encode_bytes(data, &mut outbuf[..]);
            *read_bytes += result.consumed_in;
            *write_bytes += result.consumed_out;
            read.consume(result.consumed_in);

            let done = result.status.map_err(|err| {
                io::Error::new(io::ErrorKind::InvalidData, &*format!("{:?}", err))
            })?;

            if let LzwStatus::Done = done {
                writer.write_all(&outbuf[..result.consumed_out])?;
                return Ok(Progress::Done);
            }

            if let LzwStatus::NoProgress = done {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "No more data but no end marker detected",
                ));
            }

            writer.write_all(&outbuf[..result.consumed_out])?;
            Ok(Progress::Ok)
        };

        let status = core::iter::repeat_with(once)
            // scan+fuse can be replaced with map_while
            .scan((), |(), result| match result {
                Ok(Progress::Ok) => Some(Ok(())),
                Err(err) => Some(Err(err)),
                Ok(Progress::Done) => None,
            })
            .fuse()
            .collect();

        StreamResult {
            bytes_read,
            bytes_written,
            status,
        }
    }
}

impl IntoVec<'_> {
    /// Encode data from a slice.
    pub fn encode(&mut self, read: &[u8]) -> VectorResult {
        self.encode_part(read, false)
    }

    /// Decode data from a reader, adding an end marker.
    pub fn encode_all(mut self, read: &[u8]) -> VectorResult {
        self.encode_part(read, true)
    }

    fn grab_buffer(&mut self) -> (&mut [u8], &mut Encoder) {
        const CHUNK_SIZE: usize = 1 << 12;
        let decoder = &mut self.encoder;
        let length = self.vector.len();

        // Use the vector to do overflow checks and w/e.
        self.vector.reserve(CHUNK_SIZE);
        // FIXME: encoding into uninit buffer?
        self.vector.resize(length + CHUNK_SIZE, 0u8);

        (&mut self.vector[length..], decoder)
    }

    fn encode_part(&mut self, part: &[u8], finish: bool) -> VectorResult {
        let mut result = VectorResult {
            consumed_in: 0,
            consumed_out: 0,
            status: Ok(LzwStatus::Ok),
        };

        enum Progress {
            Ok,
            Done,
        }

        // Converting to mutable refs to move into the `once` closure.
        let read_bytes = &mut result.consumed_in;
        let write_bytes = &mut result.consumed_out;
        let mut data = part;

        // A 64 MB buffer is quite large but should get alloc_zeroed.
        // Note that the decoded size can be up to quadratic in code block.
        let once = move || {
            // Grab a new output buffer.
            let (outbuf, encoder) = self.grab_buffer();

            if finish {
                encoder.finish();
            }

            // Decode as much of the buffer as fits.
            let result = encoder.encode_bytes(data, &mut outbuf[..]);
            // Do the bookkeeping and consume the buffer.
            *read_bytes += result.consumed_in;
            *write_bytes += result.consumed_out;
            data = &data[result.consumed_in..];

            let unfilled = outbuf.len() - result.consumed_out;
            let filled = self.vector.len() - unfilled;
            self.vector.truncate(filled);

            // Handle the status in the result.
            let done = result.status?;
            if let LzwStatus::Done = done {
                Ok(Progress::Done)
            } else {
                Ok(Progress::Ok)
            }
        };

        // Decode chunks of input data until we're done.
        let status: Result<(), _> = core::iter::repeat_with(once)
            // scan+fuse can be replaced with map_while
            .scan((), |(), result| match result {
                Ok(Progress::Ok) => Some(Ok(())),
                Err(err) => Some(Err(err)),
                Ok(Progress::Done) => None,
            })
            .fuse()
            .collect();

        if let Err(err) = status {
            result.status = Err(err);
        }

        result
    }
}

// This is implemented in a separate file, so that 1.34.2 does not parse it. Otherwise, it would
// trip over the usage of await, which is a reserved keyword in that edition/version. It only
// contains an impl block.
#[cfg(feature = "async")]
#[path = "encode_into_async.rs"]
mod impl_encode_into_async;

impl<B: Buffer> EncodeState<B> {
    fn new(min_size: u8) -> Self {
        let clear_code = 1 << min_size;
        let mut tree = Tree::default();
        tree.init(min_size);
        let mut state = EncodeState {
            min_size,
            tree,
            has_ended: false,
            is_tiff: false,
            current_code: clear_code,
            clear_code,
            buffer: B::new(min_size),
        };
        state.buffer_code(clear_code);
        state
    }
}

impl<B: Buffer> Stateful for EncodeState<B> {
    fn advance(&mut self, mut inp: &[u8], mut out: &mut [u8]) -> BufferResult {
        let c_in = inp.len();
        let c_out = out.len();
        let mut status = Ok(LzwStatus::Ok);

        'encoding: loop {
            if self.push_out(&mut out) {
                break;
            }

            if inp.is_empty() && self.has_ended {
                let end = self.end_code();
                if self.current_code != end {
                    if self.current_code != self.clear_code {
                        self.buffer_code(self.current_code);

                        // When reading this code, the decoder will add an extra entry to its table
                        // before reading th end code. Thusly, it may increase its code size based
                        // on this additional entry.
                        if self.tree.keys.len() + usize::from(self.is_tiff)
                            > usize::from(self.buffer.max_code())
                            && self.buffer.code_size() < MAX_CODESIZE
                        {
                            self.buffer.bump_code_size();
                        }
                    }
                    self.buffer_code(end);
                    self.current_code = end;
                    self.buffer_pad();
                }

                break;
            }

            let mut next_code = None;
            let mut bytes = inp.iter();
            while let Some(&byte) = bytes.next() {
                if self.min_size < 8 && byte >= 1 << self.min_size {
                    status = Err(LzwError::InvalidCode);
                    break 'encoding;
                }

                inp = bytes.as_slice();
                match self.tree.iterate(self.current_code, byte) {
                    Ok(code) => self.current_code = code,
                    Err(_) => {
                        next_code = Some(self.current_code);

                        self.current_code = u16::from(byte);
                        break;
                    }
                }
            }

            match next_code {
                // No more bytes, no code produced.
                None => break,
                Some(code) => {
                    self.buffer_code(code);

                    if self.tree.keys.len() + usize::from(self.is_tiff)
                        > usize::from(self.buffer.max_code()) + 1
                        && self.buffer.code_size() < MAX_CODESIZE
                    {
                        self.buffer.bump_code_size();
                    }

                    if self.tree.keys.len() > MAX_ENTRIES {
                        self.buffer_code(self.clear_code);
                        self.tree.reset(self.min_size);
                        self.buffer.clear(self.min_size);
                    }
                }
            }
        }

        if inp.is_empty() && self.current_code == self.end_code() {
            if !self.flush_out(&mut out) {
                status = Ok(LzwStatus::Done);
            }
        }

        BufferResult {
            consumed_in: c_in - inp.len(),
            consumed_out: c_out - out.len(),
            status,
        }
    }

    fn mark_ended(&mut self) -> bool {
        core::mem::replace(&mut self.has_ended, true)
    }

    fn restart(&mut self) {
        self.has_ended = false;
    }

    fn reset(&mut self) {
        self.restart();
        self.current_code = self.clear_code;
        self.tree.reset(self.min_size);
        self.buffer.reset(self.min_size);
        self.buffer_code(self.clear_code);
    }
}

impl<B: Buffer> EncodeState<B> {
    fn push_out(&mut self, out: &mut &mut [u8]) -> bool {
        self.buffer.push_out(out)
    }

    fn flush_out(&mut self, out: &mut &mut [u8]) -> bool {
        self.buffer.flush_out(out)
    }

    fn end_code(&self) -> Code {
        self.clear_code + 1
    }

    fn buffer_pad(&mut self) {
        self.buffer.buffer_pad();
    }

    fn buffer_code(&mut self, code: Code) {
        self.buffer.buffer_code(code);
    }
}

impl Buffer for MsbBuffer {
    fn new(min_size: u8) -> Self {
        MsbBuffer {
            code_size: min_size + 1,
            buffer: 0,
            bits_in_buffer: 0,
        }
    }

    fn reset(&mut self, min_size: u8) {
        self.code_size = min_size + 1;
        self.buffer = 0;
        self.bits_in_buffer = 0;
    }

    fn clear(&mut self, min_size: u8) {
        self.code_size = min_size + 1;
    }

    fn buffer_code(&mut self, code: Code) {
        let shift = 64 - self.bits_in_buffer - self.code_size;
        self.buffer |= u64::from(code) << shift;
        self.bits_in_buffer += self.code_size;
    }

    fn push_out(&mut self, out: &mut &mut [u8]) -> bool {
        if self.bits_in_buffer + 2 * self.code_size < 64 {
            return false;
        }

        self.flush_out(out)
    }

    fn flush_out(&mut self, out: &mut &mut [u8]) -> bool {
        let want = usize::from(self.bits_in_buffer / 8);
        let count = want.min((*out).len());
        let (bytes, tail) = core::mem::replace(out, &mut []).split_at_mut(count);
        *out = tail;

        for b in bytes {
            *b = ((self.buffer & 0xff00_0000_0000_0000) >> 56) as u8;
            self.buffer <<= 8;
            self.bits_in_buffer -= 8;
        }

        count < want
    }

    fn buffer_pad(&mut self) {
        let to_byte = self.bits_in_buffer.wrapping_neg() & 0x7;
        self.bits_in_buffer += to_byte;
    }

    fn bump_code_size(&mut self) {
        self.code_size += 1;
    }

    fn max_code(&self) -> Code {
        (1 << self.code_size) - 1
    }

    fn code_size(&self) -> u8 {
        self.code_size
    }
}

impl Buffer for LsbBuffer {
    fn new(min_size: u8) -> Self {
        LsbBuffer {
            code_size: min_size + 1,
            buffer: 0,
            bits_in_buffer: 0,
        }
    }

    fn reset(&mut self, min_size: u8) {
        self.code_size = min_size + 1;
        self.buffer = 0;
        self.bits_in_buffer = 0;
    }

    fn clear(&mut self, min_size: u8) {
        self.code_size = min_size + 1;
    }

    fn buffer_code(&mut self, code: Code) {
        self.buffer |= u64::from(code) << self.bits_in_buffer;
        self.bits_in_buffer += self.code_size;
    }

    fn push_out(&mut self, out: &mut &mut [u8]) -> bool {
        if self.bits_in_buffer + 2 * self.code_size < 64 {
            return false;
        }

        self.flush_out(out)
    }

    fn flush_out(&mut self, out: &mut &mut [u8]) -> bool {
        let want = usize::from(self.bits_in_buffer / 8);
        let count = want.min((*out).len());
        let (bytes, tail) = core::mem::replace(out, &mut []).split_at_mut(count);
        *out = tail;

        for b in bytes {
            *b = (self.buffer & 0x0000_0000_0000_00ff) as u8;
            self.buffer >>= 8;
            self.bits_in_buffer -= 8;
        }

        count < want
    }

    fn buffer_pad(&mut self) {
        let to_byte = self.bits_in_buffer.wrapping_neg() & 0x7;
        self.bits_in_buffer += to_byte;
    }

    fn bump_code_size(&mut self) {
        self.code_size += 1;
    }

    fn max_code(&self) -> Code {
        (1 << self.code_size) - 1
    }

    fn code_size(&self) -> u8 {
        self.code_size
    }
}

impl Tree {
    fn init(&mut self, min_size: u8) {
        // We need a way to represent the state of a currently empty buffer. We use the clear code
        // for this, thus create one complex mapping that leads to the one-char base codes.
        self.keys
            .resize((1 << min_size) + 2, FullKey::NoSuccessor.into());
        self.complex.push(Full {
            char_continuation: [0; 256],
        });
        let map_of_begin = self.complex.last_mut().unwrap();
        for ch in 0u16..256 {
            map_of_begin.char_continuation[usize::from(ch)] = ch;
        }
        self.keys[1 << min_size] = FullKey::Full(0).into();
    }

    fn reset(&mut self, min_size: u8) {
        self.simples.clear();
        self.keys.truncate((1 << min_size) + 2);
        // Keep entry for clear code.
        self.complex.truncate(1);
        // The first complex is not changed..
        for k in self.keys[..(1 << min_size) + 2].iter_mut() {
            *k = FullKey::NoSuccessor.into();
        }
        self.keys[1 << min_size] = FullKey::Full(0).into();
    }

    fn at_key(&self, code: Code, ch: u8) -> Option<Code> {
        let key = self.keys[usize::from(code)];
        match FullKey::from(key) {
            FullKey::NoSuccessor => None,
            FullKey::Simple(idx) => {
                let nexts = &self.simples[usize::from(idx)];
                let successors = nexts
                    .codes
                    .iter()
                    .zip(nexts.chars.iter())
                    .take(usize::from(nexts.count));
                for (&scode, &sch) in successors {
                    if sch == ch {
                        return Some(scode);
                    }
                }

                None
            }
            FullKey::Full(idx) => {
                let full = &self.complex[usize::from(idx)];
                let precode = full.char_continuation[usize::from(ch)];
                if usize::from(precode) < MAX_ENTRIES {
                    Some(precode)
                } else {
                    None
                }
            }
        }
    }

    /// Iterate to the next char.
    /// Return Ok when it was already in the tree or creates a new entry for it and returns Err.
    fn iterate(&mut self, code: Code, ch: u8) -> Result<Code, Code> {
        if let Some(next) = self.at_key(code, ch) {
            Ok(next)
        } else {
            Err(self.append(code, ch))
        }
    }

    fn append(&mut self, code: Code, ch: u8) -> Code {
        let next: Code = self.keys.len() as u16;
        let key = self.keys[usize::from(code)];
        // TODO: with debug assertions, check for non-existence
        match FullKey::from(key) {
            FullKey::NoSuccessor => {
                let new_key = FullKey::Simple(self.simples.len() as u16);
                self.simples.push(Simple::default());
                let simples = self.simples.last_mut().unwrap();
                simples.codes[0] = next;
                simples.chars[0] = ch;
                simples.count = 1;
                self.keys[usize::from(code)] = new_key.into();
            }
            FullKey::Simple(idx) if usize::from(self.simples[usize::from(idx)].count) < SHORT => {
                let nexts = &mut self.simples[usize::from(idx)];
                let nidx = usize::from(nexts.count);
                nexts.chars[nidx] = ch;
                nexts.codes[nidx] = next;
                nexts.count += 1;
            }
            FullKey::Simple(idx) => {
                let new_key = FullKey::Full(self.complex.len() as u16);
                let simples = &self.simples[usize::from(idx)];
                self.complex.push(Full {
                    char_continuation: [Code::max_value(); 256],
                });
                let full = self.complex.last_mut().unwrap();
                for (&pch, &pcont) in simples.chars.iter().zip(simples.codes.iter()) {
                    full.char_continuation[usize::from(pch)] = pcont;
                }
                self.keys[usize::from(code)] = new_key.into();
            }
            FullKey::Full(idx) => {
                let full = &mut self.complex[usize::from(idx)];
                full.char_continuation[usize::from(ch)] = next;
            }
        }
        self.keys.push(FullKey::NoSuccessor.into());
        next
    }
}

impl Default for FullKey {
    fn default() -> Self {
        FullKey::NoSuccessor
    }
}

impl Default for Simple {
    fn default() -> Self {
        Simple {
            codes: [0; SHORT],
            chars: [0; SHORT],
            count: 0,
        }
    }
}

impl From<CompressedKey> for FullKey {
    fn from(CompressedKey(key): CompressedKey) -> Self {
        match (key >> MAX_CODESIZE) & 0xf {
            0 => FullKey::Full(key & 0xfff),
            1 => FullKey::Simple(key & 0xfff),
            _ => FullKey::NoSuccessor,
        }
    }
}

impl From<FullKey> for CompressedKey {
    fn from(full: FullKey) -> Self {
        CompressedKey(match full {
            FullKey::NoSuccessor => 0x2000,
            FullKey::Simple(code) => 0x1000 | code,
            FullKey::Full(code) => code,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{BitOrder, Encoder, LzwError, LzwStatus};
    use crate::alloc::vec::Vec;
    use crate::decode::Decoder;
    #[cfg(feature = "std")]
    use crate::StreamBuf;

    #[test]
    fn invalid_input_rejected() {
        const BIT_LEN: u8 = 2;
        let ref input = [0, 1 << BIT_LEN /* invalid */, 0];
        let ref mut target = [0u8; 128];
        let mut encoder = Encoder::new(BitOrder::Msb, BIT_LEN);

        encoder.finish();
        // We require simulation of normality, that is byte-for-byte compression.
        let result = encoder.encode_bytes(input, target);
        assert!(if let Err(LzwError::InvalidCode) = result.status {
            true
        } else {
            false
        });
        assert_eq!(result.consumed_in, 1);

        let fixed = encoder.encode_bytes(&[1, 0], &mut target[result.consumed_out..]);
        assert!(if let Ok(LzwStatus::Done) = fixed.status {
            true
        } else {
            false
        });
        assert_eq!(fixed.consumed_in, 2);

        // Okay, now test we actually fixed it.
        let ref mut compare = [0u8; 4];
        let mut todo = &target[..result.consumed_out + fixed.consumed_out];
        let mut free = &mut compare[..];
        let mut decoder = Decoder::new(BitOrder::Msb, BIT_LEN);

        // Decode with up to 16 rounds, far too much but inconsequential.
        for _ in 0..16 {
            if decoder.has_ended() {
                break;
            }

            let result = decoder.decode_bytes(todo, free);
            assert!(result.status.is_ok());
            todo = &todo[result.consumed_in..];
            free = &mut free[result.consumed_out..];
        }

        let remaining = { free }.len();
        let len = compare.len() - remaining;
        assert_eq!(todo, &[]);
        assert_eq!(compare[..len], [0, 1, 0]);
    }

    #[test]
    #[should_panic]
    fn invalid_code_size_low() {
        let _ = Encoder::new(BitOrder::Msb, 1);
    }

    #[test]
    #[should_panic]
    fn invalid_code_size_high() {
        let _ = Encoder::new(BitOrder::Msb, 14);
    }

    fn make_decoded() -> Vec<u8> {
        const FILE: &'static [u8] =
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.lock"));
        return Vec::from(FILE);
    }

    #[test]
    #[cfg(feature = "std")]
    fn into_stream_buffer_no_alloc() {
        let encoded = make_decoded();
        let mut encoder = Encoder::new(BitOrder::Msb, 8);

        let mut output = vec![];
        let mut buffer = [0; 512];
        let mut istream = encoder.into_stream(&mut output);
        istream.set_buffer(&mut buffer[..]);
        istream.encode(&encoded[..]).status.unwrap();

        match istream.buffer {
            Some(StreamBuf::Borrowed(_)) => {}
            None => panic!("Decoded without buffer??"),
            Some(StreamBuf::Owned(_)) => panic!("Unexpected buffer allocation"),
        }
    }

    #[test]
    #[cfg(feature = "std")]
    fn into_stream_buffer_small_alloc() {
        struct WriteTap<W: std::io::Write>(W);
        const BUF_SIZE: usize = 512;

        impl<W: std::io::Write> std::io::Write for WriteTap<W> {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                assert!(buf.len() <= BUF_SIZE);
                self.0.write(buf)
            }
            fn flush(&mut self) -> std::io::Result<()> {
                self.0.flush()
            }
        }

        let encoded = make_decoded();
        let mut encoder = Encoder::new(BitOrder::Msb, 8);

        let mut output = vec![];
        let mut istream = encoder.into_stream(WriteTap(&mut output));
        istream.set_buffer_size(512);
        istream.encode(&encoded[..]).status.unwrap();

        match istream.buffer {
            Some(StreamBuf::Owned(vec)) => assert!(vec.len() <= BUF_SIZE),
            Some(StreamBuf::Borrowed(_)) => panic!("Unexpected borrowed buffer, where from?"),
            None => panic!("Decoded without buffer??"),
        }
    }

    #[test]
    #[cfg(feature = "std")]
    fn reset() {
        let encoded = make_decoded();
        let mut encoder = Encoder::new(BitOrder::Msb, 8);
        let mut reference = None;

        for _ in 0..2 {
            let mut output = vec![];
            let mut buffer = [0; 512];
            let mut istream = encoder.into_stream(&mut output);
            istream.set_buffer(&mut buffer[..]);
            istream.encode_all(&encoded[..]).status.unwrap();

            encoder.reset();
            if let Some(reference) = &reference {
                assert_eq!(output, *reference);
            } else {
                reference = Some(output);
            }
        }
    }
}
