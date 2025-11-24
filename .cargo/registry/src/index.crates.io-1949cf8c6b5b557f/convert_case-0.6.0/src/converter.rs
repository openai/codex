use crate::segmentation;
use crate::Boundary;
use crate::Case;
use crate::Pattern;

/// The parameters for performing a case conversion.
///
/// A `Converter` stores three fields needed for case conversion.
/// 1) `boundaries`: how a string is segmented into _words_.
/// 2) `pattern`: how words are mutated, or how each character's case will change.
/// 3) `delim` or delimeter: how the mutated words are joined into the final string.
///
/// Then calling [`convert`](Converter::convert) on a `Converter` will apply a case conversion
/// defined by those fields.  The `Converter` struct is what is used underneath those functions
/// available in the `Casing` struct.  
///
/// You can use `Converter` when you need more specificity on conversion
/// than those provided in `Casing`, or if it is simply more convenient or explicit.
///
/// ```
/// use convert_case::{Boundary, Case, Casing, Converter, Pattern};
///
/// let s = "DialogueBox-border-shadow";
///
/// // Convert using Casing trait
/// assert_eq!(
///     "dialoguebox_border_shadow",
///     s.from_case(Case::Kebab).to_case(Case::Snake)
/// );
///
/// // Convert using similar functions on Converter
/// let conv = Converter::new()
///     .from_case(Case::Kebab)
///     .to_case(Case::Snake);
/// assert_eq!("dialoguebox_border_shadow", conv.convert(s));
///
/// // Convert by setting each field explicitly.
/// let conv = Converter::new()
///     .set_boundaries(&[Boundary::Hyphen])
///     .set_pattern(Pattern::Lowercase)
///     .set_delim("_");
/// assert_eq!("dialoguebox_border_shadow", conv.convert(s));
/// ```
///
/// Or you can use `Converter` when you are trying to make a unique case
/// not provided as a variant of `Case`.
///
/// ```
/// use convert_case::{Boundary, Case, Casing, Converter, Pattern};
///
/// let dot_camel = Converter::new()
///     .set_boundaries(&[Boundary::LowerUpper, Boundary::LowerDigit])
///     .set_pattern(Pattern::Camel)
///     .set_delim(".");
/// assert_eq!("collision.Shape.2d", dot_camel.convert("CollisionShape2D"));
/// ```
pub struct Converter {
    /// How a string is segmented into words.
    pub boundaries: Vec<Boundary>,

    /// How each word is mutated before joining.  In the case that there is no pattern, none of the
    /// words will be mutated before joining and will maintain whatever case they were in the
    /// original string.
    pub pattern: Option<Pattern>,

    /// The string used to join mutated words together.
    pub delim: String,
}

impl Default for Converter {
    fn default() -> Self {
        Converter {
            boundaries: Boundary::defaults(),
            pattern: None,
            delim: String::new(),
        }
    }
}

impl Converter {
    /// Creates a new `Converter` with default fields.  This is the same as `Default::default()`.
    /// The `Converter` will use `Boundary::defaults()` for boundaries, no pattern, and an empty
    /// string as a delimeter.
    /// ```
    /// use convert_case::Converter;
    ///
    /// let conv = Converter::new();
    /// assert_eq!("DeathPerennialQUEST", conv.convert("Death-Perennial QUEST"))
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Converts a string.
    /// ```
    /// use convert_case::{Case, Converter};
    ///
    /// let conv = Converter::new()
    ///     .to_case(Case::Camel);
    /// assert_eq!("xmlHttpRequest", conv.convert("XML_HTTP_Request"))
    /// ```
    pub fn convert<T>(&self, s: T) -> String
    where
        T: AsRef<str>,
    {
        let words = segmentation::split(&s, &self.boundaries);
        if let Some(p) = self.pattern {
            let words = words.iter().map(|s| s.as_ref()).collect::<Vec<&str>>();
            p.mutate(&words).join(&self.delim)
        } else {
            words.join(&self.delim)
        }
    }

    /// Set the pattern and delimiter to those associated with the given case.
    /// ```
    /// use convert_case::{Case, Converter};
    ///
    /// let conv = Converter::new()
    ///     .to_case(Case::Pascal);
    /// assert_eq!("VariableName", conv.convert("variable name"))
    /// ```
    pub fn to_case(mut self, case: Case) -> Self {
        self.pattern = Some(case.pattern());
        self.delim = case.delim().to_string();
        self
    }

    /// Sets the boundaries to those associated with the provided case.  This is used
    /// by the `from_case` function in the `Casing` trait.
    /// ```
    /// use convert_case::{Case, Converter};
    ///
    /// let conv = Converter::new()
    ///     .from_case(Case::Snake)
    ///     .to_case(Case::Title);
    /// assert_eq!("Dot Productvalue", conv.convert("dot_productValue"))
    /// ```
    pub fn from_case(mut self, case: Case) -> Self {
        self.boundaries = case.boundaries();
        self
    }

    /// Sets the boundaries to those provided.
    /// ```
    /// use convert_case::{Boundary, Case, Converter};
    ///
    /// let conv = Converter::new()
    ///     .set_boundaries(&[Boundary::Underscore, Boundary::LowerUpper])
    ///     .to_case(Case::Lower);
    /// assert_eq!("panic attack dream theater", conv.convert("panicAttack_dreamTheater"))
    /// ```
    pub fn set_boundaries(mut self, bs: &[Boundary]) -> Self {
        self.boundaries = bs.to_vec();
        self
    }

    /// Adds a boundary to the list of boundaries.
    /// ```
    /// use convert_case::{Boundary, Case, Converter};
    ///
    /// let conv = Converter::new()
    ///     .from_case(Case::Title)
    ///     .add_boundary(Boundary::Hyphen)
    ///     .to_case(Case::Snake);
    /// assert_eq!("my_biography_video_1", conv.convert("My Biography - Video 1"))
    /// ```
    pub fn add_boundary(mut self, b: Boundary) -> Self {
        self.boundaries.push(b);
        self
    }

    /// Adds a vector of boundaries to the list of boundaries.
    /// ```
    /// use convert_case::{Boundary, Case, Converter};
    ///
    /// let conv = Converter::new()
    ///     .from_case(Case::Kebab)
    ///     .to_case(Case::Title)
    ///     .add_boundaries(&[Boundary::Underscore, Boundary::LowerUpper]);
    /// assert_eq!("2020 10 First Day", conv.convert("2020-10_firstDay"));
    /// ```
    pub fn add_boundaries(mut self, bs: &[Boundary]) -> Self {
        self.boundaries.extend(bs);
        self
    }

    /// Removes a boundary from the list of boundaries if it exists.
    /// ```
    /// use convert_case::{Boundary, Case, Converter};
    ///
    /// let conv = Converter::new()
    ///     .remove_boundary(Boundary::Acronym)
    ///     .to_case(Case::Kebab);
    /// assert_eq!("httprequest-parser", conv.convert("HTTPRequest_parser"));
    /// ```
    pub fn remove_boundary(mut self, b: Boundary) -> Self {
        self.boundaries.retain(|&x| x != b);
        self
    }

    /// Removes all the provided boundaries from the list of boundaries if it exists.
    /// ```
    /// use convert_case::{Boundary, Case, Converter};
    ///
    /// let conv = Converter::new()
    ///     .remove_boundaries(&Boundary::digits())
    ///     .to_case(Case::Snake);
    /// assert_eq!("c04_s03_path_finding.pdf", conv.convert("C04 S03 Path Finding.pdf"));
    /// ```
    pub fn remove_boundaries(mut self, bs: &[Boundary]) -> Self {
        for b in bs {
            self.boundaries.retain(|&x| x != *b);
        }
        self
    }

    /// Sets the delimeter.
    /// ```
    /// use convert_case::{Case, Converter};
    ///
    /// let conv = Converter::new()
    ///     .to_case(Case::Snake)
    ///     .set_delim(".");
    /// assert_eq!("lower.with.dots", conv.convert("LowerWithDots"));
    /// ```
    pub fn set_delim<T>(mut self, d: T) -> Self
    where
        T: ToString,
    {
        self.delim = d.to_string();
        self
    }

    /// Sets the delimeter to an empty string.
    /// ```
    /// use convert_case::{Case, Converter};
    ///
    /// let conv = Converter::new()
    ///     .to_case(Case::Snake)
    ///     .remove_delim();
    /// assert_eq!("nodelimshere", conv.convert("No Delims Here"));
    /// ```
    pub fn remove_delim(mut self) -> Self {
        self.delim = String::new();
        self
    }

    /// Sets the pattern.
    /// ```
    /// use convert_case::{Case, Converter, Pattern};
    ///
    /// let conv = Converter::new()
    ///     .set_delim("_")
    ///     .set_pattern(Pattern::Sentence);
    /// assert_eq!("Bjarne_case", conv.convert("BJARNE CASE"));
    /// ```
    pub fn set_pattern(mut self, p: Pattern) -> Self {
        self.pattern = Some(p);
        self
    }

    /// Sets the pattern field to `None`.  Where there is no pattern, a character's case is never
    /// mutated and will be maintained at the end of conversion.
    /// ```
    /// use convert_case::{Case, Converter};
    ///
    /// let conv = Converter::new()
    ///     .from_case(Case::Title)
    ///     .to_case(Case::Snake)
    ///     .remove_pattern();
    /// assert_eq!("KoRn_Alone_I_Break", conv.convert("KoRn Alone I Break"));
    /// ```
    pub fn remove_pattern(mut self) -> Self {
        self.pattern = None;
        self
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::Casing;
    use crate::Pattern;

    #[test]
    fn snake_converter_from_case() {
        let conv = Converter::new().to_case(Case::Snake);
        let s = String::from("my var name");
        assert_eq!(s.to_case(Case::Snake), conv.convert(s));
    }

    #[test]
    fn snake_converter_from_scratch() {
        let conv = Converter::new()
            .set_delim("_")
            .set_pattern(Pattern::Lowercase);
        let s = String::from("my var name");
        assert_eq!(s.to_case(Case::Snake), conv.convert(s));
    }

    #[test]
    fn custom_pattern() {
        let conv = Converter::new()
            .to_case(Case::Snake)
            .set_pattern(Pattern::Sentence);
        assert_eq!("Bjarne_case", conv.convert("bjarne case"));
    }

    #[test]
    fn custom_delim() {
        let conv = Converter::new().set_delim("..");
        assert_eq!("oh..My", conv.convert("ohMy"));
    }

    #[test]
    fn no_pattern() {
        let conv = Converter::new()
            .from_case(Case::Title)
            .to_case(Case::Kebab)
            .remove_pattern();
        assert_eq!("wIErd-CASing", conv.convert("wIErd CASing"));
    }

    #[test]
    fn no_delim() {
        let conv = Converter::new()
            .from_case(Case::Title)
            .to_case(Case::Kebab)
            .remove_delim();
        assert_eq!("justflat", conv.convert("Just Flat"));
    }

    #[test]
    fn no_digit_boundaries() {
        let conv = Converter::new()
            .remove_boundaries(&Boundary::digits())
            .to_case(Case::Snake);
        assert_eq!("test_08bound", conv.convert("Test 08Bound"));
        assert_eq!("a8a_a8a", conv.convert("a8aA8A"));
    }

    #[test]
    fn remove_boundary() {
        let conv = Converter::new()
            .remove_boundary(Boundary::DigitUpper)
            .to_case(Case::Snake);
        assert_eq!("test_08bound", conv.convert("Test 08Bound"));
        assert_eq!("a_8_a_a_8a", conv.convert("a8aA8A"));
    }

    #[test]
    fn add_boundary() {
        let conv = Converter::new()
            .from_case(Case::Snake)
            .to_case(Case::Kebab)
            .add_boundary(Boundary::LowerUpper);
        assert_eq!("word-word-word", conv.convert("word_wordWord"));
    }

    #[test]
    fn add_boundaries() {
        let conv = Converter::new()
            .from_case(Case::Snake)
            .to_case(Case::Kebab)
            .add_boundaries(&[Boundary::LowerUpper, Boundary::UpperLower]);
        assert_eq!("word-word-w-ord", conv.convert("word_wordWord"));
    }

    #[test]
    fn reuse_after_change() {
        let conv = Converter::new().from_case(Case::Snake).to_case(Case::Kebab);
        assert_eq!("word-wordword", conv.convert("word_wordWord"));

        let conv = conv.add_boundary(Boundary::LowerUpper);
        assert_eq!("word-word-word", conv.convert("word_wordWord"));
    }

    #[test]
    fn explicit_boundaries() {
        let conv = Converter::new()
            .set_boundaries(&[
                Boundary::DigitLower,
                Boundary::DigitUpper,
                Boundary::Acronym,
            ])
            .to_case(Case::Snake);
        assert_eq!(
            "section8_lesson2_http_requests",
            conv.convert("section8lesson2HTTPRequests")
        );
    }
}
