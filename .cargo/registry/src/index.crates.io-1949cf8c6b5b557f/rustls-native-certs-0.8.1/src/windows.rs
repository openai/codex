use pki_types::CertificateDer;
use schannel::cert_context::ValidUses;
use schannel::cert_store::CertStore;

use super::CertificateResult;

pub fn load_native_certs() -> CertificateResult {
    let mut result = CertificateResult::default();
    let current_user_store = match CertStore::open_current_user("ROOT") {
        Ok(store) => store,
        Err(err) => {
            result.os_error(err.into(), "failed to open current user certificate store");
            return result;
        }
    };

    for cert in current_user_store.certs() {
        if usable_for_rustls(cert.valid_uses().unwrap()) && cert.is_time_valid().unwrap() {
            result
                .certs
                .push(CertificateDer::from(cert.to_der().to_vec()));
        }
    }

    result
}

fn usable_for_rustls(uses: ValidUses) -> bool {
    match uses {
        ValidUses::All => true,
        ValidUses::Oids(strs) => strs
            .iter()
            .any(|x| x == PKIX_SERVER_AUTH),
    }
}

static PKIX_SERVER_AUTH: &str = "1.3.6.1.5.5.7.3.1";
