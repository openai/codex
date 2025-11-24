use std::collections::HashMap;

use pki_types::CertificateDer;
use security_framework::trust_settings::{Domain, TrustSettings, TrustSettingsForCertificate};

use super::CertificateResult;

pub fn load_native_certs() -> CertificateResult {
    // The various domains are designed to interact like this:
    //
    // "Per-user Trust Settings override locally administered
    //  Trust Settings, which in turn override the System Trust
    //  Settings."
    //
    // So we collect the certificates in this order; as a map of
    // their DER encoding to what we'll do with them.  We don't
    // overwrite existing elements, which mean User settings
    // trump Admin trump System, as desired.

    let mut result = CertificateResult::default();
    let mut all_certs = HashMap::new();
    for domain in &[Domain::User, Domain::Admin, Domain::System] {
        let ts = TrustSettings::new(*domain);
        let iter = match ts.iter() {
            Ok(iter) => iter,
            Err(err) => {
                result.os_error(
                    err.into(),
                    match domain {
                        Domain::User => "failed to load user trust settings",
                        Domain::Admin => "failed to load admin trust settings",
                        Domain::System => "failed to load system trust settings",
                    },
                );
                continue;
            }
        };

        for cert in iter {
            let der = cert.to_der();

            // If there are no specific trust settings, the default
            // is to trust the certificate as a root cert.  Weird API but OK.
            // The docs say:
            //
            // "Note that an empty Trust Settings array means "always trust this cert,
            //  with a resulting kSecTrustSettingsResult of kSecTrustSettingsResultTrustRoot".
            let trusted = match ts.tls_trust_settings_for_certificate(&cert) {
                Ok(trusted) => trusted.unwrap_or(TrustSettingsForCertificate::TrustRoot),
                Err(err) => {
                    result.os_error(err.into(), "certificate not trusted");
                    continue;
                }
            };

            all_certs.entry(der).or_insert(trusted);
        }
    }

    // Now we have all the certificates and an idea of whether
    // to use them.
    for (der, trusted) in all_certs.drain() {
        use TrustSettingsForCertificate::*;
        if let TrustRoot | TrustAsRoot = trusted {
            result
                .certs
                .push(CertificateDer::from(der));
        }
    }

    result
}
