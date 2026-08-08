use pretty_assertions::assert_eq;

use super::MAX_PENDING_BYTES;
use super::OrderedCiphertextFrames;
use crate::relay::RelayAckState;

#[test]
fn releases_ciphertexts_only_in_nonce_order() {
    let mut frames = OrderedCiphertextFrames::default();

    assert_eq!(
        frames.push(/*seq*/ 2, b"second".to_vec()).unwrap(),
        Vec::<Vec<u8>>::new()
    );
    assert_eq!(
        frames.ack_state(),
        RelayAckState {
            ack: 0,
            ack_bits: 0b10,
        }
    );
    assert_eq!(
        frames.push(/*seq*/ 1, b"first".to_vec()).unwrap(),
        vec![b"first".to_vec(), b"second".to_vec()]
    );
    assert_eq!(
        frames.ack_state(),
        RelayAckState {
            ack: 2,
            ack_bits: 0,
        }
    );
}

#[test]
fn ignores_duplicate_ciphertexts_without_replacing_buffered_record() {
    let mut frames = OrderedCiphertextFrames::default();

    assert_eq!(
        frames.push(/*seq*/ 2, b"first copy".to_vec()).unwrap(),
        Vec::<Vec<u8>>::new()
    );
    assert_eq!(
        frames.push(/*seq*/ 2, b"replacement".to_vec()).unwrap(),
        Vec::<Vec<u8>>::new()
    );
    assert_eq!(
        frames.ack_state(),
        RelayAckState {
            ack: 0,
            ack_bits: 0b10,
        }
    );
    assert_eq!(
        frames.push(/*seq*/ 1, b"one".to_vec()).unwrap(),
        vec![b"one".to_vec(), b"first copy".to_vec()]
    );
    assert_eq!(
        frames.push(/*seq*/ 1, b"duplicate".to_vec()).unwrap(),
        Vec::<Vec<u8>>::new()
    );
}

#[test]
fn selective_ack_bits_shift_after_a_gap_closes() {
    let mut frames = OrderedCiphertextFrames::default();

    assert_eq!(
        frames.push(/*seq*/ 2, b"two".to_vec()).unwrap(),
        Vec::<Vec<u8>>::new()
    );
    assert_eq!(
        frames.push(/*seq*/ 4, b"four".to_vec()).unwrap(),
        Vec::<Vec<u8>>::new()
    );
    assert_eq!(
        frames.ack_state(),
        RelayAckState {
            ack: 0,
            ack_bits: 0b1010,
        }
    );

    assert_eq!(
        frames.push(/*seq*/ 1, b"one".to_vec()).unwrap(),
        vec![b"one".to_vec(), b"two".to_vec()]
    );
    assert_eq!(
        frames.ack_state(),
        RelayAckState {
            ack: 2,
            ack_bits: 0b10,
        }
    );

    assert_eq!(
        frames.push(/*seq*/ 3, b"three".to_vec()).unwrap(),
        vec![b"three".to_vec(), b"four".to_vec()]
    );
    assert_eq!(
        frames.ack_state(),
        RelayAckState {
            ack: 4,
            ack_bits: 0,
        }
    );
}

#[test]
fn rejects_unbounded_reordering() {
    let mut frames = OrderedCiphertextFrames::default();

    assert!(frames.push(/*seq*/ 0, Vec::new()).is_err());
    assert!(frames.push(/*seq*/ 33, Vec::new()).is_err());
    assert!(
        frames
            .push(/*seq*/ 2, vec![0; MAX_PENDING_BYTES + 1])
            .is_err()
    );
}

#[test]
fn buffers_the_full_receive_window_behind_one_gap() {
    let mut frames = OrderedCiphertextFrames::default();

    for seq in 2..=32 {
        assert!(frames.push(seq, vec![0; 64 * 1024]).is_ok());
    }
    assert_eq!(
        frames.ack_state(),
        RelayAckState {
            ack: 0,
            ack_bits: u32::MAX - 1,
        }
    );
}
