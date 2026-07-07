//! Windows proxy parsing tests.

use super::*;

#[test]
fn proxy_bypass_matches_whitespace_separated_winhttp_entries() {
    let local_origin = RequestOrigin {
        scheme: "https".to_string(),
        host: "intranet".to_string(),
        port: 443,
    };
    assert!(proxy_bypass_matches_origin("<local> *.corp", &local_origin));

    let corp_origin = RequestOrigin {
        scheme: "https".to_string(),
        host: "service.corp".to_string(),
        port: 443,
    };
    assert!(proxy_bypass_matches_origin("<local> *.corp", &corp_origin));
}
