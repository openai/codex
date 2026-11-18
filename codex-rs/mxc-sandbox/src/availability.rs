//! Cache the public SDK's ProcessContainer availability probe.

use std::sync::OnceLock;

pub(super) fn is_available() -> bool {
    static USABLE: OnceLock<bool> = OnceLock::new();
    *USABLE.get_or_init(|| {
        mxc_sdk::v1::probe(/* request */ None)
            .is_ok_and(|probe| probe.error.is_none() && probe.probes.base_container_api_present)
    })
}
