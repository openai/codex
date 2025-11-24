use alloc::vec::Vec;
use core::cmp;

use pki_types::{DnsName, UnixTime};
use zeroize::Zeroizing;

use crate::client::ResolvesClientCert;
use crate::enums::{CipherSuite, ProtocolVersion};
use crate::error::InvalidMessage;
use crate::msgs::base::{MaybeEmpty, PayloadU8, PayloadU16};
use crate::msgs::codec::{Codec, Reader};
#[cfg(feature = "tls12")]
use crate::msgs::handshake::SessionId;
use crate::msgs::handshake::{CertificateChain, ProtocolName};
use crate::sync::{Arc, Weak};
#[cfg(feature = "tls12")]
use crate::tls12::Tls12CipherSuite;
use crate::tls13::Tls13CipherSuite;
use crate::verify::ServerCertVerifier;

pub(crate) struct Retrieved<T> {
    pub(crate) value: T,
    retrieved_at: UnixTime,
}

impl<T> Retrieved<T> {
    pub(crate) fn new(value: T, retrieved_at: UnixTime) -> Self {
        Self {
            value,
            retrieved_at,
        }
    }

    pub(crate) fn map<M>(&self, f: impl FnOnce(&T) -> Option<&M>) -> Option<Retrieved<&M>> {
        Some(Retrieved {
            value: f(&self.value)?,
            retrieved_at: self.retrieved_at,
        })
    }
}

impl Retrieved<&Tls13ClientSessionValue> {
    pub(crate) fn obfuscated_ticket_age(&self) -> u32 {
        let age_secs = self
            .retrieved_at
            .as_secs()
            .saturating_sub(self.value.common.epoch);
        let age_millis = age_secs as u32 * 1000;
        age_millis.wrapping_add(self.value.age_add)
    }
}

impl<T: core::ops::Deref<Target = ClientSessionCommon>> Retrieved<T> {
    pub(crate) fn has_expired(&self) -> bool {
        let common = &*self.value;
        common.lifetime_secs != 0
            && common
                .epoch
                .saturating_add(u64::from(common.lifetime_secs))
                < self.retrieved_at.as_secs()
    }
}

impl<T> core::ops::Deref for Retrieved<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

#[derive(Debug)]
pub struct Tls13ClientSessionValue {
    suite: &'static Tls13CipherSuite,
    age_add: u32,
    max_early_data_size: u32,
    pub(crate) common: ClientSessionCommon,
    quic_params: PayloadU16,
}

impl Tls13ClientSessionValue {
    pub(crate) fn new(
        suite: &'static Tls13CipherSuite,
        ticket: Arc<PayloadU16>,
        secret: &[u8],
        server_cert_chain: CertificateChain<'static>,
        server_cert_verifier: &Arc<dyn ServerCertVerifier>,
        client_creds: &Arc<dyn ResolvesClientCert>,
        time_now: UnixTime,
        lifetime_secs: u32,
        age_add: u32,
        max_early_data_size: u32,
    ) -> Self {
        Self {
            suite,
            age_add,
            max_early_data_size,
            common: ClientSessionCommon::new(
                ticket,
                secret,
                time_now,
                lifetime_secs,
                server_cert_chain,
                server_cert_verifier,
                client_creds,
            ),
            quic_params: PayloadU16::new(Vec::new()),
        }
    }

    pub fn max_early_data_size(&self) -> u32 {
        self.max_early_data_size
    }

    pub fn suite(&self) -> &'static Tls13CipherSuite {
        self.suite
    }

    #[doc(hidden)]
    /// Test only: rewind epoch by `delta` seconds.
    pub fn rewind_epoch(&mut self, delta: u32) {
        self.common.epoch -= delta as u64;
    }

    #[doc(hidden)]
    /// Test only: replace `max_early_data_size` with `new`
    pub fn _private_set_max_early_data_size(&mut self, new: u32) {
        self.max_early_data_size = new;
    }

    pub fn set_quic_params(&mut self, quic_params: &[u8]) {
        self.quic_params = PayloadU16::new(quic_params.to_vec());
    }

    pub fn quic_params(&self) -> Vec<u8> {
        self.quic_params.0.clone()
    }
}

impl core::ops::Deref for Tls13ClientSessionValue {
    type Target = ClientSessionCommon;

    fn deref(&self) -> &Self::Target {
        &self.common
    }
}

#[derive(Debug, Clone)]
pub struct Tls12ClientSessionValue {
    #[cfg(feature = "tls12")]
    suite: &'static Tls12CipherSuite,
    #[cfg(feature = "tls12")]
    pub(crate) session_id: SessionId,
    #[cfg(feature = "tls12")]
    extended_ms: bool,
    #[doc(hidden)]
    #[cfg(feature = "tls12")]
    pub(crate) common: ClientSessionCommon,
}

#[cfg(feature = "tls12")]
impl Tls12ClientSessionValue {
    pub(crate) fn new(
        suite: &'static Tls12CipherSuite,
        session_id: SessionId,
        ticket: Arc<PayloadU16>,
        master_secret: &[u8],
        server_cert_chain: CertificateChain<'static>,
        server_cert_verifier: &Arc<dyn ServerCertVerifier>,
        client_creds: &Arc<dyn ResolvesClientCert>,
        time_now: UnixTime,
        lifetime_secs: u32,
        extended_ms: bool,
    ) -> Self {
        Self {
            suite,
            session_id,
            extended_ms,
            common: ClientSessionCommon::new(
                ticket,
                master_secret,
                time_now,
                lifetime_secs,
                server_cert_chain,
                server_cert_verifier,
                client_creds,
            ),
        }
    }

    pub(crate) fn ticket(&mut self) -> Arc<PayloadU16> {
        self.common.ticket.clone()
    }

    pub(crate) fn extended_ms(&self) -> bool {
        self.extended_ms
    }

    pub(crate) fn suite(&self) -> &'static Tls12CipherSuite {
        self.suite
    }

    #[doc(hidden)]
    /// Test only: rewind epoch by `delta` seconds.
    pub fn rewind_epoch(&mut self, delta: u32) {
        self.common.epoch -= delta as u64;
    }
}

#[cfg(feature = "tls12")]
impl core::ops::Deref for Tls12ClientSessionValue {
    type Target = ClientSessionCommon;

    fn deref(&self) -> &Self::Target {
        &self.common
    }
}

#[derive(Debug, Clone)]
pub struct ClientSessionCommon {
    ticket: Arc<PayloadU16>,
    secret: Zeroizing<PayloadU8>,
    epoch: u64,
    lifetime_secs: u32,
    server_cert_chain: Arc<CertificateChain<'static>>,
    server_cert_verifier: Weak<dyn ServerCertVerifier>,
    client_creds: Weak<dyn ResolvesClientCert>,
}

impl ClientSessionCommon {
    fn new(
        ticket: Arc<PayloadU16>,
        secret: &[u8],
        time_now: UnixTime,
        lifetime_secs: u32,
        server_cert_chain: CertificateChain<'static>,
        server_cert_verifier: &Arc<dyn ServerCertVerifier>,
        client_creds: &Arc<dyn ResolvesClientCert>,
    ) -> Self {
        Self {
            ticket,
            secret: Zeroizing::new(PayloadU8::new(secret.to_vec())),
            epoch: time_now.as_secs(),
            lifetime_secs: cmp::min(lifetime_secs, MAX_TICKET_LIFETIME),
            server_cert_chain: Arc::new(server_cert_chain),
            server_cert_verifier: Arc::downgrade(server_cert_verifier),
            client_creds: Arc::downgrade(client_creds),
        }
    }

    pub(crate) fn compatible_config(
        &self,
        server_cert_verifier: &Arc<dyn ServerCertVerifier>,
        client_creds: &Arc<dyn ResolvesClientCert>,
    ) -> bool {
        let same_verifier = Weak::ptr_eq(
            &Arc::downgrade(server_cert_verifier),
            &self.server_cert_verifier,
        );
        let same_creds = Weak::ptr_eq(&Arc::downgrade(client_creds), &self.client_creds);

        match (same_verifier, same_creds) {
            (true, true) => true,
            (false, _) => {
                crate::log::trace!("resumption not allowed between different ServerCertVerifiers");
                false
            }
            (_, _) => {
                crate::log::trace!(
                    "resumption not allowed between different ResolvesClientCert values"
                );
                false
            }
        }
    }

    pub(crate) fn server_cert_chain(&self) -> &CertificateChain<'static> {
        &self.server_cert_chain
    }

    pub(crate) fn secret(&self) -> &[u8] {
        self.secret.0.as_ref()
    }

    pub(crate) fn ticket(&self) -> &[u8] {
        self.ticket.0.as_ref()
    }
}

static MAX_TICKET_LIFETIME: u32 = 7 * 24 * 60 * 60;

/// This is the maximum allowed skew between server and client clocks, over
/// the maximum ticket lifetime period.  This encompasses TCP retransmission
/// times in case packet loss occurs when the client sends the ClientHello
/// or receives the NewSessionTicket, _and_ actual clock skew over this period.
static MAX_FRESHNESS_SKEW_MS: u32 = 60 * 1000;

// --- Server types ---
#[derive(Debug)]
pub struct ServerSessionValue {
    pub(crate) sni: Option<DnsName<'static>>,
    pub(crate) version: ProtocolVersion,
    pub(crate) cipher_suite: CipherSuite,
    pub(crate) master_secret: Zeroizing<PayloadU8>,
    pub(crate) extended_ms: bool,
    pub(crate) client_cert_chain: Option<CertificateChain<'static>>,
    pub(crate) alpn: Option<PayloadU8>,
    pub(crate) application_data: PayloadU16,
    pub creation_time_sec: u64,
    pub(crate) age_obfuscation_offset: u32,
    freshness: Option<bool>,
}

impl Codec<'_> for ServerSessionValue {
    fn encode(&self, bytes: &mut Vec<u8>) {
        if let Some(sni) = &self.sni {
            1u8.encode(bytes);
            let sni_bytes: &str = sni.as_ref();
            PayloadU8::<MaybeEmpty>::encode_slice(sni_bytes.as_bytes(), bytes);
        } else {
            0u8.encode(bytes);
        }
        self.version.encode(bytes);
        self.cipher_suite.encode(bytes);
        self.master_secret.encode(bytes);
        (u8::from(self.extended_ms)).encode(bytes);
        if let Some(chain) = &self.client_cert_chain {
            1u8.encode(bytes);
            chain.encode(bytes);
        } else {
            0u8.encode(bytes);
        }
        if let Some(alpn) = &self.alpn {
            1u8.encode(bytes);
            alpn.encode(bytes);
        } else {
            0u8.encode(bytes);
        }
        self.application_data.encode(bytes);
        self.creation_time_sec.encode(bytes);
        self.age_obfuscation_offset
            .encode(bytes);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, InvalidMessage> {
        let has_sni = u8::read(r)?;
        let sni = if has_sni == 1 {
            let dns_name = PayloadU8::<MaybeEmpty>::read(r)?;
            let dns_name = match DnsName::try_from(dns_name.0.as_slice()) {
                Ok(dns_name) => dns_name.to_owned(),
                Err(_) => return Err(InvalidMessage::InvalidServerName),
            };

            Some(dns_name)
        } else {
            None
        };

        let v = ProtocolVersion::read(r)?;
        let cs = CipherSuite::read(r)?;
        let ms = Zeroizing::new(PayloadU8::read(r)?);
        let ems = u8::read(r)?;
        let has_ccert = u8::read(r)? == 1;
        let ccert = if has_ccert {
            Some(CertificateChain::read(r)?.into_owned())
        } else {
            None
        };
        let has_alpn = u8::read(r)? == 1;
        let alpn = if has_alpn {
            Some(PayloadU8::read(r)?)
        } else {
            None
        };
        let application_data = PayloadU16::read(r)?;
        let creation_time_sec = u64::read(r)?;
        let age_obfuscation_offset = u32::read(r)?;

        Ok(Self {
            sni,
            version: v,
            cipher_suite: cs,
            master_secret: ms,
            extended_ms: ems == 1u8,
            client_cert_chain: ccert,
            alpn,
            application_data,
            creation_time_sec,
            age_obfuscation_offset,
            freshness: None,
        })
    }
}

impl ServerSessionValue {
    pub(crate) fn new(
        sni: Option<&DnsName<'_>>,
        v: ProtocolVersion,
        cs: CipherSuite,
        ms: &[u8],
        client_cert_chain: Option<CertificateChain<'static>>,
        alpn: Option<ProtocolName>,
        application_data: Vec<u8>,
        creation_time: UnixTime,
        age_obfuscation_offset: u32,
    ) -> Self {
        Self {
            sni: sni.map(|dns| dns.to_owned()),
            version: v,
            cipher_suite: cs,
            master_secret: Zeroizing::new(PayloadU8::new(ms.to_vec())),
            extended_ms: false,
            client_cert_chain,
            alpn: alpn.map(|p| PayloadU8::new(p.as_ref().to_vec())),
            application_data: PayloadU16::new(application_data),
            creation_time_sec: creation_time.as_secs(),
            age_obfuscation_offset,
            freshness: None,
        }
    }

    #[cfg(feature = "tls12")]
    pub(crate) fn set_extended_ms_used(&mut self) {
        self.extended_ms = true;
    }

    pub(crate) fn set_freshness(
        mut self,
        obfuscated_client_age_ms: u32,
        time_now: UnixTime,
    ) -> Self {
        let client_age_ms = obfuscated_client_age_ms.wrapping_sub(self.age_obfuscation_offset);
        let server_age_ms = (time_now
            .as_secs()
            .saturating_sub(self.creation_time_sec) as u32)
            .saturating_mul(1000);

        let age_difference = server_age_ms.abs_diff(client_age_ms);

        self.freshness = Some(age_difference <= MAX_FRESHNESS_SKEW_MS);
        self
    }

    pub(crate) fn is_fresh(&self) -> bool {
        self.freshness.unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "std")] // for UnixTime::now
    #[test]
    fn serversessionvalue_is_debug() {
        use std::{println, vec};
        let ssv = ServerSessionValue::new(
            None,
            ProtocolVersion::TLSv1_3,
            CipherSuite::TLS13_AES_128_GCM_SHA256,
            &[1, 2, 3],
            None,
            None,
            vec![4, 5, 6],
            UnixTime::now(),
            0x12345678,
        );
        println!("{ssv:?}");
    }

    #[test]
    fn serversessionvalue_no_sni() {
        let bytes = [
            0x00, 0x03, 0x03, 0xc0, 0x23, 0x03, 0x01, 0x02, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x12, 0x23, 0x34, 0x45, 0x56, 0x67, 0x78, 0x89, 0xfe, 0xed, 0xf0, 0x0d,
        ];
        let mut rd = Reader::init(&bytes);
        let ssv = ServerSessionValue::read(&mut rd).unwrap();
        assert_eq!(ssv.get_encoding(), bytes);
    }

    #[test]
    fn serversessionvalue_with_cert() {
        let bytes = [
            0x00, 0x03, 0x03, 0xc0, 0x23, 0x03, 0x01, 0x02, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x12, 0x23, 0x34, 0x45, 0x56, 0x67, 0x78, 0x89, 0xfe, 0xed, 0xf0, 0x0d,
        ];
        let mut rd = Reader::init(&bytes);
        let ssv = ServerSessionValue::read(&mut rd).unwrap();
        assert_eq!(ssv.get_encoding(), bytes);
    }
}
