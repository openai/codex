use alloc::boxed::Box;
use alloc::vec::Vec;

use super::INVALID_KEY_SHARE;
use crate::crypto::{ActiveKeyExchange, CompletedKeyExchange, SharedSecret, SupportedKxGroup};
use crate::ffdhe_groups::FfdheGroup;
use crate::{Error, NamedGroup, ProtocolVersion};

/// A generalization of hybrid key exchange.
#[derive(Debug)]
pub(crate) struct Hybrid {
    pub(crate) classical: &'static dyn SupportedKxGroup,
    pub(crate) post_quantum: &'static dyn SupportedKxGroup,
    pub(crate) name: NamedGroup,
    pub(crate) layout: Layout,
}

impl SupportedKxGroup for Hybrid {
    fn start(&self) -> Result<Box<dyn ActiveKeyExchange>, Error> {
        let classical = self.classical.start()?;
        let post_quantum = self.post_quantum.start()?;

        let combined_pub_key = self
            .layout
            .concat(post_quantum.pub_key(), classical.pub_key());

        Ok(Box::new(ActiveHybrid {
            classical,
            post_quantum,
            name: self.name,
            layout: self.layout,
            combined_pub_key,
        }))
    }

    fn start_and_complete(&self, client_share: &[u8]) -> Result<CompletedKeyExchange, Error> {
        let (post_quantum_share, classical_share) = self
            .layout
            .split_received_client_share(client_share)
            .ok_or(INVALID_KEY_SHARE)?;

        let cl = self
            .classical
            .start_and_complete(classical_share)?;
        let pq = self
            .post_quantum
            .start_and_complete(post_quantum_share)?;

        let combined_pub_key = self
            .layout
            .concat(&pq.pub_key, &cl.pub_key);
        let secret = self
            .layout
            .concat(pq.secret.secret_bytes(), cl.secret.secret_bytes());

        Ok(CompletedKeyExchange {
            group: self.name,
            pub_key: combined_pub_key,
            secret: SharedSecret::from(secret),
        })
    }

    fn ffdhe_group(&self) -> Option<FfdheGroup<'static>> {
        None
    }

    fn name(&self) -> NamedGroup {
        self.name
    }

    fn fips(&self) -> bool {
        // Behold! The Night Mare: SP800-56C rev 2:
        //
        // "In addition to the currently approved techniques for the generation of the
        // shared secret Z as specified in SP 800-56A and SP 800-56B, this Recommendation
        // permits the use of a "hybrid" shared secret of the form Z′ = Z || T, a
        // concatenation consisting of a "standard" shared secret Z that was generated
        // during the execution of a key-establishment scheme (as currently specified in
        // [SP 800-56A] or [SP 800-56B])"
        //
        // NIST plan to adjust this and allow both orders: see
        // <https://csrc.nist.gov/pubs/sp/800/227/ipd> (Jan 2025) lines 1070-1080.
        //
        // But, for now, we follow the SP800-56C logic: the element appearing first is the
        // one that controls approval.
        match self.layout.post_quantum_first {
            true => self.post_quantum.fips(),
            false => self.classical.fips(),
        }
    }

    fn usable_for_version(&self, version: ProtocolVersion) -> bool {
        version == ProtocolVersion::TLSv1_3
    }
}

struct ActiveHybrid {
    classical: Box<dyn ActiveKeyExchange>,
    post_quantum: Box<dyn ActiveKeyExchange>,
    name: NamedGroup,
    layout: Layout,
    combined_pub_key: Vec<u8>,
}

impl ActiveKeyExchange for ActiveHybrid {
    fn complete(self: Box<Self>, peer_pub_key: &[u8]) -> Result<SharedSecret, Error> {
        let (post_quantum_share, classical_share) = self
            .layout
            .split_received_server_share(peer_pub_key)
            .ok_or(INVALID_KEY_SHARE)?;

        let cl = self
            .classical
            .complete(classical_share)?;
        let pq = self
            .post_quantum
            .complete(post_quantum_share)?;

        let secret = self
            .layout
            .concat(pq.secret_bytes(), cl.secret_bytes());
        Ok(SharedSecret::from(secret))
    }

    /// Allow the classical computation to be offered and selected separately.
    fn hybrid_component(&self) -> Option<(NamedGroup, &[u8])> {
        Some((self.classical.group(), self.classical.pub_key()))
    }

    fn complete_hybrid_component(
        self: Box<Self>,
        peer_pub_key: &[u8],
    ) -> Result<SharedSecret, Error> {
        self.classical.complete(peer_pub_key)
    }

    fn pub_key(&self) -> &[u8] {
        &self.combined_pub_key
    }

    fn ffdhe_group(&self) -> Option<FfdheGroup<'static>> {
        None
    }

    fn group(&self) -> NamedGroup {
        self.name
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Layout {
    /// Length of classical key share.
    pub(crate) classical_share_len: usize,

    /// Length of post-quantum key share sent by client
    pub(crate) post_quantum_client_share_len: usize,

    /// Length of post-quantum key share sent by server
    pub(crate) post_quantum_server_share_len: usize,

    /// Whether the post-quantum element comes first in shares and secrets.
    ///
    /// For dismal and unprincipled reasons, SECP256R1MLKEM768 has the
    /// classical element first, while X25519MLKEM768 has it second.
    pub(crate) post_quantum_first: bool,
}

impl Layout {
    fn split_received_client_share<'a>(&self, share: &'a [u8]) -> Option<(&'a [u8], &'a [u8])> {
        self.split(share, self.post_quantum_client_share_len)
    }

    fn split_received_server_share<'a>(&self, share: &'a [u8]) -> Option<(&'a [u8], &'a [u8])> {
        self.split(share, self.post_quantum_server_share_len)
    }

    /// Return the PQ and classical component of a key share.
    fn split<'a>(
        &self,
        share: &'a [u8],
        post_quantum_share_len: usize,
    ) -> Option<(&'a [u8], &'a [u8])> {
        if share.len() != self.classical_share_len + post_quantum_share_len {
            return None;
        }

        Some(match self.post_quantum_first {
            true => {
                let (first_share, second_share) = share.split_at(post_quantum_share_len);
                (first_share, second_share)
            }
            false => {
                let (first_share, second_share) = share.split_at(self.classical_share_len);
                (second_share, first_share)
            }
        })
    }

    fn concat(&self, post_quantum: &[u8], classical: &[u8]) -> Vec<u8> {
        match self.post_quantum_first {
            true => [post_quantum, classical].concat(),
            false => [classical, post_quantum].concat(),
        }
    }
}
