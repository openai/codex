use super::encoder::EncoderWriter;
use crate::engine::Engine;
use std::io;

/// A `Write` implementation that base64-encodes data using the provided config and accumulates the
/// resulting base64 utf8 `&str` in a [StrConsumer] implementation (typically `String`), which is
/// then exposed via `into_inner()`.
///
/// # Examples
///
/// Buffer base64 in a new String:
///
/// ```
/// use std::io::Write;
/// use base64::engine::general_purpose;
///
/// let mut enc = base64::write::EncoderStringWriter::new(&general_purpose::STANDARD);
///
/// enc.write_all(b"asdf").unwrap();
///
/// // get the resulting String
/// let b64_string = enc.into_inner();
///
/// assert_eq!("YXNkZg==", &b64_string);
/// ```
///
/// Or, append to an existing `String`, which implements `StrConsumer`:
///
/// ```
/// use std::io::Write;
/// use base64::engine::general_purpose;
///
/// let mut buf = String::from("base64: ");
///
/// let mut enc = base64::write::EncoderStringWriter::from_consumer(
///     &mut buf,
///     &general_purpose::STANDARD);
///
/// enc.write_all(b"asdf").unwrap();
///
/// // release the &mut reference on buf
/// let _ = enc.into_inner();
///
/// assert_eq!("base64: YXNkZg==", &buf);
/// ```
///
/// # Performance
///
/// Because it has to validate that the base64 is UTF-8, it is about 80% as fast as writing plain
/// bytes to a `io::Write`.
pub struct EncoderStringWriter<'e, E: Engine, S: StrConsumer> {
    encoder: EncoderWriter<'e, E, Utf8SingleCodeUnitWriter<S>>,
}

impl<'e, E: Engine, S: StrConsumer> EncoderStringWriter<'e, E, S> {
    /// Create a EncoderStringWriter that will append to the provided `StrConsumer`.
    pub fn from_consumer(str_consumer: S, engine: &'e E) -> Self {
        EncoderStringWriter {
            encoder: EncoderWriter::new(Utf8SingleCodeUnitWriter { str_consumer }, engine),
        }
    }

    /// Encode all remaining buffered data, including any trailing incomplete input triples and
    /// associated padding.
    ///
    /// Returns the base64-encoded form of the accumulated written data.
    pub fn into_inner(mut self) -> S {
        self.encoder
            .finish()
            .expect("Writing to a consumer should never fail")
            .str_consumer
    }
}

impl<'e, E: Engine> EncoderStringWriter<'e, E, String> {
    /// Create a EncoderStringWriter that will encode into a new `String` with the provided config.
    pub fn new(engine: &'e E) -> Self {
        EncoderStringWriter::from_consumer(String::new(), engine)
    }
}

impl<'e, E: Engine, S: StrConsumer> io::Write for EncoderStringWriter<'e, E, S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.encoder.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.encoder.flush()
    }
}

/// An abstraction around consuming `str`s produced by base64 encoding.
pub trait StrConsumer {
    /// Consume the base64 encoded data in `buf`
    fn consume(&mut self, buf: &str);
}

/// As for io::Write, `StrConsumer` is implemented automatically for `&mut S`.
impl<S: StrConsumer + ?Sized> StrConsumer for &mut S {
    fn consume(&mut self, buf: &str) {
        (**self).consume(buf);
    }
}

/// Pushes the str onto the end of the String
impl StrConsumer for String {
    fn consume(&mut self, buf: &str) {
        self.push_str(buf);
    }
}

/// A `Write` that only can handle bytes that are valid single-byte UTF-8 code units.
///
/// This is safe because we only use it when writing base64, which is always valid UTF-8.
struct Utf8SingleCodeUnitWriter<S: StrConsumer> {
    str_consumer: S,
}

impl<S: StrConsumer> io::Write for Utf8SingleCodeUnitWriter<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Because we expect all input to be valid utf-8 individual bytes, we can encode any buffer
        // length
        let s = std::str::from_utf8(buf).expect("Input must be valid UTF-8");

        self.str_consumer.consume(s);

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        // no op
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        engine::Engine, tests::random_engine, write::encoder_string_writer::EncoderStringWriter,
    };
    use rand::Rng;
    use std::cmp;
    use std::io::Write;

    #[test]
    fn every_possible_split_of_input() {
        let mut rng = rand::thread_rng();
        let mut orig_data = Vec::<u8>::new();
        let mut normal_encoded = String::new();

        let size = 5_000;

        for i in 0..size {
            orig_data.clear();
            normal_encoded.clear();

            orig_data.resize(size, 0);
            rng.fill(&mut orig_data[..]);

            let engine = random_engine(&mut rng);
            engine.encode_string(&orig_data, &mut normal_encoded);

            let mut stream_encoder = EncoderStringWriter::new(&engine);
            // Write the first i bytes, then the rest
            stream_encoder.write_all(&orig_data[0..i]).unwrap();
            stream_encoder.write_all(&orig_data[i..]).unwrap();

            let stream_encoded = stream_encoder.into_inner();

            assert_eq!(normal_encoded, stream_encoded);
        }
    }
    #[test]
    fn incremental_writes() {
        let mut rng = rand::thread_rng();
        let mut orig_data = Vec::<u8>::new();
        let mut normal_encoded = String::new();

        let size = 5_000;

        for _ in 0..size {
            orig_data.clear();
            normal_encoded.clear();

            orig_data.resize(size, 0);
            rng.fill(&mut orig_data[..]);

            let engine = random_engine(&mut rng);
            engine.encode_string(&orig_data, &mut normal_encoded);

            let mut stream_encoder = EncoderStringWriter::new(&engine);
            // write small nibbles of data
            let mut offset = 0;
            while offset < size {
                let nibble_size = cmp::min(rng.gen_range(0..=64), size - offset);
                let len = stream_encoder
                    .write(&orig_data[offset..offset + nibble_size])
                    .unwrap();
                offset += len;
            }

            let stream_encoded = stream_encoder.into_inner();

            assert_eq!(normal_encoded, stream_encoded);
        }
    }
}
