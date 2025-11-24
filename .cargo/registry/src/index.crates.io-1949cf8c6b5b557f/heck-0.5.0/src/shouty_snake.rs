use core::fmt;

use alloc::{borrow::ToOwned, string::ToString};

use crate::{transform, uppercase};

/// This trait defines a shouty snake case conversion.
///
/// In SHOUTY_SNAKE_CASE, word boundaries are indicated by underscores and all
/// words are in uppercase.
///
/// ## Example:
///
/// ```rust
/// use heck::ToShoutySnakeCase;
///
/// let sentence = "That world is growing in this minute.";
/// assert_eq!(sentence.to_shouty_snake_case(), "THAT_WORLD_IS_GROWING_IN_THIS_MINUTE");
/// ```
pub trait ToShoutySnakeCase: ToOwned {
    /// Convert this type to shouty snake case.
    fn to_shouty_snake_case(&self) -> Self::Owned;
}

/// Oh heck, `ToShoutySnekCase` is an alias for [`ToShoutySnakeCase`]. See
/// ToShoutySnakeCase for more documentation.
pub trait ToShoutySnekCase: ToOwned {
    /// CONVERT THIS TYPE TO SNEK CASE.
    #[allow(non_snake_case)]
    fn TO_SHOUTY_SNEK_CASE(&self) -> Self::Owned;
}

impl<T: ?Sized + ToShoutySnakeCase> ToShoutySnekCase for T {
    fn TO_SHOUTY_SNEK_CASE(&self) -> Self::Owned {
        self.to_shouty_snake_case()
    }
}

impl ToShoutySnakeCase for str {
    fn to_shouty_snake_case(&self) -> Self::Owned {
        AsShoutySnakeCase(self).to_string()
    }
}

/// This wrapper performs a shouty snake  case conversion in [`fmt::Display`].
///
/// ## Example:
///
/// ```
/// use heck::AsShoutySnakeCase;
///
/// let sentence = "That world is growing in this minute.";
/// assert_eq!(format!("{}", AsShoutySnakeCase(sentence)), "THAT_WORLD_IS_GROWING_IN_THIS_MINUTE");
/// ```
pub struct AsShoutySnakeCase<T: AsRef<str>>(pub T);

impl<T: AsRef<str>> fmt::Display for AsShoutySnakeCase<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        transform(self.0.as_ref(), uppercase, |f| write!(f, "_"), f)
    }
}

#[cfg(test)]
mod tests {
    use super::ToShoutySnakeCase;

    macro_rules! t {
        ($t:ident : $s1:expr => $s2:expr) => {
            #[test]
            fn $t() {
                assert_eq!($s1.to_shouty_snake_case(), $s2)
            }
        };
    }

    t!(test1: "CamelCase" => "CAMEL_CASE");
    t!(test2: "This is Human case." => "THIS_IS_HUMAN_CASE");
    t!(test3: "MixedUP CamelCase, with some Spaces" => "MIXED_UP_CAMEL_CASE_WITH_SOME_SPACES");
    t!(test4: "mixed_up_snake_case with some _spaces" => "MIXED_UP_SNAKE_CASE_WITH_SOME_SPACES");
    t!(test5: "kebab-case" => "KEBAB_CASE");
    t!(test6: "SHOUTY_SNAKE_CASE" => "SHOUTY_SNAKE_CASE");
    t!(test7: "snake_case" => "SNAKE_CASE");
    t!(test8: "this-contains_ ALLKinds OfWord_Boundaries" => "THIS_CONTAINS_ALL_KINDS_OF_WORD_BOUNDARIES");
    t!(test9: "XΣXΣ baﬄe" => "XΣXΣ_BAFFLE");
    t!(test10: "XMLHttpRequest" => "XML_HTTP_REQUEST");
}
