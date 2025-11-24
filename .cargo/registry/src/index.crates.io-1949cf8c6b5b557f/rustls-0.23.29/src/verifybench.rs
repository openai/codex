//! This program does benchmarking of the functions in verify.rs,
//! that do certificate chain validation and signature verification.
//!
//! This uses captured certificate chains for a selection of websites,
//! saved in `testdata/cert-{SITE}.{I}.der`.
//!
//! To update that data:
//!
//! - delete all `testdata/cert-*.der`.
//! - run the `admin/capture-certdata` script.
//! - update the verification timestamp near the bottom of this file
//!   to the current time.
//! - where a website's chain length changed, reflect that in the list
//!   of certificate files below.
//!
//! This does not need to be done regularly; because the verification
//! time is fixed, it only needs doing if a root certificate is
//! distrusted.

#![cfg(bench)]

use core::time::Duration;
use std::prelude::v1::*;

use pki_types::{CertificateDer, ServerName, UnixTime};
use webpki_roots;

use crate::crypto::CryptoProvider;
use crate::verify::ServerCertVerifier;
use crate::webpki::{RootCertStore, WebPkiServerVerifier};

#[macro_rules_attribute::apply(bench_for_each_provider)]
mod benchmarks {
    use super::{Context, provider};

    #[bench]
    fn reddit_cert(b: &mut test::Bencher) {
        let ctx = Context::new(
            provider::default_provider(),
            "reddit.com",
            &[
                include_bytes!("testdata/cert-reddit.0.der"),
                include_bytes!("testdata/cert-reddit.1.der"),
            ],
        );
        b.iter(|| ctx.verify_once());
    }

    #[bench]
    fn github_cert(b: &mut test::Bencher) {
        let ctx = Context::new(
            provider::default_provider(),
            "github.com",
            &[
                include_bytes!("testdata/cert-github.0.der"),
                include_bytes!("testdata/cert-github.1.der"),
                include_bytes!("testdata/cert-github.2.der"),
            ],
        );
        b.iter(|| ctx.verify_once());
    }

    #[bench]
    fn arstechnica_cert(b: &mut test::Bencher) {
        let ctx = Context::new(
            provider::default_provider(),
            "arstechnica.com",
            &[
                include_bytes!("testdata/cert-arstechnica.0.der"),
                include_bytes!("testdata/cert-arstechnica.1.der"),
                include_bytes!("testdata/cert-arstechnica.2.der"),
            ],
        );
        b.iter(|| ctx.verify_once());
    }

    #[bench]
    fn servo_cert(b: &mut test::Bencher) {
        let ctx = Context::new(
            provider::default_provider(),
            "servo.org",
            &[
                include_bytes!("testdata/cert-servo.0.der"),
                include_bytes!("testdata/cert-servo.1.der"),
                include_bytes!("testdata/cert-servo.2.der"),
            ],
        );
        b.iter(|| ctx.verify_once());
    }

    #[bench]
    fn twitter_cert(b: &mut test::Bencher) {
        let ctx = Context::new(
            provider::default_provider(),
            "twitter.com",
            &[
                include_bytes!("testdata/cert-twitter.0.der"),
                include_bytes!("testdata/cert-twitter.1.der"),
            ],
        );
        b.iter(|| ctx.verify_once());
    }

    #[bench]
    fn wikipedia_cert(b: &mut test::Bencher) {
        let ctx = Context::new(
            provider::default_provider(),
            "wikipedia.org",
            &[
                include_bytes!("testdata/cert-wikipedia.0.der"),
                include_bytes!("testdata/cert-wikipedia.1.der"),
            ],
        );
        b.iter(|| ctx.verify_once());
    }

    #[bench]
    fn google_cert(b: &mut test::Bencher) {
        let ctx = Context::new(
            provider::default_provider(),
            "www.google.com",
            &[
                include_bytes!("testdata/cert-google.0.der"),
                include_bytes!("testdata/cert-google.1.der"),
                include_bytes!("testdata/cert-google.2.der"),
            ],
        );
        b.iter(|| ctx.verify_once());
    }

    #[bench]
    fn hn_cert(b: &mut test::Bencher) {
        let ctx = Context::new(
            provider::default_provider(),
            "news.ycombinator.com",
            &[
                include_bytes!("testdata/cert-hn.0.der"),
                include_bytes!("testdata/cert-hn.1.der"),
            ],
        );
        b.iter(|| ctx.verify_once());
    }

    #[bench]
    fn stackoverflow_cert(b: &mut test::Bencher) {
        let ctx = Context::new(
            provider::default_provider(),
            "stackoverflow.com",
            &[
                include_bytes!("testdata/cert-stackoverflow.0.der"),
                include_bytes!("testdata/cert-stackoverflow.1.der"),
            ],
        );
        b.iter(|| ctx.verify_once());
    }

    #[bench]
    fn duckduckgo_cert(b: &mut test::Bencher) {
        let ctx = Context::new(
            provider::default_provider(),
            "duckduckgo.com",
            &[
                include_bytes!("testdata/cert-duckduckgo.0.der"),
                include_bytes!("testdata/cert-duckduckgo.1.der"),
                include_bytes!("testdata/cert-duckduckgo.2.der"),
            ],
        );
        b.iter(|| ctx.verify_once());
    }

    #[bench]
    fn rustlang_cert(b: &mut test::Bencher) {
        let ctx = Context::new(
            provider::default_provider(),
            "www.rust-lang.org",
            &[
                include_bytes!("testdata/cert-rustlang.0.der"),
                include_bytes!("testdata/cert-rustlang.1.der"),
                include_bytes!("testdata/cert-rustlang.2.der"),
                include_bytes!("testdata/cert-rustlang.3.der"),
            ],
        );
        b.iter(|| ctx.verify_once());
    }

    #[bench]
    fn wapo_cert(b: &mut test::Bencher) {
        let ctx = Context::new(
            provider::default_provider(),
            "www.washingtonpost.com",
            &[
                include_bytes!("testdata/cert-wapo.0.der"),
                include_bytes!("testdata/cert-wapo.1.der"),
            ],
        );
        b.iter(|| ctx.verify_once());
    }
}

struct Context {
    server_name: ServerName<'static>,
    chain: Vec<CertificateDer<'static>>,
    now: UnixTime,
    verifier: WebPkiServerVerifier,
}

impl Context {
    fn new(provider: CryptoProvider, domain: &'static str, certs: &[&'static [u8]]) -> Self {
        let mut roots = RootCertStore::empty();
        roots.extend(
            webpki_roots::TLS_SERVER_ROOTS
                .iter()
                .cloned(),
        );
        Self {
            server_name: domain.try_into().unwrap(),
            chain: certs
                .iter()
                .copied()
                .map(|bytes| CertificateDer::from(bytes.to_vec()))
                .collect(),
            now: UnixTime::since_unix_epoch(Duration::from_secs(1_746_605_469)),
            verifier: WebPkiServerVerifier::new_without_revocation(
                roots,
                provider.signature_verification_algorithms,
            ),
        }
    }

    fn verify_once(&self) {
        const OCSP_RESPONSE: &[u8] = &[];

        let (end_entity, intermediates) = self.chain.split_first().unwrap();
        self.verifier
            .verify_server_cert(
                end_entity,
                intermediates,
                &self.server_name,
                OCSP_RESPONSE,
                self.now,
            )
            .unwrap();
    }
}
