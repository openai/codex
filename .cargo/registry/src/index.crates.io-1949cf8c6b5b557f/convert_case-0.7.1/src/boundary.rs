use unicode_segmentation::UnicodeSegmentation;

fn grapheme_is_digit(c: &&str) -> bool {
    c.chars().all(|c| c.is_ascii_digit())
}

fn grapheme_is_uppercase(c: &&str) -> bool {
    c.to_uppercase() != c.to_lowercase() && *c == c.to_uppercase()
}

fn grapheme_is_lowercase(c: &&str) -> bool {
    c.to_uppercase() != c.to_lowercase() && *c == c.to_lowercase()
}

/// How an identifier is split into words.  
///
/// Some boundaries, `HYPHEN`, `UNDERSCORE`, and `SPACE`, consume the character they
/// split on, whereas the other boundaries do not.
///
/// `Boundary` includes methods that return useful groups of boundaries.  It also
/// contains the [`defaults_from`](Boundary::defaults_from) method which will generate a subset
/// of default boundaries based on the boundaries present in a string.
///
/// You can also create custom delimiter boundaries using the [`from_delim`](Boundary::from_delim)
/// method or directly instantiate Boundary for complex boundary conditions.
/// ```
/// use convert_case::{Boundary, Case, Casing, Converter};
///
/// assert_eq!(
///     "transformations_in_3d",
///     "TransformationsIn3D"
///         .from_case(Case::Camel)
///         .without_boundaries(&Boundary::digit_letter())
///         .to_case(Case::Snake)
/// );
///
/// let conv = Converter::new()
///     .set_boundaries(&Boundary::defaults_from("aA "))
///     .to_case(Case::Title);
/// assert_eq!("7empest By Tool", conv.convert("7empest byTool"));
/// ```
#[derive(Debug, Eq, Hash, Clone, Copy)]
pub struct Boundary {
    /// A unique name used for comparison.
    pub name: &'static str,
    /// A function that determines if this boundary is present at the start
    /// of the string.  Second argument is the `arg` field.
    pub condition: fn(&[&str], Option<&'static str>) -> bool,
    /// An optional string passed to `condition` at runtime.  Used
    /// internally for [`Boundary::from_delim`] method.
    pub arg: Option<&'static str>,
    /// Where the beginning of the boundary is.
    pub start: usize,
    /// The length of the boundary.  This is the number of graphemes that
    /// are removed when splitting.
    pub len: usize,
}

impl PartialEq for Boundary {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Boundary {
    /// Splits on space, consuming the character on segmentation.
    /// ```
    /// # use convert_case::Boundary;
    /// assert_eq!(
    ///     vec![Boundary::SPACE],
    ///     Boundary::defaults_from(" ")
    /// );
    /// ```
    pub const SPACE: Boundary = Boundary {
        name: "Space",
        condition: |s, _| s.get(0) == Some(&" "),
        arg: None,
        start: 0,
        len: 1,
    };

    /// Splits on `-`, consuming the character on segmentation.
    /// ```
    /// # use convert_case::Boundary;
    /// assert_eq!(
    ///     vec![Boundary::HYPHEN],
    ///     Boundary::defaults_from("-")
    /// );
    /// ```
    pub const HYPHEN: Boundary = Boundary {
        name: "Hyphen",
        condition: |s, _| s.get(0) == Some(&"-"),
        arg: None,
        start: 0,
        len: 1,
    };

    /// Splits on `_`, consuming the character on segmentation.
    /// ```
    /// # use convert_case::Boundary;
    /// assert_eq!(
    ///     vec![Boundary::UNDERSCORE],
    ///     Boundary::defaults_from("_")
    /// );
    /// ```
    pub const UNDERSCORE: Boundary = Boundary {
        name: "Underscore",
        condition: |s, _| s.get(0) == Some(&"_"),
        arg: None,
        start: 0,
        len: 1,
    };

    /// Splits where a lowercase letter is followed by an uppercase letter.
    /// ```
    /// # use convert_case::Boundary;
    /// assert_eq!(
    ///     vec![Boundary::LOWER_UPPER],
    ///     Boundary::defaults_from("aA")
    /// );
    /// ```
    pub const LOWER_UPPER: Boundary = Boundary {
        name: "LowerUpper",
        condition: |s, _| {
            s.get(0).map(grapheme_is_lowercase) == Some(true)
                && s.get(1).map(grapheme_is_uppercase) == Some(true)
        },
        arg: None,
        start: 1,
        len: 0,
    };
    /// Splits where an uppercase letter is followed by a lowercase letter.  This is seldom used,
    /// and is **not** included in the [defaults](Boundary::defaults).
    /// ```
    /// # use convert_case::Boundary;
    /// assert!(
    ///     Boundary::defaults_from("Aa").len() == 0
    /// );
    /// ```
    pub const UPPER_LOWER: Boundary = Boundary {
        name: "UpperLower",
        condition: |s, _| {
            s.get(0).map(grapheme_is_uppercase) == Some(true)
                && s.get(1).map(grapheme_is_lowercase) == Some(true)
        },
        arg: None,
        start: 1,
        len: 0,
    };

    /// Acronyms are identified by two uppercase letters followed by a lowercase letter.
    /// The word boundary is between the two uppercase letters.  For example, "HTTPRequest"
    /// would have an acronym boundary identified at "PRe" and split into "HTTP" and "Request".
    /// ```
    /// # use convert_case::Boundary;
    /// assert_eq!(
    ///     vec![Boundary::ACRONYM],
    ///     Boundary::defaults_from("AAa")
    /// );
    /// ```
    pub const ACRONYM: Boundary = Boundary {
        name: "Acronym",
        condition: |s, _| {
            s.get(0).map(grapheme_is_uppercase) == Some(true)
                && s.get(1).map(grapheme_is_uppercase) == Some(true)
                && s.get(2).map(grapheme_is_lowercase) == Some(true)
        },
        arg: None,
        start: 1,
        len: 0,
    };

    /// Splits where a lowercase letter is followed by a digit.
    /// ```
    /// # use convert_case::Boundary;
    /// assert_eq!(
    ///     vec![Boundary::LOWER_DIGIT],
    ///     Boundary::defaults_from("a1")
    /// );
    /// ```
    pub const LOWER_DIGIT: Boundary = Boundary {
        name: "LowerDigit",
        condition: |s, _| {
            s.get(0).map(grapheme_is_lowercase) == Some(true)
                && s.get(1).map(grapheme_is_digit) == Some(true)
        },
        arg: None,
        start: 1,
        len: 0,
    };

    /// Splits where an uppercase letter is followed by a digit.
    /// ```
    /// # use convert_case::Boundary;
    /// assert_eq!(
    ///     vec![Boundary::UPPER_DIGIT],
    ///     Boundary::defaults_from("A1")
    /// );
    /// ```
    pub const UPPER_DIGIT: Boundary = Boundary {
        name: "UpperDigit",
        condition: |s, _| {
            s.get(0).map(grapheme_is_uppercase) == Some(true)
                && s.get(1).map(grapheme_is_digit) == Some(true)
        },
        arg: None,
        start: 1,
        len: 0,
    };

    /// Splits where digit is followed by a lowercase letter.
    /// ```
    /// # use convert_case::Boundary;
    /// assert_eq!(
    ///     vec![Boundary::DIGIT_LOWER],
    ///     Boundary::defaults_from("1a")
    /// );
    /// ```
    pub const DIGIT_LOWER: Boundary = Boundary {
        name: "DigitLower",
        condition: |s, _| {
            s.get(0).map(grapheme_is_digit) == Some(true)
                && s.get(1).map(grapheme_is_lowercase) == Some(true)
        },
        arg: None,
        start: 1,
        len: 0,
    };

    /// Splits where digit is followed by an uppercase letter.
    /// ```
    /// # use convert_case::Boundary;
    /// assert_eq!(
    ///     vec![Boundary::DIGIT_UPPER],
    ///     Boundary::defaults_from("1A")
    /// );
    /// ```
    pub const DIGIT_UPPER: Boundary = Boundary {
        name: "DigitUpper",
        condition: |s, _| {
            s.get(0).map(grapheme_is_digit) == Some(true)
                && s.get(1).map(grapheme_is_uppercase) == Some(true)
        },
        arg: None,
        start: 1,
        len: 0,
    };

    /// Create a new boundary based on a delimiter.
    /// ```
    /// # use convert_case::{Case, Converter, Boundary};
    /// let conv = Converter::new()
    ///     .set_boundaries(&[Boundary::from_delim("::")])
    ///     .to_case(Case::Camel);
    /// assert_eq!(
    ///     "myVarName",
    ///     conv.convert("my::var::name")
    /// )
    /// ```
    pub const fn from_delim(delim: &'static str) -> Boundary {
        Boundary {
            name: delim,
            arg: Some(delim),
            condition: |s, arg| s.join("").starts_with(arg.unwrap()),
            start: 0,
            len: delim.len(),
        }
    }

    /// The default list of boundaries used when `Casing::to_case` is called directly
    /// and in a `Converter` generated from `Converter::new()`.
    /// ```
    /// # use convert_case::Boundary;
    /// assert_eq!(
    ///     [
    ///         Boundary::SPACE,
    ///         Boundary::HYPHEN,
    ///         Boundary::UNDERSCORE,
    ///         Boundary::LOWER_UPPER,
    ///         Boundary::ACRONYM,
    ///         Boundary::LOWER_DIGIT,
    ///         Boundary::UPPER_DIGIT,
    ///         Boundary::DIGIT_LOWER,
    ///         Boundary::DIGIT_UPPER,
    ///     ],
    ///     Boundary::defaults()
    /// );
    /// ```
    pub const fn defaults() -> [Boundary; 9] {
        [
            Boundary::SPACE,
            Boundary::HYPHEN,
            Boundary::UNDERSCORE,
            Boundary::LOWER_UPPER,
            Boundary::ACRONYM,
            Boundary::LOWER_DIGIT,
            Boundary::UPPER_DIGIT,
            Boundary::DIGIT_LOWER,
            Boundary::DIGIT_UPPER,
        ]
    }

    /// Returns the boundaries that involve digits.
    /// `LowerDigit`.
    /// ```
    /// # use convert_case::Boundary;
    /// assert_eq!(
    ///     [
    ///         Boundary::LOWER_DIGIT,
    ///         Boundary::UPPER_DIGIT,
    ///         Boundary::DIGIT_LOWER,
    ///         Boundary::DIGIT_UPPER,
    ///     ],
    ///     Boundary::digits()
    /// );
    /// ```
    pub const fn digits() -> [Boundary; 4] {
        [
            Boundary::LOWER_DIGIT,
            Boundary::UPPER_DIGIT,
            Boundary::DIGIT_LOWER,
            Boundary::DIGIT_UPPER,
        ]
    }

    /// Returns the boundaries that are letters followed by digits.
    /// ```
    /// # use convert_case::Boundary;
    /// assert_eq!(
    ///     [
    ///         Boundary::LOWER_DIGIT,
    ///         Boundary::UPPER_DIGIT,
    ///     ],
    ///     Boundary::letter_digit()
    /// );
    /// ```
    pub const fn letter_digit() -> [Boundary; 2] {
        [Boundary::LOWER_DIGIT, Boundary::UPPER_DIGIT]
    }

    /// Returns the boundaries that are digits followed by letters.
    /// ```
    /// # use convert_case::Boundary;
    /// assert_eq!(
    ///     [
    ///         Boundary::DIGIT_LOWER,
    ///         Boundary::DIGIT_UPPER
    ///     ],
    ///     Boundary::digit_letter()
    /// );
    /// ```
    pub fn digit_letter() -> [Boundary; 2] {
        [Boundary::DIGIT_LOWER, Boundary::DIGIT_UPPER]
    }

    /// Returns a list of all boundaries that are identified within the given string.
    /// Could be a short of writing out all the boundaries in a list directly.  This will not
    /// identify boundary `UpperLower` if it also used as part of `Acronym`.
    ///
    /// If you want to be very explicit and not overlap boundaries, it is recommended to use a colon
    /// character.
    /// ```
    /// # use convert_case::Boundary;
    /// assert_eq!(
    ///     vec![
    ///         Boundary::SPACE,
    ///         Boundary::HYPHEN,
    ///         Boundary::LOWER_UPPER,
    ///         Boundary::UPPER_DIGIT,
    ///         Boundary::DIGIT_LOWER,
    ///     ],
    ///     Boundary::defaults_from("aA8a -")
    /// );
    /// assert_eq!(
    ///     vec![
    ///         Boundary::UNDERSCORE,
    ///         Boundary::LOWER_UPPER,
    ///         Boundary::ACRONYM,
    ///         Boundary::DIGIT_UPPER,
    ///     ],
    ///     Boundary::defaults_from("bD:0B:_:AAa")
    /// );
    /// ```
    pub fn defaults_from(pattern: &str) -> Vec<Boundary> {
        let mut boundaries = Vec::new();
        for boundary in Boundary::defaults() {
            let parts = split(&pattern, &[boundary]);
            if parts.len() > 1 || parts.len() == 0 || parts[0] != pattern {
                boundaries.push(boundary);
            }
        }
        boundaries
    }
}

/// Split an identifier into a list of words using the list of boundaries.
///
/// This is used internally for splitting an identifier before mutating by
/// a pattern and joining again with a delimiter.
/// ```
/// use convert_case::{Boundary, split};
/// assert_eq!(
///     vec!["one", "two", "three.four"],
///     split(&"one_two-three.four", &[Boundary::UNDERSCORE, Boundary::HYPHEN]),
/// )
/// ```
pub fn split<'s, T>(s: &'s T, boundaries: &[Boundary]) -> Vec<&'s str>
where
    T: AsRef<str>,
{
    let s = s.as_ref();

    if s.len() == 0 {
        return vec![];
    }

    let mut words = Vec::new();
    let mut last_boundary_end = 0;

    let (indices, graphemes): (Vec<_>, Vec<_>) = s.grapheme_indices(true).unzip();
    let grapheme_length = indices[graphemes.len() - 1] + graphemes[graphemes.len() - 1].len();

    for i in 0..graphemes.len() {
        for boundary in boundaries {
            //let byte_index = indices[i];

            if (boundary.condition)(&graphemes[i..], boundary.arg) {
                // What if we find a condition at the end of the array?
                // Maybe we can stop early based on length
                // To do this, need to switch the loops
                // TODO
                let boundary_byte_start: usize =
                    *indices.get(i + boundary.start).unwrap_or(&grapheme_length);
                let boundary_byte_end: usize = *indices
                    .get(i + boundary.start + boundary.len)
                    .unwrap_or(&grapheme_length);

                // todo clean this up a bit
                words.push(&s[last_boundary_end..boundary_byte_start]);
                last_boundary_end = boundary_byte_end;
                break;
            }
        }
    }
    words.push(&s[last_boundary_end..]);
    words.into_iter().filter(|s| !s.is_empty()).collect()
}

// ascii version
//pub fn split<'s, T>(s: &'s T, boundaries: &[Boundary]) -> Vec<&'s str>
//where
//    T: AsRef<str>,
//{
//    let s = s.as_ref();
//
//    let mut words = Vec::new();
//    let mut last_end = 0;
//    for i in 0..s.len() {
//        for boundary in boundaries {
//            if (boundary.condition)(&s[i..]) {
//                words.push(&s[last_end..i + boundary.start]);
//                last_end = i + boundary.start + boundary.len;
//                break;
//            }
//        }
//    }
//    words.push(&s[last_end..]);
//    words
//}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hyphen() {
        let s = "a-b-c";
        let v = split(&s, &[Boundary::HYPHEN]);
        assert_eq!(v, vec!["a", "b", "c"]);
    }

    #[test]
    fn underscore() {
        let s = "a_b_c";
        let v = split(&s, &[Boundary::UNDERSCORE]);
        assert_eq!(v, vec!["a", "b", "c"]);
    }

    #[test]
    fn space() {
        let s = "a b c";
        let v = split(&s, &[Boundary::SPACE]);
        assert_eq!(v, vec!["a", "b", "c"]);
    }

    #[test]
    fn delimiters() {
        let s = "aaa-bbb_ccc ddd ddd-eee";
        let v = split(
            &s,
            &[Boundary::SPACE, Boundary::UNDERSCORE, Boundary::HYPHEN],
        );
        assert_eq!(v, vec!["aaa", "bbb", "ccc", "ddd", "ddd", "eee"]);
    }

    #[test]
    fn lower_upper() {
        let s = "lowerUpperUpper";
        let v = split(&s, &[Boundary::LOWER_UPPER]);
        assert_eq!(v, vec!["lower", "Upper", "Upper"]);
    }

    #[test]
    fn acronym() {
        let s = "XMLRequest";
        let v = split(&s, &[Boundary::ACRONYM]);
        assert_eq!(v, vec!["XML", "Request"]);
    }

    // TODO: add tests for other boundaries

    #[test]
    fn boundaries_found_in_string() {
        // upper lower is not longer a default
        assert_eq!(Vec::<Boundary>::new(), Boundary::defaults_from(".Aaaa"));
        assert_eq!(
            vec![Boundary::LOWER_UPPER, Boundary::LOWER_DIGIT,],
            Boundary::defaults_from("a8.Aa.aA")
        );
        assert_eq!(
            Boundary::digits().to_vec(),
            Boundary::defaults_from("b1B1b")
        );
        assert_eq!(
            vec![
                Boundary::SPACE,
                Boundary::HYPHEN,
                Boundary::UNDERSCORE,
                Boundary::ACRONYM,
            ],
            Boundary::defaults_from("AAa -_")
        );
    }

    #[test]
    fn boundary_consts_same() {
        assert_eq!(Boundary::SPACE, Boundary::SPACE);
    }

    #[test]
    fn from_delim_dot() {
        let boundary = Boundary::from_delim(".");
        let s = "lower.Upper.Upper";
        let v = split(&s, &[boundary]);
        assert_eq!(vec!["lower", "Upper", "Upper"], v)
    }

    #[test]
    fn from_delim_double_colon() {
        let boundary = Boundary::from_delim("::");
        let s = "lower::lowerUpper::Upper";
        let v = split(&s, &[boundary]);
        assert_eq!(vec!["lower", "lowerUpper", "Upper"], v)
    }
}
