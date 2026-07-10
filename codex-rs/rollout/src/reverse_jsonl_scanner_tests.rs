use std::io::Cursor;
use std::io::Read;
use std::io::Seek;

use pretty_assertions::assert_eq;
use serde::Deserialize;
use serde::Serialize;

use super::ReverseJsonlScanner;
use super::ScanOutcome;

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct TestRecord {
    value: String,
}

fn record(value: &str) -> TestRecord {
    TestRecord {
        value: value.to_string(),
    }
}

fn parsed<T>(outcome: Option<ScanOutcome<T>>) -> T {
    let Some(ScanOutcome::Parsed(record)) = outcome else {
        panic!("expected parsed record");
    };
    record
}

fn assert_records<R>(scanner: &mut ReverseJsonlScanner<R>, expected: &[&str]) -> std::io::Result<()>
where
    R: Read + Seek,
{
    for value in expected {
        assert_eq!(parsed(scanner.scan_next::<TestRecord>()?), record(value));
    }
    assert!(scanner.scan_next::<TestRecord>()?.is_none());
    Ok(())
}

#[test]
fn scans_jsonl_records_from_end() -> std::io::Result<()> {
    let input = br#"{"value":"first"}
{"value":"second"}
{"value":"third"}
"#;

    assert_records(
        &mut ReverseJsonlScanner::new(Cursor::new(input))?,
        &["third", "second", "first"],
    )
}

#[test]
fn rejects_invalid_json_and_continues_scanning() -> std::io::Result<()> {
    let input = br#"{"value":"first"}
not-json
{"value":"third"}
"#;
    let mut scanner = ReverseJsonlScanner::new(Cursor::new(input))?;

    assert_eq!(parsed(scanner.scan_next::<TestRecord>()?), record("third"));
    let Some(ScanOutcome::Rejected(error)) = scanner.scan_next::<TestRecord>()? else {
        panic!("expected rejected record");
    };
    assert!(error.is_syntax());
    assert_eq!(parsed(scanner.scan_next::<TestRecord>()?), record("first"));
    Ok(())
}

#[test]
fn accepts_valid_json_at_eof() -> std::io::Result<()> {
    let input = b"{\"value\":\"first\"}\n{\"value\":\"second\"}";

    assert_records(
        &mut ReverseJsonlScanner::new(Cursor::new(input))?,
        &["second", "first"],
    )
}

#[test]
fn rejects_invalid_json_at_eof_and_continues_scanning() -> std::io::Result<()> {
    let input = b"{\"value\":\"first\"}\n{\"value\":";
    let mut scanner = ReverseJsonlScanner::new(Cursor::new(input))?;

    let Some(ScanOutcome::Rejected(error)) = scanner.scan_next::<TestRecord>()? else {
        panic!("expected rejected record");
    };
    assert!(error.is_eof());
    assert_eq!(parsed(scanner.scan_next::<TestRecord>()?), record("first"));
    Ok(())
}

#[test]
fn skips_blank_lines_with_or_without_termination() -> std::io::Result<()> {
    let input = b"{\"value\":\"first\"}\r\n\n \t\r";

    assert_records(
        &mut ReverseJsonlScanner::new(Cursor::new(input))?,
        &["first"],
    )
}

#[test]
fn scans_across_read_chunk_boundaries() -> std::io::Result<()> {
    let empty_record_len = serde_json::to_string(&record(""))?.len();
    for distance_from_eof in [
        super::READ_CHUNK_SIZE - 1,
        super::READ_CHUNK_SIZE,
        super::READ_CHUNK_SIZE + 1,
    ] {
        let large_value = "x".repeat(distance_from_eof - empty_record_len - 2);
        let input = format!(
            "{}\n{}\n",
            serde_json::to_string(&record("first"))?,
            serde_json::to_string(&record(&large_value))?
        );
        let mut scanner = ReverseJsonlScanner::new(Cursor::new(input.into_bytes()))?;

        assert_eq!(
            parsed(scanner.scan_next::<TestRecord>()?),
            record(&large_value)
        );
        assert_eq!(parsed(scanner.scan_next::<TestRecord>()?), record("first"));
    }
    Ok(())
}

#[test]
fn scans_record_spanning_three_read_chunks() -> std::io::Result<()> {
    let large_value = "x".repeat(super::READ_CHUNK_SIZE * 2);
    let input = format!(
        "{}\n{}\n{}\n",
        serde_json::to_string(&record("first"))?,
        serde_json::to_string(&record(&large_value))?,
        serde_json::to_string(&record("third"))?
    );
    let mut scanner = ReverseJsonlScanner::new(Cursor::new(input.into_bytes()))?;

    assert_records(&mut scanner, &["third", &large_value, "first"])
}
