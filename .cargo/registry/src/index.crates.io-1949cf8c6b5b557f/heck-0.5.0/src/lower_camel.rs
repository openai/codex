use core::fmt;

use alloc::{
    borrow::ToOwned,
    string::{String, ToString},
};

use crate::{capitalize, lowercase, transform};

/// This trait defines a lower camel case conversion.
///
/// In lowerCamelCase, word boundaries are indicated by capital letters,
/// excepting the first word.
///
/// ## Example:
///
/// ```rust
/// use heck::ToLowerCamelCase;
///
/// let sentence = "It is we who built these palaces and cities.";
/// assert_eq!(sentence.to_lower_camel_case(), "itIsWeWhoBuiltThesePalacesAndCities");
/// ```
pub trait ToLowerCamelCase: ToOwned {
    /// Convert this type to lower camel case.
    fn to_lower_camel_case(&self) -> Self::Owned;
}

impl ToLowerCamelCase for str {
    fn to_lower_camel_case(&self) -> String {
        AsLowerCamelCase(self).to_string()
    }
}

/// This wrapper performs a lower camel case conversion in [`fmt::Display`].
///
/// ## Example:
///
/// ```
/// use heck::AsLowerCamelCase;
///
/// let sentence = "It is we who built these palaces and cities.";
/// assert_eq!(format!("{}", AsLowerCamelCase(sentence)), "itIsWeWhoBuiltThesePalacesAndCities");
/// ```
pub struct AsLowerCamelCase<T: AsRef<str>>(pub T);

impl<T: AsRef<str>> fmt::Display for AsLowerCamelCase<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut first = true;
        transform(
            self.0.as_ref(),
            |s, f| {
                if first {
                    first = false;
                    lowercase(s, f)
                } else {
                    capitalize(s, f)
                }
            },
            |_| Ok(()),
            f,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::ToLowerCamelCase;

    macro_rules! t {
        ($t:ident : $s1:expr => $s2:expr) => {
            #[test]
            fn $t() {
                assert_eq!($s1.to_lower_camel_case(), $s2)
            }
        };
    }

    t!(test1: "CamelCase" => "camelCase");
    t!(test2: "This is Human case." => "thisIsHumanCase");
    t!(test3: "MixedUP CamelCase, with some Spaces" => "mixedUpCamelCaseWithSomeSpaces");
    t!(test4: "mixed_up_ snake_case, with some _spaces" => "mixedUpSnakeCaseWithSomeSpaces");
    t!(test5: "kebab-case" => "kebabCase");
    t!(test6: "SHOUTY_SNAKE_CASE" => "shoutySnakeCase");
    t!(test7: "snake_case" => "snakeCase");
    t!(test8: "this-contains_ ALLKinds OfWord_Boundaries" => "thisContainsAllKindsOfWordBoundaries");
    t!(test9: "XΣXΣ baﬄe" => "xσxςBaﬄe");
    t!(test10: "XMLHttpRequest" => "xmlHttpRequest");
}
