use super::*;
use crate::policy::compile_allowlist;
use crate::policy::compile_denylist;
use crate::policy::normalize_host;
use pretty_assertions::assert_eq;

#[test]
fn unicode_wildcards_preserve_utf8_byte_matching() {
    for (pattern, host, expected) in [
        ("api?.com", "api1.com", true),
        ("api??.com", "apié.com", true),
        ("api????.com", "api😀.com", true),
        ("api???.com", "apie\u{301}.com", true),
        ("api?*.com", "apié.com", true),
        ("*.例???.com", "a.例子.com", true),
        ("*.例???.com", "例子.com", false),
        ("**.例???.com", "例子.com", true),
        ("büch?.de", "büch1.de", true),
        ("bücher.de", "bücher.de", true),
        ("bücher.de", "xn--bcher-kva.de", false),
        ("xn--*.de", "xn--bcher-kva.de", true),
        ("É.com", "é.com", false),
        ("É.COM", "É.com", true),
    ] {
        let mut legacy = globset::GlobSetBuilder::new();
        for expanded in expand_domain_pattern(&normalize_pattern(pattern)) {
            legacy.add(
                globset::GlobBuilder::new(&expanded)
                    .case_insensitive(true)
                    .build()
                    .unwrap(),
            );
        }
        assert_eq!(
            legacy.build().unwrap().is_match(normalize_host(host)),
            expected
        );
        for compile in [compile_allowlist, compile_denylist] {
            assert_eq!(
                compile(&[pattern.to_string()])
                    .unwrap()
                    .is_match(normalize_host(host)),
                expected,
                "{pattern} / {host}"
            );
        }
    }
}

#[test]
fn compilation_rejects_unsupported_syntax_instead_of_dropping_rules() {
    for pattern in [
        "",
        " ",
        "*.",
        "**.",
        "**",
        "a**b.com",
        "[ab].com",
        "a[bc].com",
        "{a,b}.com",
        r"a\*.com",
        "a/b.com",
        "a b.com",
        "[::1]suffix",
        "[fe80::1%abc[def]",
        "[::1]:443[ab]",
        "example.com:{a,b}",
    ] {
        for compile in [compile_allowlist, compile_denylist] {
            assert!(
                compile(&["valid.com".to_string(), pattern.to_string()]).is_err(),
                "{pattern}"
            );
        }
    }
    for pattern in [
        "[::1]",
        "[::1]:443",
        "[fe80::1%25en0]:443",
        "region*.com",
        "**.api?.com",
    ] {
        assert!(
            compile_allowlist(&[pattern.to_string()]).is_ok(),
            "{pattern}"
        );
    }
    for pattern in ["*", "**.*", "[*]", "**.[*]"] {
        assert!(
            compile_denylist(&[pattern.to_string()]).is_err(),
            "{pattern}"
        );
    }
}

#[test]
fn bracketed_patterns_preserve_port_and_trailing_dot_normalization() {
    for (bracketed, plain) in [
        ("[*]", "*"),
        ("[*]:443", "*"),
        ("[*.]", "*"),
        ("[*].", "*"),
        ("[::1]:443", "::1"),
        ("[::1.]", "::1"),
        ("[::1].", "::1"),
        ("[fe80::1%25en0]:443", "fe80::1%en0"),
    ] {
        for prefix in ["", "*.", "**."] {
            for compile in [compile_allowlist, compile_denylist] {
                let compiled_bracketed = compile(&[format!("  {prefix}{bracketed}  ")]);
                let compiled_plain = compile(&[format!("{prefix}{plain}")]);
                match (compiled_bracketed, compiled_plain) {
                    (Ok(bracketed), Ok(plain)) => assert_eq!(bracketed.patterns, plain.patterns),
                    (Err(_), Err(_)) => {}
                    _ => panic!("different compilation outcomes for {prefix}{bracketed:?}"),
                }
            }
        }
    }
}

#[test]
fn matcher_agrees_with_independent_dynamic_program_for_short_inputs() {
    fn words(alphabet: &[u8], max_len: usize) -> Vec<Vec<u8>> {
        let mut all = vec![vec![]];
        let mut level = vec![vec![]];
        for _ in 0..max_len {
            level = level
                .iter()
                .flat_map(|word| {
                    alphabet.iter().map(move |c| {
                        let mut next = word.clone();
                        next.push(*c);
                        next
                    })
                })
                .collect();
            all.extend(level.iter().cloned());
        }
        all
    }
    fn reference(pattern: &[u8], host: &[u8]) -> bool {
        let mut rows = vec![vec![false; host.len() + 1]; pattern.len() + 1];
        rows[0][0] = true;
        for (p, c) in pattern.iter().enumerate() {
            rows[p + 1][0] = *c == b'*' && rows[p][0];
            for (h, candidate) in host.iter().enumerate() {
                rows[p + 1][h + 1] = if *c == b'*' {
                    rows[p][h + 1] || rows[p + 1][h]
                } else {
                    rows[p][h] && (*c == b'?' || c.eq_ignore_ascii_case(candidate))
                };
            }
        }
        rows[pattern.len()][host.len()]
    }
    for pattern in words(&[b'a', 0xc3, 0xa9, b'?', b'*'], /*max_len*/ 4) {
        for host in words(&[b'a', 0xc3, 0xa9], /*max_len*/ 5) {
            assert_eq!(
                matches(&pattern, &host),
                reference(&pattern, &host),
                "{pattern:?} / {host:?}"
            );
        }
    }
}

#[test]
fn adversarial_retries_and_empty_boundaries() {
    let pattern = format!("*{}b", "a".repeat(1024)).into_bytes();
    let host = vec![b'a'; 4096];
    assert!(!matches(&pattern, &host));
    let pattern = format!("{}b", "*a".repeat(1024)).into_bytes();
    assert!(!matches(&pattern, &host));
    assert_eq!(
        (matches(&[], &[]), matches(b"*", &[]), matches(b"?", &[])),
        (true, true, false)
    );
    assert!(!compile_allowlist(&[]).unwrap().is_match("host"));
}

#[test]
fn policy_loading_rejects_an_invalid_entry_in_either_list() {
    for denied in [false, true] {
        let mut config = crate::config::NetworkProxyConfig::default();
        config.set_allowed_domains(vec!["example.com".to_string()]);
        let invalid = vec!["valid.com".to_string(), "api[12].com".to_string()];
        if denied {
            config.set_denied_domains(invalid);
        } else {
            config.set_allowed_domains(invalid);
        }
        assert!(
            crate::state::build_config_state(config, Default::default(), crate::Platform::native())
                .is_err()
        );
    }
}

#[test]
fn normalize_host_lowercases_and_trims() {
    assert_eq!(normalize_host("  ExAmPlE.CoM  "), "example.com");
}

#[test]
fn normalize_host_strips_port_for_host_port() {
    assert_eq!(normalize_host("example.com:1234"), "example.com");
}

#[test]
fn normalize_host_preserves_unbracketed_ipv6() {
    assert_eq!(normalize_host("2001:db8::1"), "2001:db8::1");
}

#[test]
fn normalize_host_strips_trailing_dot() {
    assert_eq!(normalize_host("example.com."), "example.com");
    assert_eq!(normalize_host("ExAmPlE.CoM."), "example.com");
}

#[test]
fn normalize_host_strips_trailing_dot_with_port() {
    assert_eq!(normalize_host("example.com.:443"), "example.com");
}

#[test]
fn normalize_host_strips_brackets_for_ipv6() {
    assert_eq!(normalize_host("[::1]"), "::1");
    assert_eq!(normalize_host("[::1]:443"), "::1");
}

#[test]
fn normalize_host_preserves_ipv6_scope_ids() {
    assert_eq!(normalize_host("fe80::1%lo0"), "fe80::1%lo0");
    assert_eq!(normalize_host("[fe80::1%lo0]"), "fe80::1%lo0");
    assert_eq!(normalize_host("[fe80::1%25lo0]"), "fe80::1%lo0");
}
