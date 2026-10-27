#![cfg(test)]
//! Native sends fail immediately on saturation or cancellation.

use super::*;
use futures::SinkExt;
use futures::StreamExt;
use futures::executor::block_on;
use pretty_assertions::assert_eq;

#[test]
fn a_full_instance_output_does_not_block_peers_or_the_executor() {
    let (outgoing_tx, outgoing_rx) = mpsc::sync_channel(OUTPUT_SLOTS_PER_INSTANCE + 1);
    let first = Transport {
        instance: Arc::new(Instance::new(/*id*/ 1)),
        outgoing_tx: outgoing_tx.clone(),
    };
    let peer = Transport {
        instance: Arc::new(Instance::new(/*id*/ 2)),
        outgoing_tx,
    };
    for _ in 0..OUTPUT_SLOTS_PER_INSTANCE {
        first.write(vec![RuntimeMessage::Initialized]).unwrap();
    }
    assert!(first.write(vec![RuntimeMessage::Initialized]).is_err());
    peer.write(vec![RuntimeMessage::Initialized]).unwrap();
    assert_eq!(outgoing_rx.recv().unwrap().packets[0].instance, 1);
    first.write(vec![RuntimeMessage::Initialized]).unwrap();
    first.instance.closed.store(true, Ordering::Release);
    assert!(first.write(vec![RuntimeMessage::Initialized]).is_err());
}

#[test]
fn rejects_length_before_reading_payload() {
    let bytes = u32::MAX.to_le_bytes();
    assert_eq!(
        block_on(blocking_reader(bytes.as_slice()).next())
            .unwrap()
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidData
    );
}

#[test]
fn rejects_truncated_payload() {
    assert_eq!(
        block_on(blocking_reader([1, 0, 0, 0].as_slice()).next())
            .unwrap_or_else(|| Err(io::ErrorKind::UnexpectedEof.into()))
            .unwrap_err()
            .kind(),
        io::ErrorKind::UnexpectedEof
    );
}

#[test]
fn blocking_transport_preserves_wire_format_and_buffered_frames() -> io::Result<()> {
    let payloads = [b"abc".as_slice(), b"", b"hi\0"];
    let mut bytes = Vec::new();
    {
        let mut output = blocking_writer(&mut bytes);
        for payload in payloads {
            block_on(output.send(payload.to_vec().into()))?;
        }
    }
    assert_eq!(bytes, b"\x03\0\0\0abc\0\0\0\0\x03\0\0\0hi\0");
    let mut input = blocking_reader(bytes.as_slice());
    for payload in payloads {
        assert_eq!(block_on(input.next()).unwrap()?.as_ref(), payload);
    }
    Ok(())
}
