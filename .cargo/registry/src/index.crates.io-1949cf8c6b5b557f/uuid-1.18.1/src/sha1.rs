#[cfg(feature = "v5")]
pub(crate) fn hash(ns: &[u8], src: &[u8]) -> [u8; 16] {
    use sha1_smol::Sha1;

    let mut hasher = Sha1::new();

    hasher.update(ns);
    hasher.update(src);

    let mut bytes = [0; 16];
    bytes.copy_from_slice(&hasher.digest().bytes()[..16]);

    bytes
}
