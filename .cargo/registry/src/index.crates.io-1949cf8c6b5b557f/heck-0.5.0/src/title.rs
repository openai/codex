use core::fmt;

use alloc::{
    borrow::ToOwned,
    string::{String, ToString},
};

use crate::{capitalize, transform};

/// This trait defines a title case conversion.
///
/// In Title Case, word boundaries are indicated by spaces, and every word is
/// capitalized.
///
/// ## Example:
///
/// ```rust
/// use heck::ToTitleCase;
///
/// let sentence = "We have always lived in slums and holes in the wall.";
/// assert_eq!(sentence.to_title_case(), "We Have Always Lived In Slums And Holes In The Wall");
/// ```
pub trait ToTitleCase: ToOwned {
    /// Convert this type to title case.
    fn to_title_case(&self) -> Self::Owned;
}

impl ToTitleCase for str {
    fn to_title_case(&self) -> String {
        AsTitleCase(self).to_string()
    }
}

/// This wrapper performs a title case conversion in [`fmt::Display`].
///
/// ## Example:
///
/// ```
/// use heck::AsTitleCase;
///
/// let sentence = "We have always lived in slums and holes in the wall.";
/// assert_eq!(format!("{}", AsTitleCase(sentence)), "We Have Always Lived In Slums And Holes In The Wall");
/// ```
pub struct AsTitleCase<T: AsRef<str>>(pub T);

impl<T: AsRef<str>> fmt::Display for AsTitleCase<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        transform(self.0.as_ref(), capitalize, |f| write!(f, " "), f)
    }
}

#[cfg(test)]
mod tests {
    use super::ToTitleCase;

    macro_rules! t {
        ($t:ident : $s1:expr => $s2:expr) => {
            #[test]
            fn $t() {
                assert_eq!($s1.to_title_case(), $s2)
            }
        };
    }

    t!(test1: "CamelCase" => "Camel Case");
    t!(test2: "This is Human case." => "This Is Human Case");
    t!(test3: "MixedUP CamelCase, with some Spaces" => "Mixed Up Camel Case With Some Spaces");
    t!(test4: "mixed_up_ snake_case, with some _spaces" => "Mixed Up Snake Case With Some Spaces");
    t!(test5: "kebab-case" => "Kebab Case");
    t!(test6: "SHOUTY_SNAKE_CASE" => "Shouty Snake Case");
    t!(test7: "snake_case" => "Snake Case");
    t!(test8: "this-contains_ ALLKinds OfWord_Boundaries" => "This Contains All Kinds Of Word Boundaries");
    t!(test9: "XΣXΣ baﬄe" => "Xσxς Baﬄe");
    t!(test10: "XMLHttpRequest" => "Xml Http Request");
}
