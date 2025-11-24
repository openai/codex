use crate::{CertPaths, CertificateResult};

pub fn load_native_certs() -> CertificateResult {
    let likely_locations = openssl_probe::probe();
    CertPaths {
        file: likely_locations.cert_file,
        dir: likely_locations.cert_dir,
    }
    .load()
}
