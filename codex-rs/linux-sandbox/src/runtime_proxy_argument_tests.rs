use super::*;
use pretty_assertions::assert_eq;

#[test]
fn rewrites_runtime_proxy_argument_to_effective_loopback_endpoint() {
    let mut command = vec![
        "carbonyl".to_string(),
        "--proxy-server=http://127.0.0.1:43128".to_string(),
        "--remote-debugging-pipe".to_string(),
    ];

    rewrite_http_proxy_argument(&mut command, "--proxy-server=", "http://127.0.0.1:45219")
        .expect("rewrite proxy argument");

    assert_eq!(
        command,
        vec![
            "carbonyl".to_string(),
            "--proxy-server=http://127.0.0.1:45219".to_string(),
            "--remote-debugging-pipe".to_string(),
        ]
    );
}

#[test]
fn runtime_proxy_argument_rewrite_rejects_ambiguous_or_non_loopback_inputs() {
    let mut duplicate = vec![
        "carbonyl".to_string(),
        "--proxy-server=http://127.0.0.1:1".to_string(),
        "--proxy-server=http://127.0.0.1:2".to_string(),
    ];
    let duplicate_error =
        rewrite_http_proxy_argument(&mut duplicate, "--proxy-server=", "http://127.0.0.1:45219")
            .expect_err("duplicate proxy arguments must be rejected");
    assert_eq!(duplicate_error.kind(), std::io::ErrorKind::InvalidInput);

    let mut command = vec![
        "carbonyl".to_string(),
        "--proxy-server=http://127.0.0.1:1".to_string(),
    ];
    let endpoint_error =
        rewrite_http_proxy_argument(&mut command, "--proxy-server=", "http://192.0.2.10:45219")
            .expect_err("non-loopback effective proxy must be rejected");
    assert_eq!(endpoint_error.kind(), std::io::ErrorKind::InvalidInput);
}

#[test]
fn runtime_proxy_argument_rewrite_rejects_non_http_proxy_schemes() {
    let mut command = vec![
        "carbonyl".to_string(),
        "--proxy-server=http://127.0.0.1:1".to_string(),
    ];

    let error =
        rewrite_http_proxy_argument(&mut command, "--proxy-server=", "socks5h://127.0.0.1:45219")
            .expect_err("non-HTTP effective proxy must be rejected");

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
}
