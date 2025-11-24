use std::{collections::BTreeSet, fmt};

use crate::error::{
    util::{write_delimited, Quoted},
    Error,
};

type DeriveInputShape = String;
type FieldName = String;
type MetaFormat = String;

#[derive(Debug, Clone)]
// Don't want to publicly commit to ErrorKind supporting equality yet, but
// not having it makes testing very difficult.
#[cfg_attr(test, derive(PartialEq))]
pub(in crate::error) enum ErrorKind {
    /// An arbitrary error message.
    Custom(String),
    DuplicateField(FieldName),
    MissingField(FieldName),
    UnsupportedShape {
        observed: DeriveInputShape,
        expected: Option<String>,
    },
    UnknownField(Box<ErrorUnknownValue>),
    UnexpectedFormat(MetaFormat),
    UnexpectedType(String),
    UnknownValue(Box<ErrorUnknownValue>),
    TooFewItems(usize),
    TooManyItems(usize),
    /// A set of errors.
    Multiple(Vec<Error>),

    // TODO make this variant take `!` so it can't exist
    #[doc(hidden)]
    __NonExhaustive,
}

impl ErrorKind {
    /// Deeply counts the number of errors this item represents.
    pub fn len(&self) -> usize {
        if let ErrorKind::Multiple(ref items) = *self {
            items.iter().map(Error::len).sum()
        } else {
            1
        }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use self::ErrorKind::*;

        match *self {
            Custom(ref s) => s.fmt(f),
            DuplicateField(ref field) => write!(f, "Duplicate field `{}`", field),
            MissingField(ref field) => write!(f, "Missing field `{}`", field),
            UnknownField(ref field) => field.fmt(f),
            UnsupportedShape {
                ref observed,
                ref expected,
            } => {
                write!(f, "Unsupported shape `{}`", observed)?;
                if let Some(expected) = &expected {
                    write!(f, ". Expected {}.", expected)?;
                }

                Ok(())
            }
            UnexpectedFormat(ref format) => write!(f, "Unexpected meta-item format `{}`", format),
            UnexpectedType(ref ty) => write!(f, "Unexpected type `{}`", ty),
            UnknownValue(ref val) => val.fmt(f),
            TooFewItems(ref min) => write!(f, "Too few items: Expected at least {}", min),
            TooManyItems(ref max) => write!(f, "Too many items: Expected no more than {}", max),
            Multiple(ref items) if items.len() == 1 => items[0].fmt(f),
            Multiple(ref items) => {
                write!(f, "Multiple errors: (")?;
                write_delimited(f, items, ", ")?;
                write!(f, ")")
            }
            __NonExhaustive => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::error) enum UnknownValuePosition {
    Field,
    Value,
}

impl AsRef<str> for UnknownValuePosition {
    fn as_ref(&self) -> &str {
        match self {
            UnknownValuePosition::Field => "field",
            UnknownValuePosition::Value => "value",
        }
    }
}

impl fmt::Display for UnknownValuePosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

impl From<ErrorUnknownValue> for ErrorKind {
    fn from(value: ErrorUnknownValue) -> Self {
        match value.noun {
            UnknownValuePosition::Field => Self::UnknownField(Box::new(value)),
            UnknownValuePosition::Value => Self::UnknownValue(Box::new(value)),
        }
    }
}

/// An error where an unknown value was seen in a given position,
/// with a possible "did-you-mean" suggestion to get the user back on the right track.
#[derive(Clone, Debug)]
// Don't want to publicly commit to ErrorKind supporting equality yet, but
// not having it makes testing very difficult.
#[cfg_attr(test, derive(PartialEq))]
pub(in crate::error) struct ErrorUnknownValue {
    /// The thing whose value is unknown.
    noun: UnknownValuePosition,

    value: String,
    /// The best suggestion of what field the caller could have meant, along with
    /// the similarity score between that best option and the actual caller-provided
    /// field name.
    did_you_mean: Option<(f64, String)>,
    /// Set of all known valid field names.
    ///
    /// This is a `BTreeSet` so that names will be displayed in alphabetical order
    /// without needing display-time sorting.
    alts: BTreeSet<String>,
}

impl ErrorUnknownValue {
    pub fn new<I: Into<String>>(noun: UnknownValuePosition, value: I) -> Self {
        ErrorUnknownValue {
            noun,
            value: value.into(),
            did_you_mean: None,
            alts: BTreeSet::new(),
        }
    }

    pub fn with_alts<'a, T, I>(noun: UnknownValuePosition, value: &str, alternates: I) -> Self
    where
        T: AsRef<str> + 'a,
        I: IntoIterator<Item = &'a T>,
    {
        let alts = alternates
            .into_iter()
            .map(|s| s.as_ref().to_string())
            .collect();
        Self {
            noun,
            value: value.into(),
            did_you_mean: did_you_mean(value, &alts),
            alts,
        }
    }

    /// Add more alternate values to the error, updating the `did_you_mean` suggestion
    /// if a closer match to the unknown value is found.
    pub fn add_alts<'a, T, I>(&mut self, alternates: I)
    where
        T: AsRef<str> + 'a,
        I: IntoIterator<Item = &'a T>,
    {
        let alts = alternates.into_iter().collect::<Vec<_>>();
        self.alts
            .extend(alts.iter().map(|s| s.as_ref().to_string()));
        if let Some(bna) = did_you_mean(&self.value, alts) {
            if let Some(current) = &self.did_you_mean {
                if bna.0 > current.0 {
                    self.did_you_mean = Some(bna);
                }
            } else {
                self.did_you_mean = Some(bna);
            }
        }
    }

    #[cfg(feature = "diagnostics")]
    pub fn into_diagnostic(self, span: Option<::proc_macro2::Span>) -> ::proc_macro::Diagnostic {
        let mut diag = span
            .unwrap_or_else(::proc_macro2::Span::call_site)
            .unwrap()
            .error(self.top_line());

        if let Some((_, alt_name)) = self.did_you_mean {
            diag = diag.help(format!("did you mean `{}`?", alt_name));
        }

        if !self.alts.is_empty() {
            let mut alts = String::new();

            // Per documentation, formatting is infallible unless underlying stream closes
            // https://doc.rust-lang.org/std/fmt/struct.Error.html
            write_delimited(&mut alts, self.alts.iter().map(Quoted::backticks), ", ")
                .expect("writing to a string never fails");
            diag = diag.help(format!("available values: {}", alts));
        }

        diag
    }

    #[cfg(feature = "diagnostics")]
    fn top_line(&self) -> String {
        format!("Unknown {}: `{}`", self.noun, self.value)
    }
}

impl fmt::Display for ErrorUnknownValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unknown {}: `{}`", self.noun, self.value)?;

        if let Some((_, ref did_you_mean)) = self.did_you_mean {
            write!(f, ". Did you mean `{}`?", did_you_mean)?;
        } else if !self.alts.is_empty() && self.alts.len() < 10 {
            write!(f, ". Available values: ")?;
            write_delimited(f, self.alts.iter().map(Quoted::backticks), ", ")?;
        }

        Ok(())
    }
}

#[cfg(feature = "suggestions")]
fn did_you_mean<'a, T, I>(field: &str, alternates: I) -> Option<(f64, String)>
where
    T: AsRef<str> + 'a,
    I: IntoIterator<Item = &'a T>,
{
    let mut candidate: Option<(f64, &str)> = None;
    for pv in alternates {
        let confidence = ::strsim::jaro_winkler(field, pv.as_ref());
        if confidence > 0.8 && (candidate.is_none() || (candidate.as_ref().unwrap().0 < confidence))
        {
            candidate = Some((confidence, pv.as_ref()));
        }
    }
    candidate.map(|(score, candidate)| (score, candidate.into()))
}

#[cfg(not(feature = "suggestions"))]
fn did_you_mean<'a, T, I>(_field: &str, _alternates: I) -> Option<(f64, String)>
where
    T: AsRef<str> + 'a,
    I: IntoIterator<Item = &'a T>,
{
    None
}

#[cfg(test)]
mod tests {
    use super::{ErrorUnknownValue, UnknownValuePosition};

    /// Make sure that an unknown field error with no alts or suggestions has
    /// only the relevant information and no fragments of other sentences.
    #[test]
    fn present_no_alts() {
        let err = ErrorUnknownValue::new(UnknownValuePosition::Field, "hello");
        assert_eq!(&err.to_string(), "Unknown field: `hello`");
    }

    #[test]
    fn present_few_alts() {
        let err = ErrorUnknownValue::with_alts(
            UnknownValuePosition::Field,
            "hello",
            &["world", "friend"],
        );
        assert!(err.to_string().contains("`friend`"));
    }
}
