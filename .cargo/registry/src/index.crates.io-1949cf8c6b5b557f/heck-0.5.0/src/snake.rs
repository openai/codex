use alloc::{
    borrow::ToOwned,
    fmt,
    string::{String, ToString},
};

use crate::{lowercase, transform};

/// This trait defines a snake case conversion.
///
/// In snake_case, word boundaries are indicated by underscores.
///
/// ## Example:
///
/// ```rust
/// use heck::ToSnakeCase;
///
/// let sentence = "We carry a new world here, in our hearts.";
/// assert_eq!(sentence.to_snake_case(), "we_carry_a_new_world_here_in_our_hearts");
/// ```
pub trait ToSnakeCase: ToOwned {
    /// Convert this type to snake case.
    fn to_snake_case(&self) -> Self::Owned;
}

/// Oh heck, `SnekCase` is an alias for [`ToSnakeCase`]. See ToSnakeCase for
/// more documentation.
pub trait ToSnekCase: ToOwned {
    /// Convert this type to snek case.
    fn to_snek_case(&self) -> Self::Owned;
}

impl<T: ?Sized + ToSnakeCase> ToSnekCase for T {
    fn to_snek_case(&self) -> Self::Owned {
        self.to_snake_case()
    }
}

impl ToSnakeCase for str {
    fn to_snake_case(&self) -> String {
        AsSnakeCase(self).to_string()
    }
}

/// This wrapper performs a snake case conversion in [`fmt::Display`].
///
/// ## Example:
///
/// ```
/// use heck::AsSnakeCase;
///
/// let sentence = "We carry a new world here, in our hearts.";
/// assert_eq!(format!("{}", AsSnakeCase(sentence)), "we_carry_a_new_world_here_in_our_hearts");
/// ```
pub struct AsSnakeCase<T: AsRef<str>>(pub T);

impl<T: AsRef<str>> fmt::Display for AsSnakeCase<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        transform(self.0.as_ref(), lowercase, |f| write!(f, "_"), f)
    }
}

#[cfg(test)]
mod tests {
    use super::ToSnakeCase;

    macro_rules! t {
        ($t:ident : $s1:expr => $s2:expr) => {
            #[test]
            fn $t() {
                assert_eq!($s1.to_snake_case(), $s2)
            }
        };
    }

    t!(test1: "CamelCase" => "camel_case");
    t!(test2: "This is Human case." => "this_is_human_case");
    t!(test3: "MixedUP CamelCase, with some Spaces" => "mixed_up_camel_case_with_some_spaces");
    t!(test4: "mixed_up_ snake_case with some _spaces" => "mixed_up_snake_case_with_some_spaces");
    t!(test5: "kebab-case" => "kebab_case");
    t!(test6: "SHOUTY_SNAKE_CASE" => "shouty_snake_case");
    t!(test7: "snake_case" => "snake_case");
    t!(test8: "this-contains_ ALLKinds OfWord_Boundaries" => "this_contains_all_kinds_of_word_boundaries");
    t!(test9: "XΣXΣ baﬄe" => "xσxς_baﬄe");
    t!(test10: "XMLHttpRequest" => "xml_http_request");
    t!(test11: "FIELD_NAME11" => "field_name11");
    t!(test12: "99BOTTLES" => "99bottles");
    t!(test13: "FieldNamE11" => "field_nam_e11");
    t!(test14: "abc123def456" => "abc123def456");
    t!(test16: "abc123DEF456" => "abc123_def456");
    t!(test17: "abc123Def456" => "abc123_def456");
    t!(test18: "abc123DEf456" => "abc123_d_ef456");
    t!(test19: "ABC123def456" => "abc123def456");
    t!(test20: "ABC123DEF456" => "abc123def456");
    t!(test21: "ABC123Def456" => "abc123_def456");
    t!(test22: "ABC123DEf456" => "abc123d_ef456");
    t!(test23: "ABC123dEEf456FOO" => "abc123d_e_ef456_foo");
    t!(test24: "abcDEF" => "abc_def");
    t!(test25: "ABcDE" => "a_bc_de");
}
