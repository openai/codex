use rustls_pki_types::DnsName;

// (name, is_valid)
static DNS_NAME_VALIDITY: &[(&[u8], bool)] = &[
    (b"a", true),
    (b"a.b", true),
    (b"a.b.c", true),
    (b"a.b.c.d", true),

    // Hyphens, one component.
    (b"-", false),
    (b"-a", false),
    (b"a-", false),
    (b"a-b", true),

    // Hyphens, last component.
    (b"a.-", false),
    (b"a.-a", false),
    (b"a.a-", false),
    (b"a.a-b", true),

    // Hyphens, not last component.
    (b"-.a", false),
    (b"-a.a", false),
    (b"a-.a", false),
    (b"a-b.a", true),

    // Underscores, one component.
    (b"_", true), // TODO: Perhaps this should be rejected for '_' being sole character?.
    (b"_a", true), // TODO: Perhaps this should be rejected for '_' being 1st?
    (b"a_", true),
    (b"a_b", true),

    // Underscores, last component.
    (b"a._", true), // TODO: Perhaps this should be rejected for '_' being sole character?.
    (b"a._a", true), // TODO: Perhaps this should be rejected for '_' being 1st?
    (b"a.a_", true),
    (b"a.a_b", true),

    // Underscores, not last component.
    (b"_.a", true), // TODO: Perhaps this should be rejected for '_' being sole character?.
    (b"_a.a", true),
    (b"a_.a", true),
    (b"a_b.a", true),

    // empty labels
    (b"", false),
    (b".", false),
    (b"a", true),
    (b".a", false),
    (b".a.b", false),
    (b"..a", false),
    (b"a..b", false),
    (b"a...b", false),
    (b"a..b.c", false),
    (b"a.b..c", false),
    (b".a.b.c.", false),

    // absolute names
    (b"a.", true),
    (b"a.b.", true),
    (b"a.b.c.", true),

    // absolute names with empty label at end
    (b"a..", false),
    (b"a.b..", false),
    (b"a.b.c..", false),
    (b"a...", false),

    // Punycode
    (b"xn--", false),
    (b"xn--.", false),
    (b"xn--.a", false),
    (b"a.xn--", false),
    (b"a.xn--.", false),
    (b"a.xn--.b", false),
    (b"a.xn--.b", false),
    (b"a.xn--\0.b", false),
    (b"a.xn--a.b", true),
    (b"xn--a", true),
    (b"a.xn--a", true),
    (b"a.xn--a.a", true),
    (b"\xc4\x95.com", false), // UTF-8 ĕ
    (b"xn--jea.com", true), // punycode ĕ
    (b"xn--\xc4\x95.com", false), // UTF-8 ĕ, malformed punycode + UTF-8 mashup

    // Surprising punycode
    (b"xn--google.com", true), // 䕮䕵䕶䕱.com
    (b"xn--citibank.com", true), // 岍岊岊岅岉岎.com
    (b"xn--cnn.com", true), // 䁾.com
    (b"a.xn--cnn", true), // a.䁾
    (b"a.xn--cnn.com", true), // a.䁾.com

    (b"1.2.3.4", false), // IPv4 address
    (b"1::2", false), // IPV6 address

    // whitespace not allowed anywhere.
    (b" ", false),
    (b" a", false),
    (b"a ", false),
    (b"a b", false),
    (b"a.b 1", false),
    (b"a\t", false),

    // Nulls not allowed
    (b"\0", false),
    (b"a\0", false),
    (b"example.org\0.example.com", false), // Hi Moxie!
    (b"\0a", false),
    (b"xn--\0", false),

    // Allowed character set
    (b"a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z", true),
    (b"A.B.C.D.E.F.G.H.I.J.K.L.M.N.O.P.Q.R.S.T.U.V.W.X.Y.Z", true),
    (b"0.1.2.3.4.5.6.7.8.9.a", true), // "a" needed to avoid numeric last label
    (b"a-b", true), // hyphen (a label cannot start or end with a hyphen)

    // An invalid character in various positions
    (b"!", false),
    (b"!a", false),
    (b"a!", false),
    (b"a!b", false),
    (b"a.!", false),
    (b"a.a!", false),
    (b"a.!a", false),
    (b"a.a!a", false),
    (b"a.!a.a", false),
    (b"a.a!.a", false),
    (b"a.a!a.a", false),

    // Various other invalid characters
    (b"a!", false),
    (b"a@", false),
    (b"a#", false),
    (b"a$", false),
    (b"a%", false),
    (b"a^", false),
    (b"a&", false),
    (b"a*", false),
    (b"a(", false),
    (b"a)", false),

    // last label can't be fully numeric
    (b"1", false),
    (b"a.1", false),

    // other labels can be fully numeric
    (b"1.a", true),
    (b"1.2.a", true),
    (b"1.2.3.a", true),

    // last label can be *partly* numeric
    (b"1a", true),
    (b"1.1a", true),
    (b"1-1", true),
    (b"a.1-1", true),
    (b"a.1-a", true),

    // labels cannot start with a hyphen
    (b"-", false),
    (b"-1", false),

    // labels cannot end with a hyphen
    (b"1-", false),
    (b"1-.a", false),
    (b"a-", false),
    (b"a-.a", false),
    (b"a.1-.a", false),
    (b"a.a-.a", false),

    // labels can contain a hyphen in the middle
    (b"a-b", true),
    (b"1-2", true),
    (b"a.a-1", true),

    // multiple consecutive hyphens allowed
    (b"a--1", true),
    (b"1---a", true),
    (b"a-----------------b", true),

    // Wildcard specifications are not valid reference names.
    (b"*.a", false),
    (b"a*", false),
    (b"a*.", false),
    (b"a*.a", false),
    (b"a*.a.", false),
    (b"*.a.b", false),
    (b"*.a.b.", false),
    (b"a*.b.c", false),
    (b"*.a.b.c", false),
    (b"a*.b.c.d", false),

    // Multiple wildcards.
    (b"a**.b.c", false),
    (b"a*b*.c.d", false),
    (b"a*.b*.c", false),

    // Wildcards not in the first label.
    (b"a.*", false),
    (b"a.*.b", false),
    (b"a.b.*", false),
    (b"a.b*.c", false),
    (b"*.b*.c", false),
    (b".*.a.b", false),
    (b".a*.b.c", false),

    // Wildcards not at the end of the first label.
    (b"*a.b.c", false),
    (b"a*b.c.d", false),

    // Wildcards and IDNA prefix.
    (b"x*.a.b", false),
    (b"xn*.a.b", false),
    (b"xn-*.a.b", false),
    (b"xn--*.a.b", false),
    (b"xn--w*.a.b", false),

    // Redacted labels from RFC6962bis draft 4
    // https://tools.ietf.org/html/draft-ietf-trans-rfc6962-bis-04#section-3.2.2
    (b"(PRIVATE).foo", false),

    // maximum label length is 63 characters
    (b"123456789012345678901234567890123456789012345678901234567890abc", true),
    (b"123456789012345678901234567890123456789012345678901234567890abcd", false),

    // maximum total length is 253 characters
    (b"12345678901234567890123456789012345678901234567890.12345678901234567890123456789012345678901234567890.12345678901234567890123456789012345678901234567890.12345678901234567890123456789012345678901234567890.123456789012345678901234567890123456789012345678a",
     true),
    (b"12345678901234567890123456789012345678901234567890.12345678901234567890123456789012345678901234567890.12345678901234567890123456789012345678901234567890.12345678901234567890123456789012345678901234567890.1234567890123456789012345678901234567890123456789a",
     false),
];

// (IP address, is valid DNS name). The comments here refer to the validity of
// the string as an IP address, not as a DNS name validity.
static IP_ADDRESS_DNS_VALIDITY: &[(&[u8], bool)] = &[
    (b"", false),
    (b"1", false),
    (b"1.2", false),
    (b"1.2.3", false),
    (b"1.2.3.4", false),
    (b"1.2.3.4.5", false),
    (b"1.2.3.4a", true), // a DNS name!
    (b"a.2.3.4", false), // not even a DNS name!
    (b"1::2", false),    // IPv6 address
    // Whitespace not allowed
    (b" 1.2.3.4", false),
    (b"1.2.3.4 ", false),
    (b"1 .2.3.4", false),
    (b"\n1.2.3.4", false),
    (b"1.2.3.4\n", false),
    // Nulls not allowed
    (b"\x00", false),
    (b"\x001.2.3.4", false),
    (b"1.2.3.4\x00", false),
    (b"1.2.3.4\x00.5", false),
    // Range
    (b"0.0.0.0", false),
    (b"255.255.255.255", false),
    (b"256.0.0.0", false),
    (b"0.256.0.0", false),
    (b"0.0.256.0", false),
    (b"0.0.0.256", false),
    (b"999.0.0.0", false),
    (b"9999999999999999999.0.0.0", false),
    // All digits allowed
    (b"0.1.2.3", false),
    (b"4.5.6.7", false),
    (b"8.9.0.1", false),
    // Leading zeros not allowed
    (b"01.2.3.4", false),
    (b"001.2.3.4", false),
    (b"00000000001.2.3.4", false),
    (b"010.2.3.4", false),
    (b"1.02.3.4", false),
    (b"1.2.03.4", false),
    (b"1.2.3.04", false),
    // Empty components
    (b".2.3.4", false),
    (b"1..3.4", false),
    (b"1.2..4", false),
    (b"1.2.3.", false),
    // Too many components
    (b"1.2.3.4.5", false),
    (b"1.2.3.4.5.6", false),
    (b"0.1.2.3.4", false),
    (b"1.2.3.4.0", false),
    // Leading/trailing dot
    (b".1.2.3.4", false),
    (b"1.2.3.4.", false),
    // Other common forms of IPv4 address
    // http://en.wikipedia.org/wiki/IPv4#Address_representations
    (b"192.0.2.235", false),         // dotted decimal (control value)
    (b"0xC0.0x00.0x02.0xEB", true),  // dotted hex - actually a DNS name!
    (b"0301.0000.0002.0353", false), // dotted octal
    (b"0xC00002EB", true),           // non-dotted hex, actually a DNS name!
    (b"3221226219", false),          // non-dotted decimal
    (b"030000001353", false),        // non-dotted octal
    (b"192.0.0002.0xEB", true),      // mixed, actually a DNS name!
    (b"1234", false),
    (b"1234:5678", false),
    (b"1234:5678:9abc", false),
    (b"1234:5678:9abc:def0", false),
    (b"1234:5678:9abc:def0:1234:", false),
    (b"1234:5678:9abc:def0:1234:5678:", false),
    (b"1234:5678:9abc:def0:1234:5678:9abc:", false),
    (b"1234:5678:9abc:def0:1234:5678:9abc:def0", false),
    (b"1234:5678:9abc:def0:1234:5678:9abc:def0:", false),
    (b":1234:5678:9abc:def0:1234:5678:9abc:def0", false),
    (b"1234:5678:9abc:def0:1234:5678:9abc:def0:0000", false),
    // Valid contractions
    (b"::1", false),
    (b"::1234", false),
    (b"1234::", false),
    (b"1234::5678", false),
    (b"1234:5678::abcd", false),
    (b"1234:5678:9abc:def0:1234:5678:9abc::", false),
    // Contraction in full IPv6 addresses not allowed
    (b"::1234:5678:9abc:def0:1234:5678:9abc:def0", false), // start
    (b"1234:5678:9abc:def0:1234:5678:9abc:def0::", false), // end
    (b"1234:5678::9abc:def0:1234:5678:9abc:def0", false),  // interior
    // Multiple contractions not allowed
    (b"::1::", false),
    (b"::1::2", false),
    (b"1::2::", false),
    // Colon madness!
    (b":", false),
    (b"::", false),
    (b":::", false),
    (b"::::", false),
    (b":::1", false),
    (b"::::1", false),
    (b"1:::2", false),
    (b"1::::2", false),
    (b"1:2:::", false),
    (b"1:2::::", false),
    (b"::1234:", false),
    (b":1234::", false),
    (b"01234::", false),    // too many digits, even if zero
    (b"12345678::", false), // too many digits or missing colon
    // uppercase
    (b"ABCD:EFAB::", false),
    // miXeD CAse
    (b"aBcd:eFAb::", false),
    // IPv4-style
    (b"::2.3.4.5", false),
    (b"1234::2.3.4.5", false),
    (b"::abcd:2.3.4.5", false),
    (b"1234:5678:9abc:def0:1234:5678:252.253.254.255", false),
    (b"1234:5678:9abc:def0:1234::252.253.254.255", false),
    (b"1234::252.253.254", false),
    (b"::252.253.254", false),
    (b"::252.253.254.300", false),
    (b"1234::252.253.254.255:", false),
    (b"1234::252.253.254.255:5678", false),
    // Contractions that don't contract
    (b"::1234:5678:9abc:def0:1234:5678:9abc:def0", false),
    (b"1234:5678:9abc:def0:1234:5678:9abc:def0::", false),
    (b"1234:5678:9abc:def0::1234:5678:9abc:def0", false),
    (b"1234:5678:9abc:def0:1234:5678::252.253.254.255", false),
    // With and without leading zeros
    (b"::123", false),
    (b"::0123", false),
    (b"::012", false),
    (b"::0012", false),
    (b"::01", false),
    (b"::001", false),
    (b"::0001", false),
    (b"::0", false),
    (b"::00", false),
    (b"::000", false),
    (b"::0000", false),
    (b"::01234", false),
    (b"::00123", false),
    (b"::000123", false),
    // Trailing zero
    (b"::12340", false),
    // Whitespace
    (b" 1234:5678:9abc:def0:1234:5678:9abc:def0", false),
    (b"\t1234:5678:9abc:def0:1234:5678:9abc:def0", false),
    (b"\t1234:5678:9abc:def0:1234:5678:9abc:def0\n", false),
    (b"1234 :5678:9abc:def0:1234:5678:9abc:def0", false),
    (b"1234: 5678:9abc:def0:1234:5678:9abc:def0", false),
    (b":: 2.3.4.5", false),
    (b"1234::252.253.254.255 ", false),
    (b"1234::252.253.254.255\n", false),
    (b"1234::252.253. 254.255", false),
    // Nulls
    (b"\x00", false),
    (b"::1\x00:2", false),
    (b"::1\x00", false),
    (b"::1.2.3.4\x00", false),
    (b"::1.2\x002.3.4", false),
];

#[test]
fn dns_name_ref_try_from_ascii_test() {
    for &(s, is_valid) in DNS_NAME_VALIDITY
        .iter()
        .chain(IP_ADDRESS_DNS_VALIDITY.iter())
    {
        assert_eq!(
            DnsName::try_from(s).is_ok(),
            is_valid,
            "DnsNameRef::try_from_ascii_str failed for \"{:?}\"",
            s
        );
    }
}
