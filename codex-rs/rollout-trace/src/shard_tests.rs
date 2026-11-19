#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[test]
fn published_records_survive_rotation_and_remain_readable_while_open() -> Result<()> {
    let bundle = TempDir::new()?;
    let directory = bundle.path().join(PAYLOADS_DIR_NAME);
    std::fs::create_dir(&directory)?;
    let mut writer = ShardWriter::new(directory.clone());
    let mut reader = PayloadReader::new(bundle.path());
    let mut append = |kind, id: &str, body: &[u8]| -> Result<RawPayloadRef> {
        let shard = writer.append(kind, id, body)?;
        Ok(RawPayloadRef {
            raw_payload_id: id.to_owned(),
            kind,
            shard: Some(shard),
            capture: Default::default(),
            parts: Vec::new(),
        })
    };

    let content = append(RawPayloadKind::ContentValue, "content", b"true")?;
    let kind = RawPayloadKind::InferenceRequest;
    let first = append(kind, "first", b"{}")?;
    assert_eq!(reader.read(&first)?, b"{}");
    let body = br#"{"text":"second\nline"}"#;
    let second = append(kind, "second", body)?;
    assert_eq!(second.shard, first.shard);
    assert_eq!(reader.read(&second)?, body);

    // A record larger than the target gets its own shard and remains whole.
    let mut large = vec![b'x'; TARGET_BYTES + 1];
    large[0] = b'"';
    *large.last_mut().unwrap() = b'"';
    let oversized = append(kind, "large", &large)?;
    let last = append(kind, "last", b"null")?;
    assert_ne!(first.shard, oversized.shard);
    assert_ne!(oversized.shard, last.shard);
    assert_eq!(reader.read(&oversized)?, large.as_slice());
    assert_eq!(reader.shards.len(), 1);
    assert_eq!(reader.read(&first)?, b"{}");
    assert_eq!(reader.read(&last)?, b"null");
    // Content is still live even though the structure stream has rotated twice.
    let index = [
        content.clone(),
        first.clone(),
        second,
        oversized,
        last.clone(),
    ]
    .into_iter()
    .map(|payload| (payload.raw_payload_id.clone(), payload))
    .collect();
    reader.validate(&index, /*recording_closed*/ false)?;
    assert!(
        reader
            .read_inner(&content, /*require_sealed*/ true)
            .is_err()
    );
    assert!(reader.read_inner(&last, /*require_sealed*/ true).is_err());
    writer.finish()?;
    reader.validate(&index, /*recording_closed*/ true)?;

    // A superseded shard must retain its footer even when validating a live snapshot.
    let file = File::options()
        .write(true)
        .open(shard_path(&directory, first.shard.unwrap()))?;
    file.set_len(file.metadata()?.len() - 1)?;
    assert!(
        PayloadReader::new(bundle.path())
            .validate(&index, /*recording_closed*/ false)
            .is_err_and(|error| error.to_string().contains("missing footer"))
    );
    Ok(())
}

#[test]
fn trial_frames_share_the_budget_with_both_open_shards() -> Result<()> {
    let directory = TempDir::new()?;
    let mut writer = ShardWriter::new(directory.path().to_owned());
    let content = RawPayloadKind::ContentValue;
    let structure = RawPayloadKind::ToolResult;
    let child = writer.append(content, "child", &(0..=255).collect::<Vec<u8>>())?;
    let parent = writer.append(structure, "parent", b"{}")?;
    let charged = writer.charged_bytes();
    let child_bytes = shard_path(directory.path(), child).metadata()?.len();
    let parent_bytes = shard_path(directory.path(), parent).metadata()?.len();
    assert_eq!(charged, child_bytes + parent_bytes + 2 * 7);

    let body = [b'x'; 512];
    assert_eq!(
        writer.append_with_limit(structure, "trial", &body, charged + 1)?,
        None
    );
    assert!(!DecodedShard::open(directory.path(), parent)?.sealed);

    let limit = charged + 64;
    let trial = writer
        .append_with_limit(structure, "trial", &body, limit)?
        .unwrap();
    assert!(DecodedShard::open(directory.path(), trial)?.sealed);
    assert!(DecodedShard::open(directory.path(), parent)?.sealed);
    assert!(!DecodedShard::open(directory.path(), child)?.sealed);
    let before_close = writer.charged_bytes();
    writer.finish()?;
    let mut total = 0;
    for entry in directory.path().read_dir()? {
        total += entry?.metadata()?.len();
    }
    assert_eq!(total, before_close);
    assert!(total <= limit);
    Ok(())
}

#[test]
fn rotation_counts_the_record_header_and_id() -> Result<()> {
    let directory = TempDir::new()?;
    let mut writer = ShardWriter::new(directory.path().to_owned());
    let kind = RawPayloadKind::InferenceRequest;
    let first = writer.append(kind, "a", b"x")?;
    let remaining = TARGET_BYTES - (HEADER_BYTES + 1 + 1);
    let exact_fit = writer.append(
        kind,
        "longer",
        &vec![0; remaining - HEADER_BYTES - "longer".len()],
    )?;
    let next = writer.append(kind, "c", b"x")?;
    assert_eq!([first, exact_fit, next], [1, 1, 2]);
    Ok(())
}

#[test]
fn live_prefix_survives_a_partial_record_but_a_sealed_frame_rejects_it() -> Result<()> {
    let directory = TempDir::new()?;
    let mut writer = ShardWriter::new(directory.path().to_owned());
    let kind = RawPayloadKind::InferenceRequest;
    let shard = writer.append(kind, "published", b"true")?;
    let encoder = &mut writer.active[stream_index(kind)]
        .as_mut()
        .expect("open shard")
        .encoder;
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
    let kind = RawPayloadKind::InferenceRequest;
    let shard = writer.append(kind, "same", b"true")?;
    writer.append(kind, "same", b"false")?;
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
        let shard = writer.append(RawPayloadKind::InferenceRequest, &id, b"null")?;
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
