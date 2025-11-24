#[cfg(test)]
use strum_macros::EnumIter;

use crate::pattern::Pattern;
use crate::Boundary;

/// Defines the type of casing a string can be.
///
/// ```
/// use convert_case::{Case, Casing};
///
/// let super_mario_title: String = "super_mario_64".to_case(Case::Title);
/// assert_eq!("Super Mario 64", super_mario_title);
/// ```
///
/// A case is the pair of a [pattern](enum.Pattern.html) and a delimeter (a string).  Given
/// a list of words, a pattern describes how to mutate the words and a delimeter is how the mutated
/// words are joined together.  These inherantly are the properties of what makes a "multiword
/// identifier case", or simply "case".
///
/// This crate provides the ability to convert "from" a case.  This introduces a different feature
/// of cases which are the [word boundaries](Boundary) that segment the identifier into words.  For example, a
/// snake case identifier `my_var_name` can be split on underscores `_` to segment into words.  A
/// camel case identifier `myVarName` is split where a lowercase letter is followed by an
/// uppercase letter.  Each case is also associated with a list of boundaries that are used when
/// converting "from" a particular case.
#[cfg_attr(test, derive(EnumIter))]
#[derive(Eq, PartialEq, Hash, Clone, Copy, Debug)]
pub enum Case {
    /// Uppercase strings are delimited by spaces and all characters are uppercase.
    /// * Boundaries: [Space](`Boundary::Space`)
    /// * Pattern: [Uppercase](`Pattern::Uppercase`)
    /// * Delimeter: Space
    ///
    /// ```
    /// use convert_case::{Case, Casing};
    /// assert_eq!("MY VARIABLE NAME", "My variable NAME".to_case(Case::Upper))
    /// ```
    Upper,

    /// Lowercase strings are delimited by spaces and all characters are lowercase.
    /// * Boundaries: [Space](`Boundary::Space`)
    /// * Pattern: [Lowercase](`Pattern::Lowercase`)
    /// * Delimeter: Space
    ///
    /// ```
    /// use convert_case::{Case, Casing};
    /// assert_eq!("my variable name", "My variable NAME".to_case(Case::Lower))
    /// ```
    Lower,

    /// Title case strings are delimited by spaces. Only the leading character of
    /// each word is uppercase.  No inferences are made about language, so words
    /// like "as", "to", and "for" will still be capitalized.
    /// * Boundaries: [Space](`Boundary::Space`)
    /// * Pattern: [Capital](`Pattern::Capital`)
    /// * Delimeter: Space
    ///
    /// ```
    /// use convert_case::{Case, Casing};
    /// assert_eq!("My Variable Name", "My variable NAME".to_case(Case::Title))
    /// ```
    Title,

    /// Toggle case strings are delimited by spaces.  All characters are uppercase except
    /// for the leading character of each word, which is lowercase.
    /// * Boundaries: [Space](`Boundary::Space`)
    /// * Pattern: [Toggle](`Pattern::Toggle`)
    /// * Delimeter: Space
    ///
    /// ```
    /// use convert_case::{Case, Casing};
    /// assert_eq!("mY vARIABLE nAME", "My variable NAME".to_case(Case::Toggle))
    /// ```
    Toggle,

    /// Camel case strings are lowercase, but for every word _except the first_ the
    /// first letter is capitalized.
    /// * Boundaries: [LowerUpper](Boundary::LowerUpper), [DigitUpper](Boundary::DigitUpper),
    /// [UpperDigit](Boundary::UpperDigit), [DigitLower](Boundary::DigitLower),
    /// [LowerDigit](Boundary::LowerDigit), [Acronym](Boundary::Acronym)
    /// * Pattern: [Camel](`Pattern::Camel`)
    /// * Delimeter: No delimeter
    ///
    /// ```
    /// use convert_case::{Case, Casing};
    /// assert_eq!("myVariableName", "My variable NAME".to_case(Case::Camel))
    /// ```
    Camel,

    /// Pascal case strings are lowercase, but for every word the
    /// first letter is capitalized.
    /// * Boundaries: [LowerUpper](Boundary::LowerUpper), [DigitUpper](Boundary::DigitUpper),
    /// [UpperDigit](Boundary::UpperDigit), [DigitLower](Boundary::DigitLower),
    /// [LowerDigit](Boundary::LowerDigit), [Acronym](Boundary::Acronym)
    /// * Pattern: [Capital](`Pattern::Capital`)
    /// * Delimeter: No delimeter
    ///
    /// ```
    /// use convert_case::{Case, Casing};
    /// assert_eq!("MyVariableName", "My variable NAME".to_case(Case::Pascal))
    /// ```
    Pascal,

    /// Upper camel case is an alternative name for [Pascal case](Case::Pascal).
    UpperCamel,

    /// Snake case strings are delimited by underscores `_` and are all lowercase.
    /// * Boundaries: [Underscore](Boundary::Underscore)
    /// * Pattern: [Lowercase](Pattern::Lowercase)
    /// * Delimeter: Underscore `_`
    ///
    /// ```
    /// use convert_case::{Case, Casing};
    /// assert_eq!("my_variable_name", "My variable NAME".to_case(Case::Snake))
    /// ```
    Snake,

    /// Upper snake case strings are delimited by underscores `_` and are all uppercase.
    /// * Boundaries: [Underscore](Boundary::Underscore)
    /// * Pattern: [Uppercase](Pattern::Uppercase)
    /// * Delimeter: Underscore `_`
    ///
    /// ```
    /// use convert_case::{Case, Casing};
    /// assert_eq!("MY_VARIABLE_NAME", "My variable NAME".to_case(Case::UpperSnake))
    /// ```
    UpperSnake,

    /// Screaming snake case is an alternative name for [upper snake case](Case::UpperSnake).
    ScreamingSnake,

    /// Kebab case strings are delimited by hyphens `-` and are all lowercase.
    /// * Boundaries: [Hyphen](Boundary::Hyphen)
    /// * Pattern: [Lowercase](Pattern::Lowercase)
    /// * Delimeter: Hyphen `-`
    ///
    /// ```
    /// use convert_case::{Case, Casing};
    /// assert_eq!("my-variable-name", "My variable NAME".to_case(Case::Kebab))
    /// ```
    Kebab,

    /// Cobol case strings are delimited by hyphens `-` and are all uppercase.
    /// * Boundaries: [Hyphen](Boundary::Hyphen)
    /// * Pattern: [Uppercase](Pattern::Uppercase)
    /// * Delimeter: Hyphen `-`
    ///
    /// ```
    /// use convert_case::{Case, Casing};
    /// assert_eq!("MY-VARIABLE-NAME", "My variable NAME".to_case(Case::Cobol))
    /// ```
    Cobol,

    /// Upper kebab case is an alternative name for [Cobol case](Case::Cobol).
    UpperKebab,

    /// Train case strings are delimited by hyphens `-`.  All characters are lowercase
    /// except for the leading character of each word.
    /// * Boundaries: [Hyphen](Boundary::Hyphen)
    /// * Pattern: [Capital](Pattern::Capital)
    /// * Delimeter: Hyphen `-`
    ///
    /// ```
    /// use convert_case::{Case, Casing};
    /// assert_eq!("My-Variable-Name", "My variable NAME".to_case(Case::Train))
    /// ```
    Train,

    /// Flat case strings are all lowercase, with no delimiter. Note that word boundaries are lost.
    /// * Boundaries: No boundaries
    /// * Pattern: [Lowercase](Pattern::Lowercase)
    /// * Delimeter: No delimeter
    ///
    /// ```
    /// use convert_case::{Case, Casing};
    /// assert_eq!("myvariablename", "My variable NAME".to_case(Case::Flat))
    /// ```
    Flat,

    /// Upper flat case strings are all uppercase, with no delimiter. Note that word boundaries are lost.
    /// * Boundaries: No boundaries
    /// * Pattern: [Uppercase](Pattern::Uppercase)
    /// * Delimeter: No delimeter
    ///
    /// ```
    /// use convert_case::{Case, Casing};
    /// assert_eq!("MYVARIABLENAME", "My variable NAME".to_case(Case::UpperFlat))
    /// ```
    UpperFlat,

    /// Alternating case strings are delimited by spaces.  Characters alternate between uppercase
    /// and lowercase.
    /// * Boundaries: [Space](Boundary::Space)
    /// * Pattern: [Alternating](Pattern::Alternating)
    /// * Delimeter: Space
    ///
    /// ```
    /// use convert_case::{Case, Casing};
    /// assert_eq!("mY vArIaBlE nAmE", "My variable NAME".to_case(Case::Alternating));
    /// ```
    Alternating,

    /// Random case strings are delimited by spaces and characters are
    /// randomly upper case or lower case.  This uses the `rand` crate
    /// and is only available with the "random" feature.
    /// * Boundaries: [Space](Boundary::Space)
    /// * Pattern: [Random](Pattern::Random)
    /// * Delimeter: Space
    ///
    /// ```
    /// use convert_case::{Case, Casing};
    /// let new = "My variable NAME".to_case(Case::Random);
    /// ```
    /// String `new` could be "My vaRIAbLE nAme" for example.
    #[cfg(any(doc, feature = "random"))]
    Random,

    /// Pseudo-random case strings are delimited by spaces and characters are randomly
    /// upper case or lower case, but there will never more than two consecutive lower
    /// case or upper case letters in a row.  This uses the `rand` crate and is
    /// only available with the "random" feature.
    /// * Boundaries: [Space](Boundary::Space)
    /// * Pattern: [PseudoRandom](Pattern::PseudoRandom)
    /// * Delimeter: Space
    ///
    /// ```
    /// use convert_case::{Case, Casing};
    /// let new = "My variable NAME".to_case(Case::Random);
    /// ```
    /// String `new` could be "mY vArIAblE NamE" for example.
    #[cfg(any(doc, feature = "random"))]
    PseudoRandom,
}

impl Case {
    /// Returns the delimiter used in the corresponding case.  The following
    /// table outlines which cases use which delimeter.
    ///
    /// | Cases | Delimeter |
    /// | --- | --- |
    /// | Upper, Lower, Title, Toggle, Alternating, Random, PseudoRandom | Space |
    /// | Snake, UpperSnake, ScreamingSnake | Underscore `_` |
    /// | Kebab, Cobol, UpperKebab, Train | Hyphen `-` |
    /// | UpperFlat, Flat, Camel, UpperCamel, Pascal | Empty string, no delimeter |
    pub const fn delim(&self) -> &'static str {
        use Case::*;
        match self {
            Upper | Lower | Title | Toggle | Alternating => " ",
            Snake | UpperSnake | ScreamingSnake => "_",
            Kebab | Cobol | UpperKebab | Train => "-",

            #[cfg(feature = "random")]
            Random | PseudoRandom => " ",

            UpperFlat | Flat | Camel | UpperCamel | Pascal => "",
        }
    }

    /// Returns the pattern used in the corresponding case.  The following
    /// table outlines which cases use which pattern.
    ///
    /// | Cases | Pattern |
    /// | --- | --- |
    /// | Upper, UpperSnake, ScreamingSnake, UpperFlat, Cobol, UpperKebab | Uppercase |
    /// | Lower, Snake, Kebab, Flat | Lowercase |
    /// | Title, Pascal, UpperCamel, Train | Capital |
    /// | Camel | Camel |
    /// | Alternating | Alternating |
    /// | Random | Random |
    /// | PseudoRandom | PseudoRandom |
    pub const fn pattern(&self) -> Pattern {
        use Case::*;
        match self {
            Upper | UpperSnake | ScreamingSnake | UpperFlat | Cobol | UpperKebab => {
                Pattern::Uppercase
            }
            Lower | Snake | Kebab | Flat => Pattern::Lowercase,
            Title | Pascal | UpperCamel | Train => Pattern::Capital,
            Camel => Pattern::Camel,
            Toggle => Pattern::Toggle,
            Alternating => Pattern::Alternating,

            #[cfg(feature = "random")]
            Random => Pattern::Random,
            #[cfg(feature = "random")]
            PseudoRandom => Pattern::PseudoRandom,
        }
    }

    /// Returns the boundaries used in the corresponding case.  That is, where can word boundaries
    /// be distinguished in a string of the given case.  The table outlines which cases use which
    /// set of boundaries.
    ///
    /// | Cases | Boundaries |
    /// | --- | --- |
    /// | Upper, Lower, Title, Toggle, Alternating, Random, PseudoRandom | Space |
    /// | Snake, UpperSnake, ScreamingSnake | Underscore `_` |
    /// | Kebab, Cobol, UpperKebab, Train | Hyphen `-` |
    /// | Camel, UpperCamel, Pascal | LowerUpper, LowerDigit, UpperDigit, DigitLower, DigitUpper, Acronym |
    /// | UpperFlat, Flat | No boundaries |
    pub fn boundaries(&self) -> Vec<Boundary> {
        use Boundary::*;
        use Case::*;
        match self {
            Upper | Lower | Title | Toggle | Alternating => vec![Space],
            Snake | UpperSnake | ScreamingSnake => vec![Underscore],
            Kebab | Cobol | UpperKebab | Train => vec![Hyphen],

            #[cfg(feature = "random")]
            Random | PseudoRandom => vec![Space],

            UpperFlat | Flat => vec![],
            Camel | UpperCamel | Pascal => vec![
                LowerUpper, Acronym, LowerDigit, UpperDigit, DigitLower, DigitUpper,
            ],
        }
    }

    // Created to avoid using the EnumIter trait from strum in
    // final library.  A test confirms that all cases are listed here.
    /// Returns a vector with all case enum variants in no particular order.
    pub fn all_cases() -> Vec<Case> {
        use Case::*;
        vec![
            Upper,
            Lower,
            Title,
            Toggle,
            Camel,
            Pascal,
            UpperCamel,
            Snake,
            UpperSnake,
            ScreamingSnake,
            Kebab,
            Cobol,
            UpperKebab,
            Train,
            Flat,
            UpperFlat,
            Alternating,
            #[cfg(feature = "random")]
            Random,
            #[cfg(feature = "random")]
            PseudoRandom,
        ]
    }

    /// Returns a vector with the two "random" feature cases `Random` and `PseudoRandom`.  Only
    /// defined in the "random" feature.
    #[cfg(feature = "random")]
    pub fn random_cases() -> Vec<Case> {
        use Case::*;
        vec![Random, PseudoRandom]
    }

    /// Returns a vector with all the cases that do not depend on randomness.  This is all
    /// the cases not in the "random" feature.
    pub fn deterministic_cases() -> Vec<Case> {
        use Case::*;
        vec![
            Upper,
            Lower,
            Title,
            Toggle,
            Camel,
            Pascal,
            UpperCamel,
            Snake,
            UpperSnake,
            ScreamingSnake,
            Kebab,
            Cobol,
            UpperKebab,
            Train,
            Flat,
            UpperFlat,
            Alternating,
        ]
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use strum::IntoEnumIterator;

    #[test]
    fn all_cases_in_iter() {
        let all = Case::all_cases();
        for case in Case::iter() {
            assert!(all.contains(&case));
        }
    }
}
