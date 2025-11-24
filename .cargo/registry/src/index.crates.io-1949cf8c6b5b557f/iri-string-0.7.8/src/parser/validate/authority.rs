//! Parsers for authority.

use core::mem;

use crate::parser::char;
use crate::parser::str::{
    find_split_hole, get_wrapped_inner, rfind_split_hole, satisfy_chars_with_pct_encoded,
    strip_ascii_char_prefix,
};
use crate::spec::Spec;
use crate::validate::Error;

/// Returns `Ok(_)` if the string matches `userinfo` or `iuserinfo`.
pub(crate) fn validate_userinfo<S: Spec>(i: &str) -> Result<(), Error> {
    let is_valid = satisfy_chars_with_pct_encoded(
        i,
        char::is_ascii_userinfo_ipvfutureaddr,
        char::is_nonascii_userinfo::<S>,
    );
    if is_valid {
        Ok(())
    } else {
        Err(Error::new())
    }
}

/// Returns `true` if the string matches `dec-octet`.
///
/// In other words, this tests whether the string is decimal "0" to "255".
#[must_use]
fn is_dec_octet(i: &str) -> bool {
    matches!(
        i.as_bytes(),
        [b'0'..=b'9']
            | [b'1'..=b'9', b'0'..=b'9']
            | [b'1', b'0'..=b'9', b'0'..=b'9']
            | [b'2', b'0'..=b'4', b'0'..=b'9']
            | [b'2', b'5', b'0'..=b'5']
    )
}

/// Returns `Ok(_)` if the string matches `IPv4address`.
fn validate_ipv4address(i: &str) -> Result<(), Error> {
    let (first, rest) = find_split_hole(i, b'.').ok_or_else(Error::new)?;
    if !is_dec_octet(first) {
        return Err(Error::new());
    }
    let (second, rest) = find_split_hole(rest, b'.').ok_or_else(Error::new)?;
    if !is_dec_octet(second) {
        return Err(Error::new());
    }
    let (third, fourth) = find_split_hole(rest, b'.').ok_or_else(Error::new)?;
    if is_dec_octet(third) && is_dec_octet(fourth) {
        Ok(())
    } else {
        Err(Error::new())
    }
}

/// A part of IPv6 addr.
#[derive(Clone, Copy)]
enum V6AddrPart {
    /// `[0-9a-fA-F]{1,4}::`.
    H16Omit,
    /// `[0-9a-fA-F]{1,4}:`.
    H16Cont,
    /// `[0-9a-fA-F]{1,4}`.
    H16End,
    /// IPv4 address.
    V4,
    /// `::`.
    Omit,
}

/// Splits the IPv6 address string into the next component and the rest substring.
fn split_v6_addr_part(i: &str) -> Result<(&str, V6AddrPart), Error> {
    debug_assert!(!i.is_empty());
    match find_split_hole(i, b':') {
        Some((prefix, rest)) => {
            if prefix.len() >= 5 {
                return Err(Error::new());
            }

            if prefix.is_empty() {
                return match strip_ascii_char_prefix(rest, b':') {
                    Some(rest) => Ok((rest, V6AddrPart::Omit)),
                    None => Err(Error::new()),
                };
            }

            // Should be `h16`.
            debug_assert!((1..=4).contains(&prefix.len()));
            if !prefix.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(Error::new());
            }
            match strip_ascii_char_prefix(rest, b':') {
                Some(rest) => Ok((rest, V6AddrPart::H16Omit)),
                None => Ok((rest, V6AddrPart::H16Cont)),
            }
        }
        None => {
            if i.len() >= 5 {
                // Possibly `IPv4address`.
                validate_ipv4address(i)?;
                return Ok(("", V6AddrPart::V4));
            }
            if i.bytes().all(|b| b.is_ascii_hexdigit()) {
                Ok(("", V6AddrPart::H16End))
            } else {
                Err(Error::new())
            }
        }
    }
}

/// Returns `Ok(_)` if the string matches `IPv6address`.
fn validate_ipv6address(mut i: &str) -> Result<(), Error> {
    let mut h16_count = 0;
    let mut is_omitted = false;
    while !i.is_empty() {
        let (rest, part) = split_v6_addr_part(i)?;
        match part {
            V6AddrPart::H16Omit => {
                h16_count += 1;
                if mem::replace(&mut is_omitted, true) {
                    // Omitted twice.
                    return Err(Error::new());
                }
            }
            V6AddrPart::H16Cont => {
                h16_count += 1;
                if rest.is_empty() {
                    // `H16Cont` cannot be the last part of an IPv6 address.
                    return Err(Error::new());
                }
            }
            V6AddrPart::H16End => {
                h16_count += 1;
                break;
            }
            V6AddrPart::V4 => {
                debug_assert!(rest.is_empty());
                h16_count += 2;
                break;
            }
            V6AddrPart::Omit => {
                if mem::replace(&mut is_omitted, true) {
                    // Omitted twice.
                    return Err(Error::new());
                }
            }
        }
        if h16_count > 8 {
            return Err(Error::new());
        }
        i = rest;
    }
    let is_valid = if is_omitted {
        h16_count < 8
    } else {
        h16_count == 8
    };
    if is_valid {
        Ok(())
    } else {
        Err(Error::new())
    }
}

/// Returns `Ok(_)` if the string matches `authority` or `iauthority`.
pub(super) fn validate_authority<S: Spec>(i: &str) -> Result<(), Error> {
    // Strip and validate `userinfo`.
    let (i, _userinfo) = match find_split_hole(i, b'@') {
        Some((maybe_userinfo, i)) => {
            validate_userinfo::<S>(maybe_userinfo)?;
            (i, Some(maybe_userinfo))
        }
        None => (i, None),
    };
    // `host` can contain colons, but `port` cannot.
    // Strip and validate `port`.
    let (maybe_host, _port) = match rfind_split_hole(i, b':') {
        Some((maybe_host, maybe_port)) => {
            if maybe_port.bytes().all(|b| b.is_ascii_digit()) {
                (maybe_host, Some(maybe_port))
            } else {
                (i, None)
            }
        }
        None => (i, None),
    };
    // Validate `host`.
    validate_host::<S>(maybe_host)
}

/// Validates `host`.
pub(crate) fn validate_host<S: Spec>(i: &str) -> Result<(), Error> {
    match get_wrapped_inner(i, b'[', b']') {
        Some(maybe_addr) => {
            // `IP-literal`.
            // Note that `v` here is case insensitive. See RFC 3987 section 3.2.2.
            if let Some(maybe_addr_rest) = strip_ascii_char_prefix(maybe_addr, b'v')
                .or_else(|| strip_ascii_char_prefix(maybe_addr, b'V'))
            {
                // `IPvFuture`.
                let (maybe_ver, maybe_addr) =
                    find_split_hole(maybe_addr_rest, b'.').ok_or_else(Error::new)?;
                // Validate version.
                if maybe_ver.is_empty() || !maybe_ver.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err(Error::new());
                }
                // Validate address.
                if !maybe_addr.is_empty()
                    && maybe_addr.is_ascii()
                    && maybe_addr
                        .bytes()
                        .all(char::is_ascii_userinfo_ipvfutureaddr)
                {
                    Ok(())
                } else {
                    Err(Error::new())
                }
            } else {
                // `IPv6address`.
                validate_ipv6address(maybe_addr)
            }
        }
        None => {
            // `IPv4address` or `reg-name`. No need to distinguish them here.
            let is_valid = satisfy_chars_with_pct_encoded(
                i,
                char::is_ascii_regname,
                char::is_nonascii_regname::<S>,
            );
            if is_valid {
                Ok(())
            } else {
                Err(Error::new())
            }
        }
    }
}

#[cfg(test)]
#[cfg(feature = "alloc")]
mod tests {
    use super::*;

    use alloc::format;

    macro_rules! assert_validate {
        ($parser:expr, $($input:expr),* $(,)?) => {{
            $({
                let input = $input;
                let input: &str = input.as_ref();
                assert!($parser(input).is_ok(), "input={:?}", input);
            })*
        }};
    }

    #[test]
    fn test_ipv6address() {
        use core::cmp::Ordering;

        assert_validate!(validate_ipv6address, "a:bB:cCc:dDdD:e:F:a:B");
        assert_validate!(validate_ipv6address, "1:1:1:1:1:1:1:1");
        assert_validate!(validate_ipv6address, "1:1:1:1:1:1:1.1.1.1");
        assert_validate!(validate_ipv6address, "2001:db8::7");

        // Generate IPv6 addresses with `::`.
        let make_sub = |n: usize| {
            let mut s = "1:".repeat(n);
            s.pop();
            s
        };
        for len_pref in 0..=7 {
            let prefix = make_sub(len_pref);
            for len_suf in 1..=(7 - len_pref) {
                assert_validate!(
                    validate_ipv6address,
                    &format!("{}::{}", prefix, make_sub(len_suf))
                );
                match len_suf.cmp(&2) {
                    Ordering::Greater => assert_validate!(
                        validate_ipv6address,
                        &format!("{}::{}:1.1.1.1", prefix, make_sub(len_suf - 2))
                    ),
                    Ordering::Equal => {
                        assert_validate!(validate_ipv6address, &format!("{}::1.1.1.1", prefix))
                    }
                    Ordering::Less => {}
                }
            }
        }
    }
}
