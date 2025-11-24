use serde_derive_internals::Ctxt;
use std::collections::{BTreeMap, BTreeSet};
use syn::Ident;

pub fn get_rename_format_type_params<'a>(
    errors: &Ctxt,
    rename_format_string: &syn::LitStr,
    generics: &'a syn::Generics,
) -> BTreeSet<&'a Ident> {
    let mut type_params = BTreeSet::new();
    let mut str_value = rename_format_string.value();

    if !str_value.contains('{') {
        return type_params;
    }

    if str_value.contains("{{") {
        str_value = str_value.replace("{{", "");
    }

    if str_value.contains("}}") {
        str_value = str_value.replace("}}", "");
    }

    let all_const_params =
        BTreeSet::from_iter(generics.const_params().map(|c| c.ident.to_string()));
    let all_type_params = BTreeMap::from_iter(
        generics
            .type_params()
            .map(|c| (c.ident.to_string(), &c.ident)),
    );

    let mut segments = str_value.split('{');

    if segments.next().unwrap_or_default().contains('}') {
        // The name format string contains a '}' before the first '{'
        errors.error_spanned_by(
            rename_format_string,
            "invalid name format string: unmatched `}` found",
        );
    }

    for segment in segments {
        match segment.split_once('}') {
            Some((param, rest)) => {
                if rest.contains('}') {
                    errors.error_spanned_by(
                        rename_format_string,
                        "invalid name format string: unmatched `}` found",
                    );
                }

                if let Some(type_param) = all_type_params.get(param) {
                    type_params.insert(type_param);
                } else if all_const_params.contains(param) {
                    // Any const params will be magically picked up from the surrounding scope by
                    // `format!()`
                } else {
                    errors.error_spanned_by(
                        rename_format_string,
                        format_args!(
                            "invalid name format string: expected generic param, found `{param}`"
                        ),
                    );
                }
            }
            None => {
                errors.error_spanned_by(
                    rename_format_string,
                    "invalid name format string: found `{` without matching `}`",
                );
            }
        }
    }

    type_params
}
