use core::fmt;

use alloc::{
    borrow::ToOwned,
    string::{String, ToString},
};

use crate::{capitalize, transform};

/// This trait defines an upper camel case conversion.
///
/// In UpperCamelCase, word boundaries are indicated by capital letters,
/// including the first word.
///
/// ## Example:
///
/// ```rust
/// use heck::ToUpperCamelCase;
///
/// let sentence = "We are not in the least afraid of ruins.";
/// assert_eq!(sentence.to_upper_camel_case(), "WeAreNotInTheLeastAfraidOfRuins");
/// ```
pub trait ToUpperCamelCase: ToOwned {
    /// Convert this type to upper camel case.
    fn to_upper_camel_case(&self) -> Self::Owned;
}

impl ToUpperCamelCase for str {
    fn to_upper_camel_case(&self) -> String {
        AsUpperCamelCase(self).to_string()
    }
}

/// `ToPascalCase` is an alias for [`ToUpperCamelCase`]. See ToUpperCamelCase for more
/// documentation.
pub trait ToPascalCase: ToOwned {
    /// Convert this type to upper camel case.
    fn to_pascal_case(&self) -> Self::Owned;
}

impl<T: ?Sized + ToUpperCamelCase> ToPascalCase for T {
    fn to_pascal_case(&self) -> Self::Owned {
        self.to_upper_camel_case()
    }
}

/// This wrapper performs a upper camel case conversion in [`fmt::Display`].
///
/// ## Example:
///
/// ```
/// use heck::AsUpperCamelCase;
///
/// let sentence = "We are not in the least afraid of ruins.";
/// assert_eq!(format!("{}", AsUpperCamelCase(sentence)), "WeAreNotInTheLeastAfraidOfRuins");
/// ```
pub struct AsUpperCamelCase<T: AsRef<str>>(pub T);

impl<T: AsRef<str>> fmt::Display for AsUpperCamelCase<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        transform(self.0.as_ref(), capitalize, |_| Ok(()), f)
    }
}

#[cfg(test)]
mod tests {
    use super::ToUpperCamelCase;

    macro_rules! t {
        ($t:ident : $s1:expr => $s2:expr) => {
            #[test]
            fn $t() {
                assert_eq!($s1.to_upper_camel_case(), $s2)
            }
        };
    }

    t!(test1: "CamelCase" => "CamelCase");
    t!(test2: "This is Human case." => "ThisIsHumanCase");
    t!(test3: "MixedUP_CamelCase, with some Spaces" => "MixedUpCamelCaseWithSomeSpaces");
    t!(test4: "mixed_up_ snake_case, with some _spaces" => "MixedUpSnakeCaseWithSomeSpaces");
    t!(test5: "kebab-case" => "KebabCase");
    t!(test6: "SHOUTY_SNAKE_CASE" => "ShoutySnakeCase");
    t!(test7: "snake_case" => "SnakeCase");
    t!(test8: "this-contains_ ALLKinds OfWord_Boundaries" => "ThisContainsAllKindsOfWordBoundaries");
    t!(test9: "XΣXΣ baﬄe" => "XσxςBaﬄe");
    t!(test10: "XMLHttpRequest" => "XmlHttpRequest");
}
