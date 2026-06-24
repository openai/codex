use pretty_assertions::assert_eq;
use tokio::io::BufReader;

use super::MAX_STDERR_LINE_BYTES;
use super::read_capped_line;

#[tokio::test]
async fn oversized_lines_are_truncated_without_consuming_the_next_line() {
    let mut input = vec![b'a'; MAX_STDERR_LINE_BYTES + 1024];
    input.extend_from_slice(b"\nnext\r\nunterminated");
    let mut reader = BufReader::new(input.as_slice());
    let mut line = Vec::new();

    assert_eq!(
        read_capped_line(&mut reader, &mut line)
            .await
            .expect("read oversized line"),
        Some(true)
    );
    assert_eq!(line, vec![b'a'; MAX_STDERR_LINE_BYTES]);

    assert_eq!(
        read_capped_line(&mut reader, &mut line)
            .await
            .expect("read next line"),
        Some(false)
    );
    assert_eq!(line, b"next");

    assert_eq!(
        read_capped_line(&mut reader, &mut line)
            .await
            .expect("read unterminated line"),
        Some(false)
    );
    assert_eq!(line, b"unterminated");
    assert_eq!(
        read_capped_line(&mut reader, &mut line)
            .await
            .expect("read eof"),
        None
    );
}
