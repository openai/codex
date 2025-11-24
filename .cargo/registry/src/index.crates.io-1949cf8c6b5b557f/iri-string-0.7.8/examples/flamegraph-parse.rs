use iri_string::types::IriReferenceStr;

fn main() {
    for _ in 0..1000000 {
        let s = concat!(
            "scheme://user:pw@sub.example.com:8080/a/b/c/%30/%31/%32%33%34",
            "/foo/foo/../../../foo.foo/foo/foo/././././//////foo",
            "/\u{03B1}\u{03B2}\u{03B3}/\u{03B1}\u{03B2}\u{03B3}/\u{03B1}\u{03B2}\u{03B3}",
            "?k1=v1&k2=v2&k3=v3#fragment"
        );

        let domain = "scheme://sub.sub.sub.example.com:8080/a/b/c";
        let v4 = "scheme://198.51.100.23:8080/a/b/c";
        let v6 = "scheme://[2001:db8:0123::cafe]:8080/a/b/c";
        let v6v4 = "scheme://[2001:db8::198.51.100.23]:8080/a/b/c";
        let vfuture = "scheme://[v2.ipv2-does-not-exist]:8080/a/b/c";
        let _ = (
            IriReferenceStr::new(s),
            IriReferenceStr::new(domain),
            IriReferenceStr::new(v4),
            IriReferenceStr::new(v6),
            IriReferenceStr::new(v6v4),
            IriReferenceStr::new(vfuture),
        );
    }
}
