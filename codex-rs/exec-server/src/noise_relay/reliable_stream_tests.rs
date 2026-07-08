use std::time::Duration;

use pretty_assertions::assert_eq;
use tokio::time::Instant;

use super::MAX_UNACKED_BYTES;
use super::MAX_UNACKED_SEGMENTS;
use super::OutboundCiphertext;
use super::RESEND_AFTER;
use super::ReliableSender;
use crate::relay::RelayAckState;

#[test]
fn starts_at_sequence_one() {
    let now = Instant::now();
    let mut sender = ReliableSender::default();

    assert_eq!(
        sender
            .admit_ciphertext(b"ciphertext".to_vec(), now)
            .unwrap(),
        OutboundCiphertext {
            seq: 1,
            payload: b"ciphertext".to_vec(),
        }
    );
}

#[test]
fn cumulative_ack_zero_clears_nothing_and_positive_ack_releases_prefix() {
    let now = Instant::now();
    let mut sender = ReliableSender::default();
    sender.admit_ciphertext(b"first".to_vec(), now).unwrap();
    sender.admit_ciphertext(b"second".to_vec(), now).unwrap();

    sender
        .process_peer_ack(RelayAckState {
            ack: 0,
            ack_bits: 0,
        })
        .unwrap();
    assert_eq!(sender.unacked.len(), 2);
    sender
        .process_peer_ack(RelayAckState {
            ack: 1,
            ack_bits: 0,
        })
        .unwrap();
    assert_eq!(sender.unacked.len(), 1);
    assert_eq!(sender.unacked_bytes, b"second".len());
    sender
        .process_peer_ack(RelayAckState {
            ack: 2,
            ack_bits: 0,
        })
        .unwrap();
    assert_eq!(sender.unacked.len(), 0);
    assert_eq!(sender.unacked_bytes, 0);
}

#[test]
fn selective_ack_releases_out_of_order_cached_ciphertext() {
    let now = Instant::now();
    let mut sender = ReliableSender::default();
    let first = sender.admit_ciphertext(b"first".to_vec(), now).unwrap();
    sender.admit_ciphertext(b"second".to_vec(), now).unwrap();
    let third = sender.admit_ciphertext(b"third".to_vec(), now).unwrap();
    sender.admit_ciphertext(b"fourth".to_vec(), now).unwrap();
    let ack_state = RelayAckState {
        ack: 0,
        ack_bits: 0b1010,
    };

    sender.process_peer_ack(ack_state).unwrap();
    sender.process_peer_ack(ack_state).unwrap();

    assert_eq!(
        sender.unacked.keys().copied().collect::<Vec<_>>(),
        vec![1, 3]
    );
    assert_eq!(sender.next_retry_due(now + RESEND_AFTER), Some(first));
    assert_eq!(sender.next_retry_due(now + RESEND_AFTER), Some(third));
    assert_eq!(sender.next_retry_due(now + RESEND_AFTER), None);
}

#[test]
fn rejects_inconsistent_or_unsent_ack_metadata() {
    let now = Instant::now();
    let mut sender = ReliableSender::default();
    sender
        .admit_ciphertext(b"ciphertext".to_vec(), now)
        .unwrap();

    assert!(
        sender
            .process_peer_ack(RelayAckState {
                ack: 0,
                ack_bits: 1,
            })
            .is_err()
    );
    assert!(
        sender
            .process_peer_ack(RelayAckState {
                ack: 0,
                ack_bits: 0b100,
            })
            .is_err()
    );
    assert!(
        sender
            .process_peer_ack(RelayAckState {
                ack: 2,
                ack_bits: 0,
            })
            .is_err()
    );
    assert_eq!(sender.unacked.len(), 1);
}

#[test]
fn selective_acks_do_not_slide_the_cumulative_send_window() {
    let now = Instant::now();
    let mut sender = ReliableSender::default();
    for _ in 0..MAX_UNACKED_SEGMENTS {
        sender.admit_ciphertext(vec![0x5a], now).unwrap();
    }

    sender
        .process_peer_ack(RelayAckState {
            ack: 0,
            ack_bits: u32::MAX - 1,
        })
        .unwrap();

    assert_eq!(sender.unacked.keys().copied().collect::<Vec<_>>(), vec![1]);
    assert!(!sender.can_admit_ciphertext(/*ciphertext_len*/ 1));
    assert!(sender.admit_ciphertext(vec![0x5a], now).is_err());

    sender
        .process_peer_ack(RelayAckState {
            ack: 1,
            ack_bits: 0,
        })
        .unwrap();
    assert!(sender.can_admit_ciphertext(/*ciphertext_len*/ 1));
}

#[test]
fn retries_exact_cached_ciphertext_after_deadline() {
    let now = Instant::now();
    let mut sender = ReliableSender::default();
    let first = sender
        .admit_ciphertext(b"encrypted-once".to_vec(), now)
        .unwrap();

    assert_eq!(
        sender.next_retry_due(now + RESEND_AFTER - Duration::from_millis(1)),
        None
    );
    assert_eq!(
        sender.next_retry_due(now + RESEND_AFTER),
        Some(first.clone())
    );
    assert_eq!(sender.next_retry_due(now + RESEND_AFTER), None);
    assert_eq!(first.payload, b"encrypted-once".to_vec());
}

#[test]
fn returns_one_due_retry_per_scan() {
    let now = Instant::now();
    let mut sender = ReliableSender::default();
    let first = sender.admit_ciphertext(b"first".to_vec(), now).unwrap();
    let second = sender.admit_ciphertext(b"second".to_vec(), now).unwrap();

    assert_eq!(sender.next_retry_due(now + RESEND_AFTER), Some(first));
    assert_eq!(sender.next_retry_due(now + RESEND_AFTER), Some(second));
    assert_eq!(sender.next_retry_due(now + RESEND_AFTER), None);
}

#[test]
fn retry_cursor_does_not_starve_later_due_records() {
    let now = Instant::now();
    let mut sender = ReliableSender::default();
    let first = sender.admit_ciphertext(b"first".to_vec(), now).unwrap();
    let second = sender.admit_ciphertext(b"second".to_vec(), now).unwrap();

    assert_eq!(sender.next_retry_due(now + RESEND_AFTER), Some(first));
    assert_eq!(
        sender.next_retry_due(now + RESEND_AFTER + RESEND_AFTER),
        Some(second)
    );
}

#[test]
fn enforces_segment_and_byte_send_windows() {
    let now = Instant::now();
    let mut segment_window = ReliableSender::default();
    for _ in 0..MAX_UNACKED_SEGMENTS {
        segment_window.admit_ciphertext(vec![0x5a], now).unwrap();
    }
    assert!(!segment_window.can_admit_ciphertext(/*ciphertext_len*/ 1));
    assert!(segment_window.admit_ciphertext(vec![0x5a], now).is_err());

    let mut byte_window = ReliableSender::default();
    byte_window
        .admit_ciphertext(vec![0x5a; MAX_UNACKED_BYTES], now)
        .unwrap();
    assert!(!byte_window.can_admit_ciphertext(/*ciphertext_len*/ 1));
    assert!(byte_window.admit_ciphertext(vec![0x5a], now).is_err());
}
