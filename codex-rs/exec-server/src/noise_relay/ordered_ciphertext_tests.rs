use pretty_assertions::assert_eq;

use super::MAX_PENDING_BYTES;
use super::OrderedCiphertextFrames;

#[test]
fn releases_ciphertexts_only_in_nonce_order() {
    let mut frames = OrderedCiphertextFrames::default();

    assert_eq!(
        frames.push(/*seq*/ 1, b"second".to_vec()).unwrap(),
        Vec::<Vec<u8>>::new()
    );
    assert_eq!(
        frames.push(/*seq*/ 0, b"first".to_vec()).unwrap(),
        vec![b"first".to_vec(), b"second".to_vec()]
    );
}

#[test]
fn ignores_duplicate_ciphertexts_without_replacing_buffered_record() {
    let mut frames = OrderedCiphertextFrames::default();

    assert_eq!(
        frames.push(/*seq*/ 1, b"first copy".to_vec()).unwrap(),
        Vec::<Vec<u8>>::new()
    );
    assert_eq!(
        frames.push(/*seq*/ 1, b"replacement".to_vec()).unwrap(),
        Vec::<Vec<u8>>::new()
    );
    assert_eq!(
        frames.push(/*seq*/ 0, b"zero".to_vec()).unwrap(),
        vec![b"zero".to_vec(), b"first copy".to_vec()]
    );
    assert_eq!(
        frames.push(/*seq*/ 0, b"duplicate".to_vec()).unwrap(),
        Vec::<Vec<u8>>::new()
    );
}

#[test]
fn rejects_unbounded_reordering() {
    let mut frames = OrderedCiphertextFrames::default();

    assert!(frames.push(/*seq*/ 65, Vec::new()).is_err());
    assert!(
        frames
            .push(/*seq*/ 1, vec![0; MAX_PENDING_BYTES + 1])
            .is_err()
    );
}
