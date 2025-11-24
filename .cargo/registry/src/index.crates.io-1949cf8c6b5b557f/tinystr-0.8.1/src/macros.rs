// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

#[macro_export]
macro_rules! tinystr {
    ($n:literal, $s:literal) => {{
        // Force it into a const context; otherwise it may get evaluated at runtime instead.
        const TINYSTR_MACRO_CONST: $crate::TinyAsciiStr<$n> = {
            match $crate::TinyAsciiStr::try_from_utf8($s.as_bytes()) {
                Ok(s) => s,
                // We are okay with panicking here because this is in a const context
                #[allow(clippy::panic)]
                // Cannot format the error since formatting isn't const yet
                Err(_) => panic!(concat!("Failed to construct tinystr from ", $s)),
            }
        };
        TINYSTR_MACRO_CONST
    }};
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_macro_construction() {
        let s1 = tinystr!(8, "foobar");
        assert_eq!(&*s1, "foobar");

        let s1 = tinystr!(12, "foobarbaz");
        assert_eq!(&*s1, "foobarbaz");
    }
}
