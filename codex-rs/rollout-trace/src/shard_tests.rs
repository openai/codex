#![cfg(test)]

use super::*;
use crate::RawPayloadKind;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[test]
fn published_records_survive_rotation_and_remain_readable_while_open() -> Result<()> {
    let bundle = TempDir::new()?;
    let directory = bundle.path().join(PAYLOADS_DIR_NAME);
    std::fs::create_dir(&directory)?;
    let mut writer = ShardWriter::new(directory);
    let mut reader = PayloadReader::new(bundle.path());
    let mut append = |id: &str, body: &[u8]| -> Result<RawPayloadRef> {
        let shard = writer.append(id, body)?;
        Ok(RawPayloadRef {
            raw_payload_id: id.to_owned(),
            kind: RawPayloadKind::InferenceRequest,
            shard: Some(shard),
            capture: Default::default(),
            parts: Vec::new(),
        })
    };

    let first = append("first", b"{}")?;
    assert_eq!(reader.read(&first)?, b"{}");
    let body = br#"{"text":"second\nline"}"#;
    let second = append("second", body)?;
    assert_eq!(second.shard, first.shard);
    assert_eq!(reader.read(&second)?, body);

    // A record larger than the target gets its own shard and remains whole.
    let mut large = vec![b'x'; TARGET_BYTES + 1];
    large[0] = b'"';
    *large.last_mut().unwrap() = b'"';
    let oversized = append("large", &large)?;
    let last = append("last", b"null")?;
    assert_ne!(first.shard, oversized.shard);
    assert_ne!(oversized.shard, last.shard);
    assert_eq!(reader.read(&oversized)?, large.as_slice());
    assert_eq!(reader.shards.len(), 1);
    assert_eq!(reader.read(&first)?, b"{}");
    assert_eq!(reader.read(&last)?, b"null");
    assert!(reader.read_inner(&last, /*require_sealed*/ true).is_err());
    writer.finish()?;
    assert_eq!(reader.read_inner(&last, /*require_sealed*/ true)?, b"null");
    Ok(())
}

#[test]
fn rotation_counts_the_record_header_and_id() -> Result<()> {
    let directory = TempDir::new()?;
    let mut writer = ShardWriter::new(directory.path().to_owned());
    let first = writer.append("a", b"x")?;
    let remaining = TARGET_BYTES - (HEADER_BYTES + 1 + 1);
    let exact_fit = writer.append(
        "longer",
        &vec![0; remaining - HEADER_BYTES - "longer".len()],
    )?;
    let next = writer.append("c", b"x")?;
    assert_eq!([first, exact_fit, next], [1, 1, 2]);
    Ok(())
}

#[test]
fn live_prefix_survives_a_partial_record_but_a_sealed_frame_rejects_it() -> Result<()> {
    let directory = TempDir::new()?;
    let mut writer = ShardWriter::new(directory.path().to_owned());
    let shard = writer.append("published", b"true")?;
    let encoder = writer.active.as_mut().expect("open shard");
    // A valid header and ID promise four body bytes, but the writer stops after two.
    encoder.write_all(&1u16.to_le_bytes())?;
    encoder.write_all(&4u64.to_le_bytes())?;
    encoder.write_all(b"xnu")?;
    encoder.flush()?;
    let live = DecodedShard::open(directory.path(), shard)?;
    assert!(!live.sealed);
    assert_eq!(live.records.len(), 1);
    assert_eq!(&live.bytes[live.records["published"].clone()], b"true");
    writer.finish()?;
    assert!(
        DecodedShard::open(directory.path(), shard)
            .is_err_and(|error| error.to_string().contains("partial record"))
    );
    Ok(())
}

#[test]
fn duplicate_ids_cannot_resolve_to_different_bodies() -> Result<()> {
    let directory = TempDir::new()?;
    let mut writer = ShardWriter::new(directory.path().to_owned());
    let shard = writer.append("same", b"true")?;
    writer.append("same", b"false")?;
    writer.finish()?;
    assert!(
        DecodedShard::open(directory.path(), shard)
            .is_err_and(|error| error.to_string().contains("duplicate payload same"))
    );
    Ok(())
}

#[test]
fn reads_across_a_boundary_keep_the_two_most_recent_shards() -> Result<()> {
    let bundle = TempDir::new()?;
    let directory = bundle.path().join(PAYLOADS_DIR_NAME);
    std::fs::create_dir(&directory)?;
    let mut writer = ShardWriter::new(directory.clone());
    let mut refs = Vec::new();
    for shard in 1..=3 {
        let id = format!("id-{shard}");
        let shard = writer.append(&id, b"null")?;
        writer.finish()?;
        refs.push(RawPayloadRef {
            raw_payload_id: id,
            kind: RawPayloadKind::InferenceRequest,
            shard: Some(shard),
            capture: Default::default(),
            parts: Vec::new(),
        });
    }
    let mut reader = PayloadReader::new(bundle.path());
    reader.read(&refs[0])?;
    reader.read(&refs[1])?;
    // Removing the backing file makes a redundant decompression observable.
    std::fs::remove_file(shard_path(&directory, refs[0].shard.unwrap()))?;
    assert_eq!(reader.read(&refs[0])?, b"null");
    reader.read(&refs[2])?;
    std::fs::remove_file(shard_path(&directory, refs[1].shard.unwrap()))?;
    assert!(reader.read(&refs[1]).is_err());
    Ok(())
}
