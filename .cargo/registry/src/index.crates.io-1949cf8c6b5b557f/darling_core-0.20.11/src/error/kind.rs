use std::fmt;

use crate::error::Error;

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
    UnknownField(ErrorUnknownField),
    UnexpectedFormat(MetaFormat),
    UnexpectedType(String),
    UnknownValue(String),
    TooFewItems(usize),
    TooManyItems(usize),
    /// A set of errors.
    Multiple(Vec<Error>),

    // TODO make this variant take `!` so it can't exist
    #[doc(hidden)]
    __NonExhaustive,
}

impl ErrorKind {
    pub fn description(&self) -> &str {
        use self::ErrorKind::*;

        match *self {
            Custom(ref s) => s,
            DuplicateField(_) => "Duplicate field",
            MissingField(_) => "Missing field",
            UnknownField(_) => "Unexpected field",
            UnsupportedShape { .. } => "Unsupported shape",
            UnexpectedFormat(_) => "Unexpected meta-item format",
            UnexpectedType(_) => "Unexpected type",
            UnknownValue(_) => "Unknown literal value",
            TooFewItems(_) => "Too few items",
            TooManyItems(_) => "Too many items",
            Multiple(_) => "Multiple errors",
            __NonExhaustive => unreachable!(),
        }
    }

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
            UnknownValue(ref val) => write!(f, "Unknown literal value `{}`", val),
            TooFewItems(ref min) => write!(f, "Too few items: Expected at least {}", min),
            TooManyItems(ref max) => write!(f, "Too many items: Expected no more than {}", max),
            Multiple(ref items) if items.len() == 1 => items[0].fmt(f),
            Multiple(ref items) => {
                write!(f, "Multiple errors: (")?;
                let mut first = true;
                for item in items {
                    if !first {
                        write!(f, ", ")?;
                    } else {
                        first = false;
                    }

                    item.fmt(f)?;
                }

                write!(f, ")")
            }
            __NonExhaustive => unreachable!(),
        }
    }
}

impl From<ErrorUnknownField> for ErrorKind {
    fn from(err: ErrorUnknownField) -> Self {
        ErrorKind::UnknownField(err)
    }
}

/// An error for an unknown field, with a possible "did-you-mean" suggestion to get
/// the user back on the right track.
#[derive(Clone, Debug)]
// Don't want to publicly commit to ErrorKind supporting equality yet, but
// not having it makes testing very difficult.
#[cfg_attr(test, derive(PartialEq))]
pub(in crate::error) struct ErrorUnknownField {
    name: String,
    did_you_mean: Option<(f64, String)>,
}

impl ErrorUnknownField {
    pub fn new<I: Into<String>>(name: I, did_you_mean: Option<(f64, String)>) -> Self {
        ErrorUnknownField {
            name: name.into(),
            did_you_mean,
        }
    }

    pub fn with_alts<'a, T, I>(field: &str, alternates: I) -> Self
    where
        T: AsRef<str> + 'a,
        I: IntoIterator<Item = &'a T>,
    {
        ErrorUnknownField::new(field, did_you_mean(field, alternates))
    }

    /// Add more alternate field names to the error, updating the `did_you_mean` suggestion
    /// if a closer match to the unknown field's name is found.
    pub fn add_alts<'a, T, I>(&mut self, alternates: I)
    where
        T: AsRef<str> + 'a,
        I: IntoIterator<Item = &'a T>,
    {
        if let Some(bna) = did_you_mean(&self.name, alternates) {
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
        let base = span
            .unwrap_or_else(::proc_macro2::Span::call_site)
            .unwrap()
            .error(self.top_line());
        match self.did_you_mean {
            Some((_, alt_name)) => base.help(format!("did you mean `{}`?", alt_name)),
            None => base,
        }
    }

    #[cfg(feature = "diagnostics")]
    fn top_line(&self) -> String {
        format!("Unknown field: `{}`", self.name)
    }
}

impl From<String> for ErrorUnknownField {
    fn from(name: String) -> Self {
        ErrorUnknownField::new(name, None)
    }
}

impl<'a> From<&'a str> for ErrorUnknownField {
    fn from(name: &'a str) -> Self {
        ErrorUnknownField::new(name, None)
    }
}

impl fmt::Display for ErrorUnknownField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unknown field: `{}`", self.name)?;

        if let Some((_, ref did_you_mean)) = self.did_you_mean {
            write!(f, ". Did you mean `{}`?", did_you_mean)?;
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
