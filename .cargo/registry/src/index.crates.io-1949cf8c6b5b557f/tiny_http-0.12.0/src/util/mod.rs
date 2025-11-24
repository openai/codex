pub use self::custom_stream::CustomStream;
pub use self::equal_reader::EqualReader;
pub use self::fused_reader::FusedReader;
pub use self::messages_queue::MessagesQueue;
pub use self::refined_tcp_stream::RefinedTcpStream;
pub use self::sequential::{SequentialReader, SequentialReaderBuilder};
pub use self::sequential::{SequentialWriter, SequentialWriterBuilder};
pub use self::task_pool::TaskPool;

use std::str::FromStr;

mod custom_stream;
mod equal_reader;
mod fused_reader;
mod messages_queue;
pub(crate) mod refined_tcp_stream;
mod sequential;
mod task_pool;

/// Parses a the value of a header.
/// Suitable for `Accept-*`, `TE`, etc.
///
/// For example with `text/plain, image/png; q=1.5` this function would
/// return `[ ("text/plain", 1.0), ("image/png", 1.5) ]`
pub fn parse_header_value(input: &str) -> Vec<(&str, f32)> {
    input
        .split(',')
        .filter_map(|elem| {
            let mut params = elem.split(';');

            let t = params.next()?;

            let mut value = 1.0_f32;

            for p in params {
                if p.trim_start().starts_with("q=") {
                    if let Ok(val) = f32::from_str(p.trim_start()[2..].trim()) {
                        value = val;
                        break;
                    }
                }
            }

            Some((t.trim(), value))
        })
        .collect()
}

#[cfg(test)]
mod test {
    #[test]
    #[allow(clippy::float_cmp)]
    fn test_parse_header() {
        let result = super::parse_header_value("text/html, text/plain; q=1.5 , image/png ; q=2.0");

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].0, "text/html");
        assert_eq!(result[0].1, 1.0);
        assert_eq!(result[1].0, "text/plain");
        assert_eq!(result[1].1, 1.5);
        assert_eq!(result[2].0, "image/png");
        assert_eq!(result[2].1, 2.0);
    }
}
