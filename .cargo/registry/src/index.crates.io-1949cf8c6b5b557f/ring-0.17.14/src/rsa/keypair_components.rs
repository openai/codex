use super::PublicKeyComponents;

/// RSA key pair components.
#[derive(Clone, Copy)]
pub struct KeyPairComponents<Public, Private = Public> {
    /// The public key components.
    pub public_key: PublicKeyComponents<Public>,

    /// The private exponent.
    pub d: Private,

    /// The first prime factor of `d`.
    pub p: Private,

    /// The second prime factor of `d`.
    pub q: Private,

    /// `p`'s public Chinese Remainder Theorem exponent.
    pub dP: Private,

    /// `q`'s public Chinese Remainder Theorem exponent.
    pub dQ: Private,

    /// `q**-1 mod p`.
    pub qInv: Private,
}

impl<Public, Private> core::fmt::Debug for KeyPairComponents<Public, Private>
where
    PublicKeyComponents<Public>: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        // Non-public components are intentionally skipped
        f.debug_struct("KeyPairComponents")
            .field("public_key", &self.public_key)
            .finish()
    }
}
