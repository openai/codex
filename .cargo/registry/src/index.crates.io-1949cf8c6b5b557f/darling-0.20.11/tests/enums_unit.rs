//! Test expansion of enum variants which have no associated data.

use darling::{ast::NestedMeta, FromMeta};
use syn::{parse_quote, spanned::Spanned, Meta};

#[derive(Debug, FromMeta)]
#[darling(rename_all = "snake_case")]
enum Pattern {
    Owned,
    Immutable,
    Mutable,
}

#[test]
fn expansion() {}

#[test]
fn rejected_in_unit_enum_variants() {
    #[derive(Debug, FromMeta, PartialEq)]
    struct Opts {
        choices: Choices,
    }

    #[derive(Debug, PartialEq)]
    struct Choices {
        values: Vec<Choice>,
    }

    impl FromMeta for Choices {
        fn from_list(items: &[NestedMeta]) -> darling::Result<Self> {
            let values = items
                .iter()
                .map(|item| match item {
                    NestedMeta::Meta(meta) => match meta {
                        Meta::Path(path) => Choice::from_string(
                            &path
                                .get_ident()
                                .ok_or(
                                    darling::Error::custom("choice must be an ident (no colon)")
                                        .with_span(path),
                                )?
                                .to_string(),
                        )
                        .map_err(|e| e.with_span(path)),
                        Meta::List(list) => Choice::from_list(&[item.clone()])
                            .map_err(|e| e.with_span(&list.span())),
                        Meta::NameValue(n) => Err(darling::Error::custom(
                            "choice options are not set as name-value, use parentheses",
                        )
                        .with_span(&n.eq_token)),
                    },
                    _ => {
                        Err(darling::Error::custom("literal is not a valid choice").with_span(item))
                    }
                })
                .collect::<Result<_, _>>()?;
            Ok(Self { values })
        }
    }

    #[derive(Debug, FromMeta, PartialEq)]
    enum Choice {
        One(One),
        Other,
    }

    #[derive(Debug, FromMeta, PartialEq)]
    struct One {
        foo: String,
    }

    for (tokens, expected) in [
        (
            parse_quote! {
                choices(one(foo = "bar"))
            },
            Ok(Opts {
                choices: Choices {
                    values: vec![Choice::One(One {
                        foo: "bar".to_string(),
                    })],
                },
            }),
        ),
        (
            parse_quote! {
                choices(other)
            },
            Ok(Opts {
                choices: Choices {
                    values: vec![Choice::Other],
                },
            }),
        ),
        (
            parse_quote! {
                choices(other, one(foo = "bar"))
            },
            Ok(Opts {
                choices: Choices {
                    values: vec![
                        Choice::Other,
                        Choice::One(One {
                            foo: "bar".to_string(),
                        }),
                    ],
                },
            }),
        ),
        (
            parse_quote! {
                choices(other(foo = "bar"))
            },
            Err("Unexpected meta-item format `non-path` at choices".to_string()),
        ),
    ] {
        assert_eq!(
            Opts::from_list(&NestedMeta::parse_meta_list(tokens).unwrap())
                .map_err(|e| e.to_string()),
            expected
        )
    }
}
