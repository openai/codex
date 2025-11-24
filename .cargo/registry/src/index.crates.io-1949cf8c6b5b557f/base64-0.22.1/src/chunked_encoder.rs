use crate::{
    encode::add_padding,
    engine::{Config, Engine},
};
#[cfg(any(feature = "alloc", test))]
use alloc::string::String;
#[cfg(any(feature = "alloc", test))]
use core::str;

/// The output mechanism for ChunkedEncoder's encoded bytes.
pub trait Sink {
    type Error;

    /// Handle a chunk of encoded base64 data (as UTF-8 bytes)
    fn write_encoded_bytes(&mut self, encoded: &[u8]) -> Result<(), Self::Error>;
}

/// A base64 encoder that emits encoded bytes in chunks without heap allocation.
pub struct ChunkedEncoder<'e, E: Engine + ?Sized> {
    engine: &'e E,
}

impl<'e, E: Engine + ?Sized> ChunkedEncoder<'e, E> {
    pub fn new(engine: &'e E) -> ChunkedEncoder<'e, E> {
        ChunkedEncoder { engine }
    }

    pub fn encode<S: Sink>(&self, bytes: &[u8], sink: &mut S) -> Result<(), S::Error> {
        const BUF_SIZE: usize = 1024;
        const CHUNK_SIZE: usize = BUF_SIZE / 4 * 3;

        let mut buf = [0; BUF_SIZE];
        for chunk in bytes.chunks(CHUNK_SIZE) {
            let mut len = self.engine.internal_encode(chunk, &mut buf);
            if chunk.len() != CHUNK_SIZE && self.engine.config().encode_padding() {
                // Final, potentially partial, chunk.
                // Only need to consider if padding is needed on a partial chunk since full chunk
                // is a multiple of 3, which therefore won't be padded.
                // Pad output to multiple of four bytes if required by config.
                len += add_padding(len, &mut buf[len..]);
            }
            sink.write_encoded_bytes(&buf[..len])?;
        }

        Ok(())
    }
}

// A really simple sink that just appends to a string
#[cfg(any(feature = "alloc", test))]
pub(crate) struct StringSink<'a> {
    string: &'a mut String,
}

#[cfg(any(feature = "alloc", test))]
impl<'a> StringSink<'a> {
    pub(crate) fn new(s: &mut String) -> StringSink {
        StringSink { string: s }
    }
}

#[cfg(any(feature = "alloc", test))]
impl<'a> Sink for StringSink<'a> {
    type Error = ();

    fn write_encoded_bytes(&mut self, s: &[u8]) -> Result<(), Self::Error> {
        self.string.push_str(str::from_utf8(s).unwrap());

        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use rand::{
        distributions::{Distribution, Uniform},
        Rng, SeedableRng,
    };

    use crate::{
        alphabet::STANDARD,
        engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig, PAD},
        tests::random_engine,
    };

    use super::*;

    #[test]
    fn chunked_encode_empty() {
        assert_eq!("", chunked_encode_str(&[], PAD));
    }

    #[test]
    fn chunked_encode_intermediate_fast_loop() {
        // > 8 bytes input, will enter the pretty fast loop
        assert_eq!("Zm9vYmFyYmF6cXV4", chunked_encode_str(b"foobarbazqux", PAD));
    }

    #[test]
    fn chunked_encode_fast_loop() {
        // > 32 bytes input, will enter the uber fast loop
        assert_eq!(
            "Zm9vYmFyYmF6cXV4cXV1eGNvcmdlZ3JhdWx0Z2FycGx5eg==",
            chunked_encode_str(b"foobarbazquxquuxcorgegraultgarplyz", PAD)
        );
    }

    #[test]
    fn chunked_encode_slow_loop_only() {
        // < 8 bytes input, slow loop only
        assert_eq!("Zm9vYmFy", chunked_encode_str(b"foobar", PAD));
    }

    #[test]
    fn chunked_encode_matches_normal_encode_random_string_sink() {
        let helper = StringSinkTestHelper;
        chunked_encode_matches_normal_encode_random(&helper);
    }

    pub fn chunked_encode_matches_normal_encode_random<S: SinkTestHelper>(sink_test_helper: &S) {
        let mut input_buf: Vec<u8> = Vec::new();
        let mut output_buf = String::new();
        let mut rng = rand::rngs::SmallRng::from_entropy();
        let input_len_range = Uniform::new(1, 10_000);

        for _ in 0..20_000 {
            input_buf.clear();
            output_buf.clear();

            let buf_len = input_len_range.sample(&mut rng);
            for _ in 0..buf_len {
                input_buf.push(rng.gen());
            }

            let engine = random_engine(&mut rng);

            let chunk_encoded_string = sink_test_helper.encode_to_string(&engine, &input_buf);
            engine.encode_string(&input_buf, &mut output_buf);

            assert_eq!(output_buf, chunk_encoded_string, "input len={}", buf_len);
        }
    }

    fn chunked_encode_str(bytes: &[u8], config: GeneralPurposeConfig) -> String {
        let mut s = String::new();

        let mut sink = StringSink::new(&mut s);
        let engine = GeneralPurpose::new(&STANDARD, config);
        let encoder = ChunkedEncoder::new(&engine);
        encoder.encode(bytes, &mut sink).unwrap();

        s
    }

    // An abstraction around sinks so that we can have tests that easily to any sink implementation
    pub trait SinkTestHelper {
        fn encode_to_string<E: Engine>(&self, engine: &E, bytes: &[u8]) -> String;
    }

    struct StringSinkTestHelper;

    impl SinkTestHelper for StringSinkTestHelper {
        fn encode_to_string<E: Engine>(&self, engine: &E, bytes: &[u8]) -> String {
            let encoder = ChunkedEncoder::new(engine);
            let mut s = String::new();
            let mut sink = StringSink::new(&mut s);
            encoder.encode(bytes, &mut sink).unwrap();

            s
        }
    }
}
