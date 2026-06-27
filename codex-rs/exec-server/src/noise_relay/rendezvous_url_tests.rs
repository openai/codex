use http::Uri;
use pretty_assertions::assert_eq;

use super::rendezvous_tcp_nodelay;

#[test]
fn enables_tcp_nodelay_only_for_the_exact_treatment_path() {
    for (path, expected) in [
        ("/cloud-agent-nodelay/route/ws/environment/env", true),
        (
            "/cloud-agent-nodelay/route/ws/environment/env?role=harness&sig=abc",
            true,
        ),
        ("/cloud-agent/route/ws/environment/env", false),
        ("/cloud-agent/nodelay/ws/environment/env", false),
        ("/cloud-agent/nodelay/route/ws/environment/env", false),
        ("/cloud-agent-nodelay/route/ws/environment/", false),
        ("/cloud-agent-nodelay/route/ws/environment/env/extra", false),
    ] {
        assert_eq!(
            rendezvous_tcp_nodelay(&path.parse::<Uri>().expect("valid URI")),
            expected,
            "unexpected TCP_NODELAY decision for {path}",
        );
    }
}
