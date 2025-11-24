use heck::{
    ToKebabCase, ToLowerCamelCase, ToShoutySnakeCase, ToSnakeCase, ToTitleCase, ToTrainCase,
    ToUpperCamelCase,
};
use std::str::FromStr;
use syn::{
    parse::{Parse, ParseStream},
    Ident, LitStr,
};

#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CaseStyle {
    CamelCase,
    KebabCase,
    MixedCase,
    ShoutySnakeCase,
    SnakeCase,
    TitleCase,
    UpperCase,
    LowerCase,
    ScreamingKebabCase,
    PascalCase,
    TrainCase,
}

const VALID_CASE_STYLES: &[&str] = &[
    "camelCase",
    "PascalCase",
    "kebab-case",
    "snake_case",
    "SCREAMING_SNAKE_CASE",
    "SCREAMING-KEBAB-CASE",
    "lowercase",
    "UPPERCASE",
    "title_case",
    "mixed_case",
    "Train-Case",
];

impl Parse for CaseStyle {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let text = input.parse::<LitStr>()?;
        let val = text.value();

        val.as_str().parse().map_err(|_| {
            syn::Error::new_spanned(
                &text,
                format!(
                    "Unexpected case style for serialize_all: `{}`. Valid values are: `{:?}`",
                    val, VALID_CASE_STYLES
                ),
            )
        })
    }
}

impl FromStr for CaseStyle {
    type Err = ();

    fn from_str(text: &str) -> Result<Self, ()> {
        Ok(match text {
            // "camel_case" is a soft-deprecated case-style left for backward compatibility.
            // <https://github.com/Peternator7/strum/pull/250#issuecomment-1374682221>
            "PascalCase" | "camel_case" => CaseStyle::PascalCase,
            "camelCase" => CaseStyle::CamelCase,
            "snake_case" | "snek_case" => CaseStyle::SnakeCase,
            "kebab-case" | "kebab_case" => CaseStyle::KebabCase,
            "SCREAMING-KEBAB-CASE" => CaseStyle::ScreamingKebabCase,
            "SCREAMING_SNAKE_CASE" | "shouty_snake_case" | "shouty_snek_case" => {
                CaseStyle::ShoutySnakeCase
            }
            "title_case" => CaseStyle::TitleCase,
            "mixed_case" => CaseStyle::MixedCase,
            "lowercase" => CaseStyle::LowerCase,
            "UPPERCASE" => CaseStyle::UpperCase,
            "Train-Case" => CaseStyle::TrainCase,
            _ => return Err(()),
        })
    }
}

pub trait CaseStyleHelpers {
    fn convert_case(&self, case_style: Option<CaseStyle>) -> String;
}

impl CaseStyleHelpers for Ident {
    fn convert_case(&self, case_style: Option<CaseStyle>) -> String {
        let ident_string = self.to_string();
        if let Some(case_style) = case_style {
            match case_style {
                CaseStyle::PascalCase => ident_string.to_upper_camel_case(),
                CaseStyle::KebabCase => ident_string.to_kebab_case(),
                CaseStyle::MixedCase => ident_string.to_lower_camel_case(),
                CaseStyle::ShoutySnakeCase => ident_string.to_shouty_snake_case(),
                CaseStyle::SnakeCase => ident_string.to_snake_case(),
                CaseStyle::TitleCase => ident_string.to_title_case(),
                CaseStyle::UpperCase => ident_string.to_uppercase(),
                CaseStyle::LowerCase => ident_string.to_lowercase(),
                CaseStyle::ScreamingKebabCase => ident_string.to_kebab_case().to_uppercase(),
                CaseStyle::TrainCase => ident_string.to_train_case(),
                CaseStyle::CamelCase => {
                    let camel_case = ident_string.to_upper_camel_case();
                    let mut pascal = String::with_capacity(camel_case.len());
                    let mut it = camel_case.chars();
                    if let Some(ch) = it.next() {
                        pascal.extend(ch.to_lowercase());
                    }
                    pascal.extend(it);
                    pascal
                }
            }
        } else {
            ident_string
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_case() {
        let id = Ident::new("test_me", proc_macro2::Span::call_site());
        assert_eq!("testMe", id.convert_case(Some(CaseStyle::CamelCase)));
        assert_eq!("TestMe", id.convert_case(Some(CaseStyle::PascalCase)));
        assert_eq!("Test-Me", id.convert_case(Some(CaseStyle::TrainCase)));
    }

    #[test]
    fn test_impl_from_str_for_case_style_pascal_case() {
        use CaseStyle::*;
        let f = CaseStyle::from_str;

        assert_eq!(PascalCase, f("PascalCase").unwrap());
        assert_eq!(PascalCase, f("camel_case").unwrap());

        assert_eq!(CamelCase, f("camelCase").unwrap());

        assert_eq!(SnakeCase, f("snake_case").unwrap());
        assert_eq!(SnakeCase, f("snek_case").unwrap());

        assert_eq!(KebabCase, f("kebab-case").unwrap());
        assert_eq!(KebabCase, f("kebab_case").unwrap());

        assert_eq!(ScreamingKebabCase, f("SCREAMING-KEBAB-CASE").unwrap());

        assert_eq!(ShoutySnakeCase, f("SCREAMING_SNAKE_CASE").unwrap());
        assert_eq!(ShoutySnakeCase, f("shouty_snake_case").unwrap());
        assert_eq!(ShoutySnakeCase, f("shouty_snek_case").unwrap());

        assert_eq!(LowerCase, f("lowercase").unwrap());

        assert_eq!(UpperCase, f("UPPERCASE").unwrap());

        assert_eq!(TitleCase, f("title_case").unwrap());

        assert_eq!(MixedCase, f("mixed_case").unwrap());
    }
}

/// heck doesn't treat numbers as new words, but this function does.
/// E.g. for input `Hello2You`, heck would output `hello2_you`, and snakify would output `hello_2_you`.
pub fn snakify(s: &str) -> String {
    let mut output: Vec<char> = s.to_string().to_snake_case().chars().collect();
    let mut num_starts = vec![];
    for (pos, c) in output.iter().enumerate() {
        if c.is_digit(10) && pos != 0 && !output[pos - 1].is_digit(10) {
            num_starts.push(pos);
        }
    }
    // need to do in reverse, because after inserting, all chars after the point of insertion are off
    for i in num_starts.into_iter().rev() {
        output.insert(i, '_')
    }
    output.into_iter().collect()
}
