use core::fmt;

use alloc::{borrow::ToOwned, string::ToString};

use crate::{capitalize, transform};

/// This trait defines a train case conversion.
///
/// In Train-Case, word boundaries are indicated by hyphens and words start
/// with Capital Letters.
///
/// ## Example:
///
/// ```rust
/// use heck::ToTrainCase;
///
/// let sentence = "We are going to inherit the earth.";
/// assert_eq!(sentence.to_train_case(), "We-Are-Going-To-Inherit-The-Earth");
/// ```
pub trait ToTrainCase: ToOwned {
    /// Convert this type to Train-Case.
    fn to_train_case(&self) -> Self::Owned;
}

impl ToTrainCase for str {
    fn to_train_case(&self) -> Self::Owned {
        AsTrainCase(self).to_string()
    }
}

/// This wrapper performs a train case conversion in [`fmt::Display`].
///
/// ## Example:
///
/// ```
/// use heck::AsTrainCase;
///
/// let sentence = "We are going to inherit the earth.";
/// assert_eq!(format!("{}", AsTrainCase(sentence)), "We-Are-Going-To-Inherit-The-Earth");
/// ```
pub struct AsTrainCase<T: AsRef<str>>(pub T);

impl<T: AsRef<str>> fmt::Display for AsTrainCase<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        transform(self.0.as_ref(), capitalize, |f| write!(f, "-"), f)
    }
}

#[cfg(test)]
mod tests {
    use super::ToTrainCase;

    macro_rules! t {
        ($t:ident : $s1:expr => $s2:expr) => {
            #[test]
            fn $t() {
                assert_eq!($s1.to_train_case(), $s2)
            }
        };
    }

    t!(test1: "CamelCase" => "Camel-Case");
    t!(test2: "This is Human case." => "This-Is-Human-Case");
    t!(test3: "MixedUP CamelCase, with some Spaces" => "Mixed-Up-Camel-Case-With-Some-Spaces");
    t!(test4: "mixed_up_ snake_case with some _spaces" => "Mixed-Up-Snake-Case-With-Some-Spaces");
    t!(test5: "kebab-case" => "Kebab-Case");
    t!(test6: "SHOUTY_SNAKE_CASE" => "Shouty-Snake-Case");
    t!(test7: "snake_case" => "Snake-Case");
    t!(test8: "this-contains_ ALLKinds OfWord_Boundaries" => "This-Contains-All-Kinds-Of-Word-Boundaries");
    #[cfg(feature = "unicode")]
    t!(test9: "XΣXΣ baﬄe" => "Xσxς-Baﬄe");
    t!(test10: "XMLHttpRequest" => "Xml-Http-Request");
    t!(test11: "FIELD_NAME11" => "Field-Name11");
    t!(test12: "99BOTTLES" => "99bottles");
    t!(test13: "FieldNamE11" => "Field-Nam-E11");
    t!(test14: "abc123def456" => "Abc123def456");
    t!(test16: "abc123DEF456" => "Abc123-Def456");
    t!(test17: "abc123Def456" => "Abc123-Def456");
    t!(test18: "abc123DEf456" => "Abc123-D-Ef456");
    t!(test19: "ABC123def456" => "Abc123def456");
    t!(test20: "ABC123DEF456" => "Abc123def456");
    t!(test21: "ABC123Def456" => "Abc123-Def456");
    t!(test22: "ABC123DEf456" => "Abc123d-Ef456");
    t!(test23: "ABC123dEEf456FOO" => "Abc123d-E-Ef456-Foo");
    t!(test24: "abcDEF" => "Abc-Def");
    t!(test25: "ABcDE" => "A-Bc-De");
}
