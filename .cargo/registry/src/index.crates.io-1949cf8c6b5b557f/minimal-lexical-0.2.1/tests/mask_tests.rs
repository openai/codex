use minimal_lexical::mask;

#[test]
fn lower_n_mask_test() {
    assert_eq!(mask::lower_n_mask(2), 0b11);
}

#[test]
fn lower_n_halfway_test() {
    assert_eq!(mask::lower_n_halfway(2), 0b10);
}

#[test]
fn nth_bit_test() {
    assert_eq!(mask::nth_bit(2), 0b100);
}
