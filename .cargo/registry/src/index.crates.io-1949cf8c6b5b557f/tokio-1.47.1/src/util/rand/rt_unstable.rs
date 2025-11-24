use super::RngSeed;

use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;

impl RngSeed {
    /// Generates a seed from the provided byte slice.
    ///
    /// # Example
    ///
    /// ```
    /// # use tokio::runtime::RngSeed;
    /// let seed = RngSeed::from_bytes(b"make me a seed");
    /// ```
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut hasher = DefaultHasher::default();
        hasher.write(bytes);
        Self::from_u64(hasher.finish())
    }
}
