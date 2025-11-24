use crate::crypto::SupportedKxGroup;
use crate::crypto::aws_lc_rs::kx_group;
use crate::{Error, NamedGroup, PeerMisbehaved};

mod hybrid;
mod mlkem;

/// This is the [X25519MLKEM768] key exchange.
///
/// [X25519MLKEM768]: <https://datatracker.ietf.org/doc/draft-ietf-tls-ecdhe-mlkem/>
pub static X25519MLKEM768: &dyn SupportedKxGroup = &hybrid::Hybrid {
    classical: kx_group::X25519,
    post_quantum: MLKEM768,
    name: NamedGroup::X25519MLKEM768,
    layout: hybrid::Layout {
        classical_share_len: X25519_LEN,
        post_quantum_client_share_len: MLKEM768_ENCAP_LEN,
        post_quantum_server_share_len: MLKEM768_CIPHERTEXT_LEN,
        post_quantum_first: true,
    },
};

/// This is the [SECP256R1MLKEM768] key exchange.
///
/// [SECP256R1MLKEM768]: <https://datatracker.ietf.org/doc/draft-ietf-tls-ecdhe-mlkem/>
pub static SECP256R1MLKEM768: &dyn SupportedKxGroup = &hybrid::Hybrid {
    classical: kx_group::SECP256R1,
    post_quantum: MLKEM768,
    name: NamedGroup::secp256r1MLKEM768,
    layout: hybrid::Layout {
        classical_share_len: SECP256R1_LEN,
        post_quantum_client_share_len: MLKEM768_ENCAP_LEN,
        post_quantum_server_share_len: MLKEM768_CIPHERTEXT_LEN,
        post_quantum_first: false,
    },
};

/// This is the [MLKEM] key exchange.
///
/// [MLKEM]: https://datatracker.ietf.org/doc/draft-connolly-tls-mlkem-key-agreement
pub static MLKEM768: &dyn SupportedKxGroup = &mlkem::MlKem768;

const INVALID_KEY_SHARE: Error = Error::PeerMisbehaved(PeerMisbehaved::InvalidKeyShare);

const X25519_LEN: usize = 32;
const SECP256R1_LEN: usize = 65;
const MLKEM768_CIPHERTEXT_LEN: usize = 1088;
const MLKEM768_ENCAP_LEN: usize = 1184;
