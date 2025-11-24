/// Non-panicking `let (nonce, ciphertext) = ciphertext.split_at(...)`.
// TODO(XXX): remove once MSRV reaches 1.80
#[allow(dead_code)] // Complicated conditional compilation guards elided
pub(crate) fn try_split_at(slice: &[u8], mid: usize) -> Option<(&[u8], &[u8])> {
    match mid > slice.len() {
        true => None,
        false => Some(slice.split_at(mid)),
    }
}
