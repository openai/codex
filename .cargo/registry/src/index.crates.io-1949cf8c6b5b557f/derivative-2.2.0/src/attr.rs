use proc_macro2;
use syn;
use syn::spanned::Spanned;

/// Represent the `derivative` attributes on the input type (`struct`/`enum`).
#[derive(Debug, Default)]
pub struct Input {
    /// Whether `Clone` is present and its specific attributes.
    pub clone: Option<InputClone>,
    /// Whether `Copy` is present and its specific attributes.
    pub copy: Option<InputCopy>,
    /// Whether `Debug` is present and its specific attributes.
    pub debug: Option<InputDebug>,
    /// Whether `Default` is present and its specific attributes.
    pub default: Option<InputDefault>,
    /// Whether `Eq` is present and its specific attributes.
    pub eq: Option<InputEq>,
    /// Whether `Hash` is present and its specific attributes.
    pub hash: Option<InputHash>,
    /// Whether `PartialEq` is present and its specific attributes.
    pub partial_eq: Option<InputPartialEq>,
    /// Whether `PartialOrd` is present and its specific attributes.
    pub partial_ord: Option<InputPartialOrd>,
    /// Whether `Ord` is present and its specific attributes.
    pub ord: Option<InputOrd>,
    pub is_packed: bool,
}

#[derive(Debug, Default)]
/// Represent the `derivative` attributes on a field.
pub struct Field {
    /// The parameters for `Clone`.
    clone: FieldClone,
    /// The parameters for `Copy`.
    copy_bound: Option<Vec<syn::WherePredicate>>,
    /// The parameters for `Debug`.
    debug: FieldDebug,
    /// The parameters for `Default`.
    default: FieldDefault,
    /// The parameters for `Eq`.
    eq_bound: Option<Vec<syn::WherePredicate>>,
    /// The parameters for `Hash`.
    hash: FieldHash,
    /// The parameters for `PartialEq`.
    partial_eq: FieldPartialEq,
    /// The parameters for `PartialOrd`.
    partial_ord: FieldPartialOrd,
    /// The parameters for `Ord`.
    ord: FieldOrd,
}

#[derive(Debug, Default)]
/// Represent the `derivative(Clone(…))` attributes on an input.
pub struct InputClone {
    /// The `bound` attribute if present and the corresponding bounds.
    bounds: Option<Vec<syn::WherePredicate>>,
    /// Whether the implementation should have an explicit `clone_from`.
    pub clone_from: bool,
}

#[derive(Debug, Default)]
/// Represent the `derivative(Clone(…))` attributes on an input.
pub struct InputCopy {
    /// The `bound` attribute if present and the corresponding bounds.
    bounds: Option<Vec<syn::WherePredicate>>,
}

#[derive(Debug, Default)]
/// Represent the `derivative(Debug(…))` attributes on an input.
pub struct InputDebug {
    /// The `bound` attribute if present and the corresponding bounds.
    bounds: Option<Vec<syn::WherePredicate>>,
    /// Whether the type is marked `transparent`.
    pub transparent: bool,
}

#[derive(Debug, Default)]
/// Represent the `derivative(Default(…))` attributes on an input.
pub struct InputDefault {
    /// The `bound` attribute if present and the corresponding bounds.
    bounds: Option<Vec<syn::WherePredicate>>,
    /// Whether the type is marked with `new`.
    pub new: bool,
}

#[derive(Debug, Default)]
/// Represent the `derivative(Eq(…))` attributes on an input.
pub struct InputEq {
    /// The `bound` attribute if present and the corresponding bounds.
    bounds: Option<Vec<syn::WherePredicate>>,
}

#[derive(Debug, Default)]
/// Represent the `derivative(Hash(…))` attributes on an input.
pub struct InputHash {
    /// The `bound` attribute if present and the corresponding bounds.
    bounds: Option<Vec<syn::WherePredicate>>,
}

#[derive(Debug, Default)]
/// Represent the `derivative(PartialEq(…))` attributes on an input.
pub struct InputPartialEq {
    /// The `bound` attribute if present and the corresponding bounds.
    bounds: Option<Vec<syn::WherePredicate>>,
}

#[derive(Debug, Default)]
/// Represent the `derivative(PartialOrd(…))` attributes on an input.
pub struct InputPartialOrd {
    /// The `bound` attribute if present and the corresponding bounds.
    bounds: Option<Vec<syn::WherePredicate>>,
    /// Allow `derivative(PartialOrd)` on enums:
    on_enum: bool,
}

#[derive(Debug, Default)]
/// Represent the `derivative(Ord(…))` attributes on an input.
pub struct InputOrd {
    /// The `bound` attribute if present and the corresponding bounds.
    bounds: Option<Vec<syn::WherePredicate>>,
    /// Allow `derivative(Ord)` on enums:
    on_enum: bool,
}

#[derive(Debug, Default)]
/// Represents the `derivative(Clone(…))` attributes on a field.
pub struct FieldClone {
    /// The `bound` attribute if present and the corresponding bounds.
    bounds: Option<Vec<syn::WherePredicate>>,
    /// The `clone_with` attribute if present and the path to the cloning function.
    clone_with: Option<syn::Path>,
}

#[derive(Debug, Default)]
/// Represents the `derivative(Debug(…))` attributes on a field.
pub struct FieldDebug {
    /// The `bound` attribute if present and the corresponding bounds.
    bounds: Option<Vec<syn::WherePredicate>>,
    /// The `format_with` attribute if present and the path to the formatting function.
    format_with: Option<syn::Path>,
    /// Whether the field is to be ignored from output.
    ignore: bool,
}

#[derive(Debug, Default)]
/// Represent the `derivative(Default(…))` attributes on a field.
pub struct FieldDefault {
    /// The `bound` attribute if present and the corresponding bounds.
    bounds: Option<Vec<syn::WherePredicate>>,
    /// The default value for the field if present.
    pub value: Option<proc_macro2::TokenStream>,
}

#[derive(Debug, Default)]
/// Represents the `derivative(Hash(…))` attributes on a field.
pub struct FieldHash {
    /// The `bound` attribute if present and the corresponding bounds.
    bounds: Option<Vec<syn::WherePredicate>>,
    /// The `hash_with` attribute if present and the path to the hashing function.
    hash_with: Option<syn::Path>,
    /// Whether the field is to be ignored when hashing.
    ignore: bool,
}

#[derive(Debug, Default)]
/// Represent the `derivative(PartialEq(…))` attributes on a field.
pub struct FieldPartialEq {
    /// The `bound` attribute if present and the corresponding bounds.
    bounds: Option<Vec<syn::WherePredicate>>,
    /// The `compare_with` attribute if present and the path to the comparison function.
    compare_with: Option<syn::Path>,
    /// Whether the field is to be ignored when comparing.
    ignore: bool,
}

#[derive(Debug, Default)]
/// Represent the `derivative(PartialOrd(…))` attributes on a field.
pub struct FieldPartialOrd {
    /// The `bound` attribute if present and the corresponding bounds.
    bounds: Option<Vec<syn::WherePredicate>>,
    /// The `compare_with` attribute if present and the path to the comparison function.
    compare_with: Option<syn::Path>,
    /// Whether the field is to be ignored when comparing.
    ignore: bool,
}

#[derive(Debug, Default)]
/// Represent the `derivative(Ord(…))` attributes on a field.
pub struct FieldOrd {
    /// The `bound` attribute if present and the corresponding bounds.
    bounds: Option<Vec<syn::WherePredicate>>,
    /// The `compare_with` attribute if present and the path to the comparison function.
    compare_with: Option<syn::Path>,
    /// Whether the field is to be ignored when comparing.
    ignore: bool,
}

macro_rules! for_all_attr {
    ($errors:ident; for ($name:ident, $value:ident) in $attrs:expr; $($body:tt)*) => {
        for meta_items in $attrs.iter() {
            let meta_items = derivative_attribute(meta_items, $errors);
            if let Some(meta_items) = meta_items {
                for meta_item in meta_items.iter() {
                    let meta_item = read_items(meta_item, $errors);
                    let MetaItem($name, $value) = try!(meta_item);
                    match $name.to_string().as_ref() {
                        $($body)*
                    }
                }
            }
        }
    };
}

macro_rules! match_attributes {
    ($errors:ident for $trait:expr; let Some($name:ident) = $unwrapped:expr; for $value:ident in $values:expr; $($body:tt)* ) => {
        let mut $name = $unwrapped.take().unwrap_or_default();

        match_attributes! {
            $errors for $trait;
            for $value in $values;
            $($body)*
        }

        $unwrapped = Some($name);
    };

    ($errors:ident for $trait:expr; for $value:ident in $values:expr; $($body:tt)* ) => {
        for (name, $value) in $values {
            match name {
                Some(ident) => {
                    match ident.to_string().as_ref() {
                        $($body)*
                        unknown => {
                            let message = format!("Unknown attribute `{}` for trait `{}`", unknown, $trait);
                            $errors.extend(quote_spanned! {ident.span()=>
                                compile_error!(#message);
                            });
                        }
                    }
                }
                None => {
                    let value = $value.expect("Expected value to be passed");
                    match value.value().as_ref() {
                        $($body)*
                        unknown => {
                            let message = format!("Unknown attribute `{}` for trait `{}`", unknown, $trait);
                            let span = value.span();
                            $errors.extend(quote_spanned! {span=>
                                compile_error!(#message);
                            });
                        }
                    }
                }
            }
        }
    };
}

impl Input {
    /// Parse the `derivative` attributes on a type.
    #[allow(clippy::cognitive_complexity)] // mostly macros
    pub fn from_ast(
        attrs: &[syn::Attribute],
        errors: &mut proc_macro2::TokenStream,
    ) -> Result<Input, ()> {
        let mut input = Input {
            is_packed: attrs.iter().any(has_repr_packed_attr),
            ..Default::default()
        };

        for_all_attr! {
            errors;
            for (name, values) in attrs;
            "Clone" => {
                match_attributes! {
                    errors for "Clone";
                    let Some(clone) = input.clone;
                    for value in values;
                    "bound" => parse_bound(&mut clone.bounds, value, errors),
                    "clone_from" => {
                        clone.clone_from = parse_boolean_meta_item(value, true, "clone_from", errors);
                    }
                }
            }
            "Copy" => {
                match_attributes! {
                    errors for "Copy";
                    let Some(copy) = input.copy;
                    for value in values;
                    "bound" => parse_bound(&mut copy.bounds, value, errors),
                }
            }
            "Debug" => {
                match_attributes! {
                    errors for "Debug";
                    let Some(debug) = input.debug;
                    for value in values;
                    "bound" => parse_bound(&mut debug.bounds, value, errors),
                    "transparent" => {
                        debug.transparent = parse_boolean_meta_item(value, true, "transparent", errors);
                    }
                }
            }
            "Default" => {
                match_attributes! {
                    errors for "Default";
                    let Some(default) = input.default;
                    for value in values;
                    "bound" => parse_bound(&mut default.bounds, value, errors),
                    "new" => {
                        default.new = parse_boolean_meta_item(value, true, "new", errors);
                    }
                }
            }
            "Eq" => {
                match_attributes! {
                    errors for "Eq";
                    let Some(eq) = input.eq;
                    for value in values;
                    "bound" => parse_bound(&mut eq.bounds, value, errors),
                }
            }
            "Hash" => {
                match_attributes! {
                    errors for "Hash";
                    let Some(hash) = input.hash;
                    for value in values;
                    "bound" => parse_bound(&mut hash.bounds, value, errors),
                }
            }
            "PartialEq" => {
                match_attributes! {
                    errors for "PartialEq";
                    let Some(partial_eq) = input.partial_eq;
                    for value in values;
                    "bound" => parse_bound(&mut partial_eq.bounds, value, errors),
                    "feature_allow_slow_enum" => (), // backward compatibility, now unnecessary
                }
            }
            "PartialOrd" => {
                match_attributes! {
                    errors for "PartialOrd";
                    let Some(partial_ord) = input.partial_ord;
                    for value in values;
                    "bound" => parse_bound(&mut partial_ord.bounds, value, errors),
                    "feature_allow_slow_enum" => {
                        partial_ord.on_enum = parse_boolean_meta_item(value, true, "feature_allow_slow_enum", errors);
                    }
                }
            }
            "Ord" => {
                match_attributes! {
                    errors for "Ord";
                    let Some(ord) = input.ord;
                    for value in values;
                    "bound" => parse_bound(&mut ord.bounds, value, errors),
                    "feature_allow_slow_enum" => {
                        ord.on_enum = parse_boolean_meta_item(value, true, "feature_allow_slow_enum", errors);
                    }
                }
            }
            unknown => {
                let message = format!("deriving `{}` is not supported by derivative", unknown);
                errors.extend(quote_spanned! {name.span()=>
                    compile_error!(#message);
                });
            }
        }

        Ok(input)
    }

    pub fn clone_bound(&self) -> Option<&[syn::WherePredicate]> {
        self.clone
            .as_ref()
            .and_then(|d| d.bounds.as_ref().map(Vec::as_slice))
    }

    pub fn clone_from(&self) -> bool {
        self.clone.as_ref().map_or(false, |d| d.clone_from)
    }

    pub fn copy_bound(&self) -> Option<&[syn::WherePredicate]> {
        self.copy
            .as_ref()
            .and_then(|d| d.bounds.as_ref().map(Vec::as_slice))
    }

    pub fn debug_bound(&self) -> Option<&[syn::WherePredicate]> {
        self.debug
            .as_ref()
            .and_then(|d| d.bounds.as_ref().map(Vec::as_slice))
    }

    pub fn debug_transparent(&self) -> bool {
        self.debug.as_ref().map_or(false, |d| d.transparent)
    }

    pub fn default_bound(&self) -> Option<&[syn::WherePredicate]> {
        self.default
            .as_ref()
            .and_then(|d| d.bounds.as_ref().map(Vec::as_slice))
    }

    pub fn eq_bound(&self) -> Option<&[syn::WherePredicate]> {
        self.eq
            .as_ref()
            .and_then(|d| d.bounds.as_ref().map(Vec::as_slice))
    }

    pub fn hash_bound(&self) -> Option<&[syn::WherePredicate]> {
        self.hash
            .as_ref()
            .and_then(|d| d.bounds.as_ref().map(Vec::as_slice))
    }

    pub fn partial_eq_bound(&self) -> Option<&[syn::WherePredicate]> {
        self.partial_eq
            .as_ref()
            .and_then(|d| d.bounds.as_ref().map(Vec::as_slice))
    }

    pub fn partial_ord_bound(&self) -> Option<&[syn::WherePredicate]> {
        self.partial_ord
            .as_ref()
            .and_then(|d| d.bounds.as_ref().map(Vec::as_slice))
    }

    pub fn ord_bound(&self) -> Option<&[syn::WherePredicate]> {
        self.ord
            .as_ref()
            .and_then(|d| d.bounds.as_ref().map(Vec::as_slice))
    }

    pub fn partial_ord_on_enum(&self) -> bool {
        self.partial_ord.as_ref().map_or(false, |d| d.on_enum)
    }

    pub fn ord_on_enum(&self) -> bool {
        self.ord.as_ref().map_or(false, |d| d.on_enum)
    }
}

impl Field {
    /// Parse the `derivative` attributes on a type.
    #[allow(clippy::cognitive_complexity)] // mostly macros
    pub fn from_ast(
        field: &syn::Field,
        errors: &mut proc_macro2::TokenStream,
    ) -> Result<Field, ()> {
        let mut out = Field::default();

        for_all_attr! {
            errors;
            for (name, values) in field.attrs;
            "Clone" => {
                match_attributes! {
                    errors for "Clone";
                    for value in values;
                    "bound" => parse_bound(&mut out.clone.bounds, value, errors),
                    "clone_with" => {
                        let path = value.expect("`clone_with` needs a value");
                        out.clone.clone_with = parse_str_lit(&path, errors).ok();
                    }
                }
            }
            "Debug" => {
                match_attributes! {
                    errors for "Debug";
                    for value in values;
                    "bound" => parse_bound(&mut out.debug.bounds, value, errors),
                    "format_with" => {
                        let path = value.expect("`format_with` needs a value");
                        out.debug.format_with = parse_str_lit(&path, errors).ok();
                    }
                    "ignore" => {
                        out.debug.ignore = parse_boolean_meta_item(value, true, "ignore", errors);
                    }
                }
            }
            "Default" => {
                match_attributes! {
                    errors for "Default";
                    for value in values;
                    "bound" => parse_bound(&mut out.default.bounds, value, errors),
                    "value" => {
                        let value = value.expect("`value` needs a value");
                        out.default.value = parse_str_lit(&value, errors).ok();
                    }
                }
            }
            "Eq" => {
                match_attributes! {
                    errors for "Eq";
                    for value in values;
                    "bound" => parse_bound(&mut out.eq_bound, value, errors),
                }
            }
            "Hash" => {
                match_attributes! {
                    errors for "Hash";
                    for value in values;
                    "bound" => parse_bound(&mut out.hash.bounds, value, errors),
                    "hash_with" => {
                        let path = value.expect("`hash_with` needs a value");
                        out.hash.hash_with = parse_str_lit(&path, errors).ok();
                    }
                    "ignore" => {
                        out.hash.ignore = parse_boolean_meta_item(value, true, "ignore", errors);
                    }
                }
            }
            "PartialEq" => {
                match_attributes! {
                    errors for "PartialEq";
                    for value in values;
                    "bound" => parse_bound(&mut out.partial_eq.bounds, value, errors),
                    "compare_with" => {
                        let path = value.expect("`compare_with` needs a value");
                        out.partial_eq.compare_with = parse_str_lit(&path, errors).ok();
                    }
                    "ignore" => {
                        out.partial_eq.ignore = parse_boolean_meta_item(value, true, "ignore", errors);
                    }
                }
            }
            "PartialOrd" => {
                match_attributes! {
                    errors for "PartialOrd";
                    for value in values;
                    "bound" => parse_bound(&mut out.partial_ord.bounds, value, errors),
                    "compare_with" => {
                        let path = value.expect("`compare_with` needs a value");
                        out.partial_ord.compare_with = parse_str_lit(&path, errors).ok();
                    }
                    "ignore" => {
                        out.partial_ord.ignore = parse_boolean_meta_item(value, true, "ignore", errors);
                    }
                }
            }
            "Ord" => {
                match_attributes! {
                    errors for "Ord";
                    for value in values;
                    "bound" => parse_bound(&mut out.ord.bounds, value, errors),
                    "compare_with" => {
                        let path = value.expect("`compare_with` needs a value");
                        out.ord.compare_with = parse_str_lit(&path, errors).ok();
                    }
                    "ignore" => {
                        out.ord.ignore = parse_boolean_meta_item(value, true, "ignore", errors);
                    }
                }
            }
            unknown => {
                let message = format!("deriving `{}` is not supported by derivative", unknown);
                errors.extend(quote_spanned! {name.span()=>
                    compile_error!(#message);
                });
            }
        }

        Ok(out)
    }

    pub fn clone_bound(&self) -> Option<&[syn::WherePredicate]> {
        self.clone.bounds.as_ref().map(Vec::as_slice)
    }

    pub fn clone_with(&self) -> Option<&syn::Path> {
        self.clone.clone_with.as_ref()
    }

    pub fn copy_bound(&self) -> Option<&[syn::WherePredicate]> {
        self.copy_bound.as_ref().map(Vec::as_slice)
    }

    pub fn debug_bound(&self) -> Option<&[syn::WherePredicate]> {
        self.debug.bounds.as_ref().map(Vec::as_slice)
    }

    pub fn debug_format_with(&self) -> Option<&syn::Path> {
        self.debug.format_with.as_ref()
    }

    pub fn ignore_debug(&self) -> bool {
        self.debug.ignore
    }

    pub fn ignore_hash(&self) -> bool {
        self.hash.ignore
    }

    pub fn default_bound(&self) -> Option<&[syn::WherePredicate]> {
        self.default.bounds.as_ref().map(Vec::as_slice)
    }

    pub fn default_value(&self) -> Option<&proc_macro2::TokenStream> {
        self.default.value.as_ref()
    }

    pub fn eq_bound(&self) -> Option<&[syn::WherePredicate]> {
        self.eq_bound.as_ref().map(Vec::as_slice)
    }

    pub fn hash_bound(&self) -> Option<&[syn::WherePredicate]> {
        self.hash.bounds.as_ref().map(Vec::as_slice)
    }

    pub fn hash_with(&self) -> Option<&syn::Path> {
        self.hash.hash_with.as_ref()
    }

    pub fn partial_eq_bound(&self) -> Option<&[syn::WherePredicate]> {
        self.partial_eq.bounds.as_ref().map(Vec::as_slice)
    }

    pub fn partial_ord_bound(&self) -> Option<&[syn::WherePredicate]> {
        self.partial_ord.bounds.as_ref().map(Vec::as_slice)
    }

    pub fn ord_bound(&self) -> Option<&[syn::WherePredicate]> {
        self.ord.bounds.as_ref().map(Vec::as_slice)
    }

    pub fn partial_eq_compare_with(&self) -> Option<&syn::Path> {
        self.partial_eq.compare_with.as_ref()
    }

    pub fn partial_ord_compare_with(&self) -> Option<&syn::Path> {
        self.partial_ord.compare_with.as_ref()
    }

    pub fn ord_compare_with(&self) -> Option<&syn::Path> {
        self.ord.compare_with.as_ref()
    }

    pub fn ignore_partial_eq(&self) -> bool {
        self.partial_eq.ignore
    }

    pub fn ignore_partial_ord(&self) -> bool {
        self.partial_ord.ignore
    }

    pub fn ignore_ord(&self) -> bool {
        self.ord.ignore
    }
}

/// Represent an attribute.
///
/// We only have a limited set of possible attributes:
///
/// * `#[derivative(Debug)]` is represented as `(Debug, [])`;
/// * `#[derivative(Debug="foo")]` is represented as `(Debug, [(None, Some("foo"))])`;
/// * `#[derivative(Debug(foo="bar")]` is represented as `(Debug, [(Some(foo), Some("bar"))])`.
struct MetaItem<'a>(
    &'a syn::Ident,
    Vec<(Option<&'a syn::Ident>, Option<&'a syn::LitStr>)>,
);

/// Parse an arbitrary item for our limited `MetaItem` subset.
fn read_items<'a>(item: &'a syn::NestedMeta, errors: &mut proc_macro2::TokenStream) -> Result<MetaItem<'a>, ()> {
    let item = match *item {
        syn::NestedMeta::Meta(ref item) => item,
        syn::NestedMeta::Lit(ref lit) => {
            errors.extend(quote_spanned! {lit.span()=>
                compile_error!("expected meta-item but found literal");
            });

            return Err(());
        }
    };
    match *item {
        syn::Meta::Path(ref path) => match path.get_ident() {
            Some(name) => Ok(MetaItem(name, Vec::new())),
            None => {
                errors.extend(quote_spanned! {path.span()=>
                    compile_error!("expected derivative attribute to be a string, but found a path");
                });

                Err(())
            }
        },
        syn::Meta::List(syn::MetaList {
            ref path,
            nested: ref values,
            ..
        }) => {
            let values = values
                .iter()
                .map(|value| {
                    if let syn::NestedMeta::Meta(syn::Meta::NameValue(syn::MetaNameValue {
                        ref path,
                        lit: ref value,
                        ..
                    })) = *value
                    {
                        let (name, value) = ensure_str_lit(&path, &value, errors)?;

                        Ok((Some(name), Some(value)))
                    } else {
                        errors.extend(quote_spanned! {value.span()=>
                            compile_error!("expected named value");
                        });

                        Err(())
                    }
                })
                .collect::<Result<_, _>>()?;

            let name = match path.get_ident() {
                Some(name) => name,
                None => {
                    errors.extend(quote_spanned! {path.span()=>
                        compile_error!("expected derivative attribute to be a string, but found a path");
                    });

                    return Err(());
                }
            };

            Ok(MetaItem(name, values))
        }
        syn::Meta::NameValue(syn::MetaNameValue {
            ref path,
            lit: ref value,
            ..
        }) => {
            let (name, value) = ensure_str_lit(&path, &value, errors)?;

            Ok(MetaItem(name, vec![(None, Some(value))]))
        }
    }
}

/// Filter the `derivative` items from an attribute.
fn derivative_attribute(
    attribute: &syn::Attribute,
    errors: &mut proc_macro2::TokenStream,
) -> Option<syn::punctuated::Punctuated<syn::NestedMeta, syn::token::Comma>> {
    if !attribute.path.is_ident("derivative") {
        return None;
    }
    match attribute.parse_meta() {
        Ok(syn::Meta::List(meta_list)) => Some(meta_list.nested),
        Ok(_) => None,
        Err(e) => {
            let message = format!("invalid attribute: {}", e);
            errors.extend(quote_spanned! {e.span()=>
                compile_error!(#message);
            });

            None
        }
    }
}

/// Parse an item value as a boolean. Accepted values are the string literal `"true"` and
/// `"false"`. The `default` parameter specifies what the value of the boolean is when only its
/// name is specified (eg. `Debug="ignore"` is equivalent to `Debug(ignore="true")`). The `name`
/// parameter is used for error reporting.
fn parse_boolean_meta_item(
    item: Option<&syn::LitStr>,
    default: bool,
    name: &str,
    errors: &mut proc_macro2::TokenStream,
) -> bool {
    if let Some(item) = item.as_ref() {
        match item.value().as_ref() {
            "true" => true,
            "false" => false,
            val => {
                if val == name {
                    true
                } else {
                    let message = format!(
                        r#"expected `"true"` or `"false"` for `{}`, got `{}`"#,
                        name, val
                    );
                    errors.extend(quote_spanned! {item.span()=>
                        compile_error!(#message);
                    });

                    default
                }
            }
        }
    } else {
        default
    }
}

/// Parse a `bound` item.
fn parse_bound(
    opt_bounds: &mut Option<Vec<syn::WherePredicate>>,
    value: Option<&syn::LitStr>,
    errors: &mut proc_macro2::TokenStream,
) {
    let bound = value.expect("`bound` needs a value");
    let bound_value = bound.value();

    *opt_bounds = if !bound_value.is_empty() {
        let where_string = syn::LitStr::new(&format!("where {}", bound_value), bound.span());

        let bounds = parse_str_lit::<syn::WhereClause>(&where_string, errors)
            .map(|wh| wh.predicates.into_iter().collect());

        match bounds {
            Ok(bounds) => Some(bounds),
            Err(_) => {
                errors.extend(quote_spanned! {where_string.span()=>
                    compile_error!("could not parse bound");
                });

                None
            }
        }
    } else {
        Some(vec![])
    };
}

fn parse_str_lit<T>(value: &syn::LitStr, errors: &mut proc_macro2::TokenStream) -> Result<T, ()>
where
    T: syn::parse::Parse,
{
    match value.parse() {
        Ok(value) => Ok(value),
        Err(e) => {
            let message = format!("could not parse string literal: {}", e);
            errors.extend(quote_spanned! {value.span()=>
                compile_error!(#message);
            });
            Err(())
        }
    }
}

fn ensure_str_lit<'a>(
    attr_path: &'a syn::Path,
    lit: &'a syn::Lit,
    errors: &mut proc_macro2::TokenStream,
) -> Result<(&'a syn::Ident, &'a syn::LitStr), ()> {
    let attr_name = match attr_path.get_ident() {
        Some(attr_name) => attr_name,
        None => {
            errors.extend(quote_spanned! {attr_path.span()=>
                compile_error!("expected derivative attribute to be a string, but found a path");
            });
            return Err(());
        }
    };

    if let syn::Lit::Str(ref lit) = *lit {
        Ok((attr_name, lit))
    } else {
        let message = format!(
            "expected derivative {} attribute to be a string: `{} = \"...\"`",
            attr_name, attr_name
        );
        errors.extend(quote_spanned! {lit.span()=>
            compile_error!(#message);
        });
        Err(())
    }
}

pub fn has_repr_packed_attr(attr: &syn::Attribute) -> bool {
    if let Ok(attr) = attr.parse_meta() {
        if attr.path().get_ident().map(|i| i == "repr") == Some(true) {
            if let syn::Meta::List(items) = attr {
                for item in items.nested {
                    if let syn::NestedMeta::Meta(item) = item {
                        if item.path().get_ident().map(|i| i == "packed") == Some(true) {
                            return true;
                        }
                    }
                }
            }
        }
    }

    false
}
