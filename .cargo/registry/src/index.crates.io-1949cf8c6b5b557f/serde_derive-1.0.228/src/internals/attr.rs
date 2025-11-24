use crate::internals::name::{MultiName, Name};
use crate::internals::symbol::*;
use crate::internals::{ungroup, Ctxt};
use proc_macro2::{Spacing, Span, TokenStream, TokenTree};
use quote::ToTokens;
use std::collections::BTreeSet;
use std::iter::FromIterator;
use syn::meta::ParseNestedMeta;
use syn::parse::ParseStream;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{token, Ident, Lifetime, Token};

// This module handles parsing of `#[serde(...)]` attributes. The entrypoints
// are `attr::Container::from_ast`, `attr::Variant::from_ast`, and
// `attr::Field::from_ast`. Each returns an instance of the corresponding
// struct. Note that none of them return a Result. Unrecognized, malformed, or
// duplicated attributes result in a span_err but otherwise are ignored. The
// user will see errors simultaneously for all bad attributes in the crate
// rather than just the first.

pub use crate::internals::case::RenameRule;

pub(crate) struct Attr<'c, T> {
    cx: &'c Ctxt,
    name: Symbol,
    tokens: TokenStream,
    value: Option<T>,
}

impl<'c, T> Attr<'c, T> {
    fn none(cx: &'c Ctxt, name: Symbol) -> Self {
        Attr {
            cx,
            name,
            tokens: TokenStream::new(),
            value: None,
        }
    }

    fn set<A: ToTokens>(&mut self, obj: A, value: T) {
        let tokens = obj.into_token_stream();

        if self.value.is_some() {
            let msg = format!("duplicate serde attribute `{}`", self.name);
            self.cx.error_spanned_by(tokens, msg);
        } else {
            self.tokens = tokens;
            self.value = Some(value);
        }
    }

    fn set_opt<A: ToTokens>(&mut self, obj: A, value: Option<T>) {
        if let Some(value) = value {
            self.set(obj, value);
        }
    }

    fn set_if_none(&mut self, value: T) {
        if self.value.is_none() {
            self.value = Some(value);
        }
    }

    pub(crate) fn get(self) -> Option<T> {
        self.value
    }

    fn get_with_tokens(self) -> Option<(TokenStream, T)> {
        match self.value {
            Some(v) => Some((self.tokens, v)),
            None => None,
        }
    }
}

struct BoolAttr<'c>(Attr<'c, ()>);

impl<'c> BoolAttr<'c> {
    fn none(cx: &'c Ctxt, name: Symbol) -> Self {
        BoolAttr(Attr::none(cx, name))
    }

    fn set_true<A: ToTokens>(&mut self, obj: A) {
        self.0.set(obj, ());
    }

    fn get(&self) -> bool {
        self.0.value.is_some()
    }
}

pub(crate) struct VecAttr<'c, T> {
    cx: &'c Ctxt,
    name: Symbol,
    first_dup_tokens: TokenStream,
    values: Vec<T>,
}

impl<'c, T> VecAttr<'c, T> {
    fn none(cx: &'c Ctxt, name: Symbol) -> Self {
        VecAttr {
            cx,
            name,
            first_dup_tokens: TokenStream::new(),
            values: Vec::new(),
        }
    }

    fn insert<A: ToTokens>(&mut self, obj: A, value: T) {
        if self.values.len() == 1 {
            self.first_dup_tokens = obj.into_token_stream();
        }
        self.values.push(value);
    }

    fn at_most_one(mut self) -> Option<T> {
        if self.values.len() > 1 {
            let dup_token = self.first_dup_tokens;
            let msg = format!("duplicate serde attribute `{}`", self.name);
            self.cx.error_spanned_by(dup_token, msg);
            None
        } else {
            self.values.pop()
        }
    }

    pub(crate) fn get(self) -> Vec<T> {
        self.values
    }
}

fn unraw(ident: &Ident) -> Ident {
    Ident::new(ident.to_string().trim_start_matches("r#"), ident.span())
}

#[derive(Copy, Clone)]
pub struct RenameAllRules {
    pub serialize: RenameRule,
    pub deserialize: RenameRule,
}

impl RenameAllRules {
    /// Returns a new `RenameAllRules` with the individual rules of `self` and
    /// `other_rules` joined by `RenameRules::or`.
    pub fn or(self, other_rules: Self) -> Self {
        Self {
            serialize: self.serialize.or(other_rules.serialize),
            deserialize: self.deserialize.or(other_rules.deserialize),
        }
    }
}

/// Represents struct or enum attribute information.
pub struct Container {
    name: MultiName,
    transparent: bool,
    deny_unknown_fields: bool,
    default: Default,
    rename_all_rules: RenameAllRules,
    rename_all_fields_rules: RenameAllRules,
    ser_bound: Option<Vec<syn::WherePredicate>>,
    de_bound: Option<Vec<syn::WherePredicate>>,
    tag: TagType,
    type_from: Option<syn::Type>,
    type_try_from: Option<syn::Type>,
    type_into: Option<syn::Type>,
    remote: Option<syn::Path>,
    identifier: Identifier,
    serde_path: Option<syn::Path>,
    is_packed: bool,
    /// Error message generated when type can't be deserialized
    expecting: Option<String>,
    non_exhaustive: bool,
}

/// Styles of representing an enum.
pub enum TagType {
    /// The default.
    ///
    /// ```json
    /// {"variant1": {"key1": "value1", "key2": "value2"}}
    /// ```
    External,

    /// `#[serde(tag = "type")]`
    ///
    /// ```json
    /// {"type": "variant1", "key1": "value1", "key2": "value2"}
    /// ```
    Internal { tag: String },

    /// `#[serde(tag = "t", content = "c")]`
    ///
    /// ```json
    /// {"t": "variant1", "c": {"key1": "value1", "key2": "value2"}}
    /// ```
    Adjacent { tag: String, content: String },

    /// `#[serde(untagged)]`
    ///
    /// ```json
    /// {"key1": "value1", "key2": "value2"}
    /// ```
    None,
}

/// Whether this enum represents the fields of a struct or the variants of an
/// enum.
#[derive(Copy, Clone)]
pub enum Identifier {
    /// It does not.
    No,

    /// This enum represents the fields of a struct. All of the variants must be
    /// unit variants, except possibly one which is annotated with
    /// `#[serde(other)]` and is a newtype variant.
    Field,

    /// This enum represents the variants of an enum. All of the variants must
    /// be unit variants.
    Variant,
}

impl Identifier {
    #[cfg(feature = "deserialize_in_place")]
    pub fn is_some(self) -> bool {
        match self {
            Identifier::No => false,
            Identifier::Field | Identifier::Variant => true,
        }
    }
}

impl Container {
    /// Extract out the `#[serde(...)]` attributes from an item.
    pub fn from_ast(cx: &Ctxt, item: &syn::DeriveInput) -> Self {
        let mut ser_name = Attr::none(cx, RENAME);
        let mut de_name = Attr::none(cx, RENAME);
        let mut transparent = BoolAttr::none(cx, TRANSPARENT);
        let mut deny_unknown_fields = BoolAttr::none(cx, DENY_UNKNOWN_FIELDS);
        let mut default = Attr::none(cx, DEFAULT);
        let mut rename_all_ser_rule = Attr::none(cx, RENAME_ALL);
        let mut rename_all_de_rule = Attr::none(cx, RENAME_ALL);
        let mut rename_all_fields_ser_rule = Attr::none(cx, RENAME_ALL_FIELDS);
        let mut rename_all_fields_de_rule = Attr::none(cx, RENAME_ALL_FIELDS);
        let mut ser_bound = Attr::none(cx, BOUND);
        let mut de_bound = Attr::none(cx, BOUND);
        let mut untagged = BoolAttr::none(cx, UNTAGGED);
        let mut internal_tag = Attr::none(cx, TAG);
        let mut content = Attr::none(cx, CONTENT);
        let mut type_from = Attr::none(cx, FROM);
        let mut type_try_from = Attr::none(cx, TRY_FROM);
        let mut type_into = Attr::none(cx, INTO);
        let mut remote = Attr::none(cx, REMOTE);
        let mut field_identifier = BoolAttr::none(cx, FIELD_IDENTIFIER);
        let mut variant_identifier = BoolAttr::none(cx, VARIANT_IDENTIFIER);
        let mut serde_path = Attr::none(cx, CRATE);
        let mut expecting = Attr::none(cx, EXPECTING);
        let mut non_exhaustive = false;

        for attr in &item.attrs {
            if attr.path() != SERDE {
                non_exhaustive |=
                    matches!(&attr.meta, syn::Meta::Path(path) if path == NON_EXHAUSTIVE);
                continue;
            }

            if let syn::Meta::List(meta) = &attr.meta {
                if meta.tokens.is_empty() {
                    continue;
                }
            }

            if let Err(err) = attr.parse_nested_meta(|meta| {
                if meta.path == RENAME {
                    // #[serde(rename = "foo")]
                    // #[serde(rename(serialize = "foo", deserialize = "bar"))]
                    let (ser, de) = get_renames(cx, RENAME, &meta)?;
                    ser_name.set_opt(&meta.path, ser.as_ref().map(Name::from));
                    de_name.set_opt(&meta.path, de.as_ref().map(Name::from));
                } else if meta.path == RENAME_ALL {
                    // #[serde(rename_all = "foo")]
                    // #[serde(rename_all(serialize = "foo", deserialize = "bar"))]
                    let one_name = meta.input.peek(Token![=]);
                    let (ser, de) = get_renames(cx, RENAME_ALL, &meta)?;
                    if let Some(ser) = ser {
                        match RenameRule::from_str(&ser.value()) {
                            Ok(rename_rule) => rename_all_ser_rule.set(&meta.path, rename_rule),
                            Err(err) => cx.error_spanned_by(ser, err),
                        }
                    }
                    if let Some(de) = de {
                        match RenameRule::from_str(&de.value()) {
                            Ok(rename_rule) => rename_all_de_rule.set(&meta.path, rename_rule),
                            Err(err) => {
                                if !one_name {
                                    cx.error_spanned_by(de, err);
                                }
                            }
                        }
                    }
                } else if meta.path == RENAME_ALL_FIELDS {
                    // #[serde(rename_all_fields = "foo")]
                    // #[serde(rename_all_fields(serialize = "foo", deserialize = "bar"))]
                    let one_name = meta.input.peek(Token![=]);
                    let (ser, de) = get_renames(cx, RENAME_ALL_FIELDS, &meta)?;

                    match item.data {
                        syn::Data::Enum(_) => {
                            if let Some(ser) = ser {
                                match RenameRule::from_str(&ser.value()) {
                                    Ok(rename_rule) => {
                                        rename_all_fields_ser_rule.set(&meta.path, rename_rule);
                                    }
                                    Err(err) => cx.error_spanned_by(ser, err),
                                }
                            }
                            if let Some(de) = de {
                                match RenameRule::from_str(&de.value()) {
                                    Ok(rename_rule) => {
                                        rename_all_fields_de_rule.set(&meta.path, rename_rule);
                                    }
                                    Err(err) => {
                                        if !one_name {
                                            cx.error_spanned_by(de, err);
                                        }
                                    }
                                }
                            }
                        }
                        syn::Data::Struct(_) => {
                            let msg = "#[serde(rename_all_fields)] can only be used on enums";
                            cx.syn_error(meta.error(msg));
                        }
                        syn::Data::Union(_) => {
                            let msg = "#[serde(rename_all_fields)] can only be used on enums";
                            cx.syn_error(meta.error(msg));
                        }
                    }
                } else if meta.path == TRANSPARENT {
                    // #[serde(transparent)]
                    transparent.set_true(meta.path);
                } else if meta.path == DENY_UNKNOWN_FIELDS {
                    // #[serde(deny_unknown_fields)]
                    deny_unknown_fields.set_true(meta.path);
                } else if meta.path == DEFAULT {
                    if meta.input.peek(Token![=]) {
                        // #[serde(default = "...")]
                        if let Some(path) = parse_lit_into_expr_path(cx, DEFAULT, &meta)? {
                            match &item.data {
                                syn::Data::Struct(syn::DataStruct { fields, .. }) => match fields {
                                    syn::Fields::Named(_) | syn::Fields::Unnamed(_) => {
                                        default.set(&meta.path, Default::Path(path));
                                    }
                                    syn::Fields::Unit => {
                                        let msg = "#[serde(default = \"...\")] can only be used on structs that have fields";
                                        cx.syn_error(meta.error(msg));
                                    }
                                },
                                syn::Data::Enum(_) => {
                                    let msg = "#[serde(default = \"...\")] can only be used on structs";
                                    cx.syn_error(meta.error(msg));
                                }
                                syn::Data::Union(_) => {
                                    let msg = "#[serde(default = \"...\")] can only be used on structs";
                                    cx.syn_error(meta.error(msg));
                                }
                            }
                        }
                    } else {
                        // #[serde(default)]
                        match &item.data {
                            syn::Data::Struct(syn::DataStruct { fields, .. }) => match fields {
                                syn::Fields::Named(_) | syn::Fields::Unnamed(_) => {
                                    default.set(meta.path, Default::Default);
                                }
                                syn::Fields::Unit => {
                                    let msg = "#[serde(default)] can only be used on structs that have fields";
                                    cx.error_spanned_by(fields, msg);
                                }
                            },
                            syn::Data::Enum(_) => {
                                let msg = "#[serde(default)] can only be used on structs";
                                cx.syn_error(meta.error(msg));
                            }
                            syn::Data::Union(_) => {
                                let msg = "#[serde(default)] can only be used on structs";
                                cx.syn_error(meta.error(msg));
                            }
                        }
                    }
                } else if meta.path == BOUND {
                    // #[serde(bound = "T: SomeBound")]
                    // #[serde(bound(serialize = "...", deserialize = "..."))]
                    let (ser, de) = get_where_predicates(cx, &meta)?;
                    ser_bound.set_opt(&meta.path, ser);
                    de_bound.set_opt(&meta.path, de);
                } else if meta.path == UNTAGGED {
                    // #[serde(untagged)]
                    match item.data {
                        syn::Data::Enum(_) => {
                            untagged.set_true(&meta.path);
                        }
                        syn::Data::Struct(_) => {
                            let msg = "#[serde(untagged)] can only be used on enums";
                            cx.syn_error(meta.error(msg));
                        }
                        syn::Data::Union(_) => {
                            let msg = "#[serde(untagged)] can only be used on enums";
                            cx.syn_error(meta.error(msg));
                        }
                    }
                } else if meta.path == TAG {
                    // #[serde(tag = "type")]
                    if let Some(s) = get_lit_str(cx, TAG, &meta)? {
                        match &item.data {
                            syn::Data::Enum(_) => {
                                internal_tag.set(&meta.path, s.value());
                            }
                            syn::Data::Struct(syn::DataStruct { fields, .. }) => match fields {
                                syn::Fields::Named(_) => {
                                    internal_tag.set(&meta.path, s.value());
                                }
                                syn::Fields::Unnamed(_) | syn::Fields::Unit => {
                                    let msg = "#[serde(tag = \"...\")] can only be used on enums and structs with named fields";
                                    cx.syn_error(meta.error(msg));
                                }
                            },
                            syn::Data::Union(_) => {
                                let msg = "#[serde(tag = \"...\")] can only be used on enums and structs with named fields";
                                cx.syn_error(meta.error(msg));
                            }
                        }
                    }
                } else if meta.path == CONTENT {
                    // #[serde(content = "c")]
                    if let Some(s) = get_lit_str(cx, CONTENT, &meta)? {
                        match &item.data {
                            syn::Data::Enum(_) => {
                                content.set(&meta.path, s.value());
                            }
                            syn::Data::Struct(_) => {
                                let msg = "#[serde(content = \"...\")] can only be used on enums";
                                cx.syn_error(meta.error(msg));
                            }
                            syn::Data::Union(_) => {
                                let msg = "#[serde(content = \"...\")] can only be used on enums";
                                cx.syn_error(meta.error(msg));
                            }
                        }
                    }
                } else if meta.path == FROM {
                    // #[serde(from = "Type")]
                    if let Some(from_ty) = parse_lit_into_ty(cx, FROM, &meta)? {
                        type_from.set_opt(&meta.path, Some(from_ty));
                    }
                } else if meta.path == TRY_FROM {
                    // #[serde(try_from = "Type")]
                    if let Some(try_from_ty) = parse_lit_into_ty(cx, TRY_FROM, &meta)? {
                        type_try_from.set_opt(&meta.path, Some(try_from_ty));
                    }
                } else if meta.path == INTO {
                    // #[serde(into = "Type")]
                    if let Some(into_ty) = parse_lit_into_ty(cx, INTO, &meta)? {
                        type_into.set_opt(&meta.path, Some(into_ty));
                    }
                } else if meta.path == REMOTE {
                    // #[serde(remote = "...")]
                    if let Some(path) = parse_lit_into_path(cx, REMOTE, &meta)? {
                        if is_primitive_path(&path, "Self") {
                            remote.set(&meta.path, item.ident.clone().into());
                        } else {
                            remote.set(&meta.path, path);
                        }
                    }
                } else if meta.path == FIELD_IDENTIFIER {
                    // #[serde(field_identifier)]
                    field_identifier.set_true(&meta.path);
                } else if meta.path == VARIANT_IDENTIFIER {
                    // #[serde(variant_identifier)]
                    variant_identifier.set_true(&meta.path);
                } else if meta.path == CRATE {
                    // #[serde(crate = "foo")]
                    if let Some(path) = parse_lit_into_path(cx, CRATE, &meta)? {
                        serde_path.set(&meta.path, path);
                    }
                } else if meta.path == EXPECTING {
                    // #[serde(expecting = "a message")]
                    if let Some(s) = get_lit_str(cx, EXPECTING, &meta)? {
                        expecting.set(&meta.path, s.value());
                    }
                } else {
                    let path = meta.path.to_token_stream().to_string().replace(' ', "");
                    return Err(
                        meta.error(format_args!("unknown serde container attribute `{}`", path))
                    );
                }
                Ok(())
            }) {
                cx.syn_error(err);
            }
        }

        let mut is_packed = false;
        for attr in &item.attrs {
            if attr.path() == REPR {
                let _ = attr.parse_args_with(|input: ParseStream| {
                    while let Some(token) = input.parse()? {
                        if let TokenTree::Ident(ident) = token {
                            is_packed |= ident == "packed";
                        }
                    }
                    Ok(())
                });
            }
        }

        Container {
            name: MultiName::from_attrs(Name::from(&unraw(&item.ident)), ser_name, de_name, None),
            transparent: transparent.get(),
            deny_unknown_fields: deny_unknown_fields.get(),
            default: default.get().unwrap_or(Default::None),
            rename_all_rules: RenameAllRules {
                serialize: rename_all_ser_rule.get().unwrap_or(RenameRule::None),
                deserialize: rename_all_de_rule.get().unwrap_or(RenameRule::None),
            },
            rename_all_fields_rules: RenameAllRules {
                serialize: rename_all_fields_ser_rule.get().unwrap_or(RenameRule::None),
                deserialize: rename_all_fields_de_rule.get().unwrap_or(RenameRule::None),
            },
            ser_bound: ser_bound.get(),
            de_bound: de_bound.get(),
            tag: decide_tag(cx, item, untagged, internal_tag, content),
            type_from: type_from.get(),
            type_try_from: type_try_from.get(),
            type_into: type_into.get(),
            remote: remote.get(),
            identifier: decide_identifier(cx, item, field_identifier, variant_identifier),
            serde_path: serde_path.get(),
            is_packed,
            expecting: expecting.get(),
            non_exhaustive,
        }
    }

    pub fn name(&self) -> &MultiName {
        &self.name
    }

    pub fn rename_all_rules(&self) -> RenameAllRules {
        self.rename_all_rules
    }

    pub fn rename_all_fields_rules(&self) -> RenameAllRules {
        self.rename_all_fields_rules
    }

    pub fn transparent(&self) -> bool {
        self.transparent
    }

    pub fn deny_unknown_fields(&self) -> bool {
        self.deny_unknown_fields
    }

    pub fn default(&self) -> &Default {
        &self.default
    }

    pub fn ser_bound(&self) -> Option<&[syn::WherePredicate]> {
        self.ser_bound.as_ref().map(|vec| &vec[..])
    }

    pub fn de_bound(&self) -> Option<&[syn::WherePredicate]> {
        self.de_bound.as_ref().map(|vec| &vec[..])
    }

    pub fn tag(&self) -> &TagType {
        &self.tag
    }

    pub fn type_from(&self) -> Option<&syn::Type> {
        self.type_from.as_ref()
    }

    pub fn type_try_from(&self) -> Option<&syn::Type> {
        self.type_try_from.as_ref()
    }

    pub fn type_into(&self) -> Option<&syn::Type> {
        self.type_into.as_ref()
    }

    pub fn remote(&self) -> Option<&syn::Path> {
        self.remote.as_ref()
    }

    pub fn is_packed(&self) -> bool {
        self.is_packed
    }

    pub fn identifier(&self) -> Identifier {
        self.identifier
    }

    pub fn custom_serde_path(&self) -> Option<&syn::Path> {
        self.serde_path.as_ref()
    }

    /// Error message generated when type can't be deserialized.
    /// If `None`, default message will be used
    pub fn expecting(&self) -> Option<&str> {
        self.expecting.as_ref().map(String::as_ref)
    }

    pub fn non_exhaustive(&self) -> bool {
        self.non_exhaustive
    }
}

fn decide_tag(
    cx: &Ctxt,
    item: &syn::DeriveInput,
    untagged: BoolAttr,
    internal_tag: Attr<String>,
    content: Attr<String>,
) -> TagType {
    match (
        untagged.0.get_with_tokens(),
        internal_tag.get_with_tokens(),
        content.get_with_tokens(),
    ) {
        (None, None, None) => TagType::External,
        (Some(_), None, None) => TagType::None,
        (None, Some((_, tag)), None) => {
            // Check that there are no tuple variants.
            if let syn::Data::Enum(data) = &item.data {
                for variant in &data.variants {
                    match &variant.fields {
                        syn::Fields::Named(_) | syn::Fields::Unit => {}
                        syn::Fields::Unnamed(fields) => {
                            if fields.unnamed.len() != 1 {
                                let msg =
                                    "#[serde(tag = \"...\")] cannot be used with tuple variants";
                                cx.error_spanned_by(variant, msg);
                                break;
                            }
                        }
                    }
                }
            }
            TagType::Internal { tag }
        }
        (Some((untagged_tokens, ())), Some((tag_tokens, _)), None) => {
            let msg = "enum cannot be both untagged and internally tagged";
            cx.error_spanned_by(untagged_tokens, msg);
            cx.error_spanned_by(tag_tokens, msg);
            TagType::External // doesn't matter, will error
        }
        (None, None, Some((content_tokens, _))) => {
            let msg = "#[serde(tag = \"...\", content = \"...\")] must be used together";
            cx.error_spanned_by(content_tokens, msg);
            TagType::External
        }
        (Some((untagged_tokens, ())), None, Some((content_tokens, _))) => {
            let msg = "untagged enum cannot have #[serde(content = \"...\")]";
            cx.error_spanned_by(untagged_tokens, msg);
            cx.error_spanned_by(content_tokens, msg);
            TagType::External
        }
        (None, Some((_, tag)), Some((_, content))) => TagType::Adjacent { tag, content },
        (Some((untagged_tokens, ())), Some((tag_tokens, _)), Some((content_tokens, _))) => {
            let msg = "untagged enum cannot have #[serde(tag = \"...\", content = \"...\")]";
            cx.error_spanned_by(untagged_tokens, msg);
            cx.error_spanned_by(tag_tokens, msg);
            cx.error_spanned_by(content_tokens, msg);
            TagType::External
        }
    }
}

fn decide_identifier(
    cx: &Ctxt,
    item: &syn::DeriveInput,
    field_identifier: BoolAttr,
    variant_identifier: BoolAttr,
) -> Identifier {
    match (
        &item.data,
        field_identifier.0.get_with_tokens(),
        variant_identifier.0.get_with_tokens(),
    ) {
        (_, None, None) => Identifier::No,
        (_, Some((field_identifier_tokens, ())), Some((variant_identifier_tokens, ()))) => {
            let msg =
                "#[serde(field_identifier)] and #[serde(variant_identifier)] cannot both be set";
            cx.error_spanned_by(field_identifier_tokens, msg);
            cx.error_spanned_by(variant_identifier_tokens, msg);
            Identifier::No
        }
        (syn::Data::Enum(_), Some(_), None) => Identifier::Field,
        (syn::Data::Enum(_), None, Some(_)) => Identifier::Variant,
        (syn::Data::Struct(syn::DataStruct { struct_token, .. }), Some(_), None) => {
            let msg = "#[serde(field_identifier)] can only be used on an enum";
            cx.error_spanned_by(struct_token, msg);
            Identifier::No
        }
        (syn::Data::Union(syn::DataUnion { union_token, .. }), Some(_), None) => {
            let msg = "#[serde(field_identifier)] can only be used on an enum";
            cx.error_spanned_by(union_token, msg);
            Identifier::No
        }
        (syn::Data::Struct(syn::DataStruct { struct_token, .. }), None, Some(_)) => {
            let msg = "#[serde(variant_identifier)] can only be used on an enum";
            cx.error_spanned_by(struct_token, msg);
            Identifier::No
        }
        (syn::Data::Union(syn::DataUnion { union_token, .. }), None, Some(_)) => {
            let msg = "#[serde(variant_identifier)] can only be used on an enum";
            cx.error_spanned_by(union_token, msg);
            Identifier::No
        }
    }
}

/// Represents variant attribute information
pub struct Variant {
    name: MultiName,
    rename_all_rules: RenameAllRules,
    ser_bound: Option<Vec<syn::WherePredicate>>,
    de_bound: Option<Vec<syn::WherePredicate>>,
    skip_deserializing: bool,
    skip_serializing: bool,
    other: bool,
    serialize_with: Option<syn::ExprPath>,
    deserialize_with: Option<syn::ExprPath>,
    borrow: Option<BorrowAttribute>,
    untagged: bool,
}

struct BorrowAttribute {
    path: syn::Path,
    lifetimes: Option<BTreeSet<syn::Lifetime>>,
}

impl Variant {
    pub fn from_ast(cx: &Ctxt, variant: &syn::Variant) -> Self {
        let mut ser_name = Attr::none(cx, RENAME);
        let mut de_name = Attr::none(cx, RENAME);
        let mut de_aliases = VecAttr::none(cx, RENAME);
        let mut skip_deserializing = BoolAttr::none(cx, SKIP_DESERIALIZING);
        let mut skip_serializing = BoolAttr::none(cx, SKIP_SERIALIZING);
        let mut rename_all_ser_rule = Attr::none(cx, RENAME_ALL);
        let mut rename_all_de_rule = Attr::none(cx, RENAME_ALL);
        let mut ser_bound = Attr::none(cx, BOUND);
        let mut de_bound = Attr::none(cx, BOUND);
        let mut other = BoolAttr::none(cx, OTHER);
        let mut serialize_with = Attr::none(cx, SERIALIZE_WITH);
        let mut deserialize_with = Attr::none(cx, DESERIALIZE_WITH);
        let mut borrow = Attr::none(cx, BORROW);
        let mut untagged = BoolAttr::none(cx, UNTAGGED);

        for attr in &variant.attrs {
            if attr.path() != SERDE {
                continue;
            }

            if let syn::Meta::List(meta) = &attr.meta {
                if meta.tokens.is_empty() {
                    continue;
                }
            }

            if let Err(err) = attr.parse_nested_meta(|meta| {
                if meta.path == RENAME {
                    // #[serde(rename = "foo")]
                    // #[serde(rename(serialize = "foo", deserialize = "bar"))]
                    let (ser, de) = get_multiple_renames(cx, &meta)?;
                    ser_name.set_opt(&meta.path, ser.as_ref().map(Name::from));
                    for de_value in de {
                        de_name.set_if_none(Name::from(&de_value));
                        de_aliases.insert(&meta.path, Name::from(&de_value));
                    }
                } else if meta.path == ALIAS {
                    // #[serde(alias = "foo")]
                    if let Some(s) = get_lit_str(cx, ALIAS, &meta)? {
                        de_aliases.insert(&meta.path, Name::from(&s));
                    }
                } else if meta.path == RENAME_ALL {
                    // #[serde(rename_all = "foo")]
                    // #[serde(rename_all(serialize = "foo", deserialize = "bar"))]
                    let one_name = meta.input.peek(Token![=]);
                    let (ser, de) = get_renames(cx, RENAME_ALL, &meta)?;
                    if let Some(ser) = ser {
                        match RenameRule::from_str(&ser.value()) {
                            Ok(rename_rule) => rename_all_ser_rule.set(&meta.path, rename_rule),
                            Err(err) => cx.error_spanned_by(ser, err),
                        }
                    }
                    if let Some(de) = de {
                        match RenameRule::from_str(&de.value()) {
                            Ok(rename_rule) => rename_all_de_rule.set(&meta.path, rename_rule),
                            Err(err) => {
                                if !one_name {
                                    cx.error_spanned_by(de, err);
                                }
                            }
                        }
                    }
                } else if meta.path == SKIP {
                    // #[serde(skip)]
                    skip_serializing.set_true(&meta.path);
                    skip_deserializing.set_true(&meta.path);
                } else if meta.path == SKIP_DESERIALIZING {
                    // #[serde(skip_deserializing)]
                    skip_deserializing.set_true(&meta.path);
                } else if meta.path == SKIP_SERIALIZING {
                    // #[serde(skip_serializing)]
                    skip_serializing.set_true(&meta.path);
                } else if meta.path == OTHER {
                    // #[serde(other)]
                    other.set_true(&meta.path);
                } else if meta.path == BOUND {
                    // #[serde(bound = "T: SomeBound")]
                    // #[serde(bound(serialize = "...", deserialize = "..."))]
                    let (ser, de) = get_where_predicates(cx, &meta)?;
                    ser_bound.set_opt(&meta.path, ser);
                    de_bound.set_opt(&meta.path, de);
                } else if meta.path == WITH {
                    // #[serde(with = "...")]
                    if let Some(path) = parse_lit_into_expr_path(cx, WITH, &meta)? {
                        let mut ser_path = path.clone();
                        ser_path
                            .path
                            .segments
                            .push(Ident::new("serialize", ser_path.span()).into());
                        serialize_with.set(&meta.path, ser_path);
                        let mut de_path = path;
                        de_path
                            .path
                            .segments
                            .push(Ident::new("deserialize", de_path.span()).into());
                        deserialize_with.set(&meta.path, de_path);
                    }
                } else if meta.path == SERIALIZE_WITH {
                    // #[serde(serialize_with = "...")]
                    if let Some(path) = parse_lit_into_expr_path(cx, SERIALIZE_WITH, &meta)? {
                        serialize_with.set(&meta.path, path);
                    }
                } else if meta.path == DESERIALIZE_WITH {
                    // #[serde(deserialize_with = "...")]
                    if let Some(path) = parse_lit_into_expr_path(cx, DESERIALIZE_WITH, &meta)? {
                        deserialize_with.set(&meta.path, path);
                    }
                } else if meta.path == BORROW {
                    let borrow_attribute = if meta.input.peek(Token![=]) {
                        // #[serde(borrow = "'a + 'b")]
                        let lifetimes = parse_lit_into_lifetimes(cx, &meta)?;
                        BorrowAttribute {
                            path: meta.path.clone(),
                            lifetimes: Some(lifetimes),
                        }
                    } else {
                        // #[serde(borrow)]
                        BorrowAttribute {
                            path: meta.path.clone(),
                            lifetimes: None,
                        }
                    };
                    match &variant.fields {
                        syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                            borrow.set(&meta.path, borrow_attribute);
                        }
                        _ => {
                            let msg = "#[serde(borrow)] may only be used on newtype variants";
                            cx.error_spanned_by(variant, msg);
                        }
                    }
                } else if meta.path == UNTAGGED {
                    untagged.set_true(&meta.path);
                } else {
                    let path = meta.path.to_token_stream().to_string().replace(' ', "");
                    return Err(
                        meta.error(format_args!("unknown serde variant attribute `{}`", path))
                    );
                }
                Ok(())
            }) {
                cx.syn_error(err);
            }
        }

        Variant {
            name: MultiName::from_attrs(
                Name::from(&unraw(&variant.ident)),
                ser_name,
                de_name,
                Some(de_aliases),
            ),
            rename_all_rules: RenameAllRules {
                serialize: rename_all_ser_rule.get().unwrap_or(RenameRule::None),
                deserialize: rename_all_de_rule.get().unwrap_or(RenameRule::None),
            },
            ser_bound: ser_bound.get(),
            de_bound: de_bound.get(),
            skip_deserializing: skip_deserializing.get(),
            skip_serializing: skip_serializing.get(),
            other: other.get(),
            serialize_with: serialize_with.get(),
            deserialize_with: deserialize_with.get(),
            borrow: borrow.get(),
            untagged: untagged.get(),
        }
    }

    pub fn name(&self) -> &MultiName {
        &self.name
    }

    pub fn aliases(&self) -> &BTreeSet<Name> {
        self.name.deserialize_aliases()
    }

    pub fn rename_by_rules(&mut self, rules: RenameAllRules) {
        if !self.name.serialize_renamed {
            self.name.serialize.value =
                rules.serialize.apply_to_variant(&self.name.serialize.value);
        }
        if !self.name.deserialize_renamed {
            self.name.deserialize.value = rules
                .deserialize
                .apply_to_variant(&self.name.deserialize.value);
        }
        self.name
            .deserialize_aliases
            .insert(self.name.deserialize.clone());
    }

    pub fn rename_all_rules(&self) -> RenameAllRules {
        self.rename_all_rules
    }

    pub fn ser_bound(&self) -> Option<&[syn::WherePredicate]> {
        self.ser_bound.as_ref().map(|vec| &vec[..])
    }

    pub fn de_bound(&self) -> Option<&[syn::WherePredicate]> {
        self.de_bound.as_ref().map(|vec| &vec[..])
    }

    pub fn skip_deserializing(&self) -> bool {
        self.skip_deserializing
    }

    pub fn skip_serializing(&self) -> bool {
        self.skip_serializing
    }

    pub fn other(&self) -> bool {
        self.other
    }

    pub fn serialize_with(&self) -> Option<&syn::ExprPath> {
        self.serialize_with.as_ref()
    }

    pub fn deserialize_with(&self) -> Option<&syn::ExprPath> {
        self.deserialize_with.as_ref()
    }

    pub fn untagged(&self) -> bool {
        self.untagged
    }
}

/// Represents field attribute information
pub struct Field {
    name: MultiName,
    skip_serializing: bool,
    skip_deserializing: bool,
    skip_serializing_if: Option<syn::ExprPath>,
    default: Default,
    serialize_with: Option<syn::ExprPath>,
    deserialize_with: Option<syn::ExprPath>,
    ser_bound: Option<Vec<syn::WherePredicate>>,
    de_bound: Option<Vec<syn::WherePredicate>>,
    borrowed_lifetimes: BTreeSet<syn::Lifetime>,
    getter: Option<syn::ExprPath>,
    flatten: bool,
    transparent: bool,
}

/// Represents the default to use for a field when deserializing.
pub enum Default {
    /// Field must always be specified because it does not have a default.
    None,
    /// The default is given by `std::default::Default::default()`.
    Default,
    /// The default is given by this function.
    Path(syn::ExprPath),
}

impl Default {
    pub fn is_none(&self) -> bool {
        match self {
            Default::None => true,
            Default::Default | Default::Path(_) => false,
        }
    }
}

impl Field {
    /// Extract out the `#[serde(...)]` attributes from a struct field.
    pub fn from_ast(
        cx: &Ctxt,
        index: usize,
        field: &syn::Field,
        attrs: Option<&Variant>,
        container_default: &Default,
        private: &Ident,
    ) -> Self {
        let mut ser_name = Attr::none(cx, RENAME);
        let mut de_name = Attr::none(cx, RENAME);
        let mut de_aliases = VecAttr::none(cx, RENAME);
        let mut skip_serializing = BoolAttr::none(cx, SKIP_SERIALIZING);
        let mut skip_deserializing = BoolAttr::none(cx, SKIP_DESERIALIZING);
        let mut skip_serializing_if = Attr::none(cx, SKIP_SERIALIZING_IF);
        let mut default = Attr::none(cx, DEFAULT);
        let mut serialize_with = Attr::none(cx, SERIALIZE_WITH);
        let mut deserialize_with = Attr::none(cx, DESERIALIZE_WITH);
        let mut ser_bound = Attr::none(cx, BOUND);
        let mut de_bound = Attr::none(cx, BOUND);
        let mut borrowed_lifetimes = Attr::none(cx, BORROW);
        let mut getter = Attr::none(cx, GETTER);
        let mut flatten = BoolAttr::none(cx, FLATTEN);

        let ident = match &field.ident {
            Some(ident) => Name::from(&unraw(ident)),
            None => Name {
                value: index.to_string(),
                span: Span::call_site(),
            },
        };

        if let Some(borrow_attribute) = attrs.and_then(|variant| variant.borrow.as_ref()) {
            if let Ok(borrowable) = borrowable_lifetimes(cx, &ident, field) {
                if let Some(lifetimes) = &borrow_attribute.lifetimes {
                    for lifetime in lifetimes {
                        if !borrowable.contains(lifetime) {
                            let msg =
                                format!("field `{}` does not have lifetime {}", ident, lifetime);
                            cx.error_spanned_by(field, msg);
                        }
                    }
                    borrowed_lifetimes.set(&borrow_attribute.path, lifetimes.clone());
                } else {
                    borrowed_lifetimes.set(&borrow_attribute.path, borrowable);
                }
            }
        }

        for attr in &field.attrs {
            if attr.path() != SERDE {
                continue;
            }

            if let syn::Meta::List(meta) = &attr.meta {
                if meta.tokens.is_empty() {
                    continue;
                }
            }

            if let Err(err) = attr.parse_nested_meta(|meta| {
                if meta.path == RENAME {
                    // #[serde(rename = "foo")]
                    // #[serde(rename(serialize = "foo", deserialize = "bar"))]
                    let (ser, de) = get_multiple_renames(cx, &meta)?;
                    ser_name.set_opt(&meta.path, ser.as_ref().map(Name::from));
                    for de_value in de {
                        de_name.set_if_none(Name::from(&de_value));
                        de_aliases.insert(&meta.path, Name::from(&de_value));
                    }
                } else if meta.path == ALIAS {
                    // #[serde(alias = "foo")]
                    if let Some(s) = get_lit_str(cx, ALIAS, &meta)? {
                        de_aliases.insert(&meta.path, Name::from(&s));
                    }
                } else if meta.path == DEFAULT {
                    if meta.input.peek(Token![=]) {
                        // #[serde(default = "...")]
                        if let Some(path) = parse_lit_into_expr_path(cx, DEFAULT, &meta)? {
                            default.set(&meta.path, Default::Path(path));
                        }
                    } else {
                        // #[serde(default)]
                        default.set(&meta.path, Default::Default);
                    }
                } else if meta.path == SKIP_SERIALIZING {
                    // #[serde(skip_serializing)]
                    skip_serializing.set_true(&meta.path);
                } else if meta.path == SKIP_DESERIALIZING {
                    // #[serde(skip_deserializing)]
                    skip_deserializing.set_true(&meta.path);
                } else if meta.path == SKIP {
                    // #[serde(skip)]
                    skip_serializing.set_true(&meta.path);
                    skip_deserializing.set_true(&meta.path);
                } else if meta.path == SKIP_SERIALIZING_IF {
                    // #[serde(skip_serializing_if = "...")]
                    if let Some(path) = parse_lit_into_expr_path(cx, SKIP_SERIALIZING_IF, &meta)? {
                        skip_serializing_if.set(&meta.path, path);
                    }
                } else if meta.path == SERIALIZE_WITH {
                    // #[serde(serialize_with = "...")]
                    if let Some(path) = parse_lit_into_expr_path(cx, SERIALIZE_WITH, &meta)? {
                        serialize_with.set(&meta.path, path);
                    }
                } else if meta.path == DESERIALIZE_WITH {
                    // #[serde(deserialize_with = "...")]
                    if let Some(path) = parse_lit_into_expr_path(cx, DESERIALIZE_WITH, &meta)? {
                        deserialize_with.set(&meta.path, path);
                    }
                } else if meta.path == WITH {
                    // #[serde(with = "...")]
                    if let Some(path) = parse_lit_into_expr_path(cx, WITH, &meta)? {
                        let mut ser_path = path.clone();
                        ser_path
                            .path
                            .segments
                            .push(Ident::new("serialize", ser_path.span()).into());
                        serialize_with.set(&meta.path, ser_path);
                        let mut de_path = path;
                        de_path
                            .path
                            .segments
                            .push(Ident::new("deserialize", de_path.span()).into());
                        deserialize_with.set(&meta.path, de_path);
                    }
                } else if meta.path == BOUND {
                    // #[serde(bound = "T: SomeBound")]
                    // #[serde(bound(serialize = "...", deserialize = "..."))]
                    let (ser, de) = get_where_predicates(cx, &meta)?;
                    ser_bound.set_opt(&meta.path, ser);
                    de_bound.set_opt(&meta.path, de);
                } else if meta.path == BORROW {
                    if meta.input.peek(Token![=]) {
                        // #[serde(borrow = "'a + 'b")]
                        let lifetimes = parse_lit_into_lifetimes(cx, &meta)?;
                        if let Ok(borrowable) = borrowable_lifetimes(cx, &ident, field) {
                            for lifetime in &lifetimes {
                                if !borrowable.contains(lifetime) {
                                    let msg = format!(
                                        "field `{}` does not have lifetime {}",
                                        ident, lifetime,
                                    );
                                    cx.error_spanned_by(field, msg);
                                }
                            }
                            borrowed_lifetimes.set(&meta.path, lifetimes);
                        }
                    } else {
                        // #[serde(borrow)]
                        if let Ok(borrowable) = borrowable_lifetimes(cx, &ident, field) {
                            borrowed_lifetimes.set(&meta.path, borrowable);
                        }
                    }
                } else if meta.path == GETTER {
                    // #[serde(getter = "...")]
                    if let Some(path) = parse_lit_into_expr_path(cx, GETTER, &meta)? {
                        getter.set(&meta.path, path);
                    }
                } else if meta.path == FLATTEN {
                    // #[serde(flatten)]
                    flatten.set_true(&meta.path);
                } else {
                    let path = meta.path.to_token_stream().to_string().replace(' ', "");
                    return Err(
                        meta.error(format_args!("unknown serde field attribute `{}`", path))
                    );
                }
                Ok(())
            }) {
                cx.syn_error(err);
            }
        }

        // Is skip_deserializing, initialize the field to Default::default() unless a
        // different default is specified by `#[serde(default = "...")]` on
        // ourselves or our container (e.g. the struct we are in).
        if let Default::None = *container_default {
            if skip_deserializing.0.value.is_some() {
                default.set_if_none(Default::Default);
            }
        }

        let mut borrowed_lifetimes = borrowed_lifetimes.get().unwrap_or_default();
        if !borrowed_lifetimes.is_empty() {
            // Cow<str> and Cow<[u8]> never borrow by default:
            //
            //     impl<'de, 'a, T: ?Sized> Deserialize<'de> for Cow<'a, T>
            //
            // A #[serde(borrow)] attribute enables borrowing that corresponds
            // roughly to these impls:
            //
            //     impl<'de: 'a, 'a> Deserialize<'de> for Cow<'a, str>
            //     impl<'de: 'a, 'a> Deserialize<'de> for Cow<'a, [u8]>
            if is_cow(&field.ty, is_str) {
                let mut path = syn::Path {
                    leading_colon: None,
                    segments: Punctuated::new(),
                };
                let span = Span::call_site();
                path.segments.push(Ident::new("_serde", span).into());
                path.segments.push(private.clone().into());
                path.segments.push(Ident::new("de", span).into());
                path.segments
                    .push(Ident::new("borrow_cow_str", span).into());
                let expr = syn::ExprPath {
                    attrs: Vec::new(),
                    qself: None,
                    path,
                };
                deserialize_with.set_if_none(expr);
            } else if is_cow(&field.ty, is_slice_u8) {
                let mut path = syn::Path {
                    leading_colon: None,
                    segments: Punctuated::new(),
                };
                let span = Span::call_site();
                path.segments.push(Ident::new("_serde", span).into());
                path.segments.push(private.clone().into());
                path.segments.push(Ident::new("de", span).into());
                path.segments
                    .push(Ident::new("borrow_cow_bytes", span).into());
                let expr = syn::ExprPath {
                    attrs: Vec::new(),
                    qself: None,
                    path,
                };
                deserialize_with.set_if_none(expr);
            }
        } else if is_implicitly_borrowed(&field.ty) {
            // Types &str and &[u8] are always implicitly borrowed. No need for
            // a #[serde(borrow)].
            collect_lifetimes(&field.ty, &mut borrowed_lifetimes);
        }

        Field {
            name: MultiName::from_attrs(ident, ser_name, de_name, Some(de_aliases)),
            skip_serializing: skip_serializing.get(),
            skip_deserializing: skip_deserializing.get(),
            skip_serializing_if: skip_serializing_if.get(),
            default: default.get().unwrap_or(Default::None),
            serialize_with: serialize_with.get(),
            deserialize_with: deserialize_with.get(),
            ser_bound: ser_bound.get(),
            de_bound: de_bound.get(),
            borrowed_lifetimes,
            getter: getter.get(),
            flatten: flatten.get(),
            transparent: false,
        }
    }

    pub fn name(&self) -> &MultiName {
        &self.name
    }

    pub fn aliases(&self) -> &BTreeSet<Name> {
        self.name.deserialize_aliases()
    }

    pub fn rename_by_rules(&mut self, rules: RenameAllRules) {
        if !self.name.serialize_renamed {
            self.name.serialize.value = rules.serialize.apply_to_field(&self.name.serialize.value);
        }
        if !self.name.deserialize_renamed {
            self.name.deserialize.value = rules
                .deserialize
                .apply_to_field(&self.name.deserialize.value);
        }
        self.name
            .deserialize_aliases
            .insert(self.name.deserialize.clone());
    }

    pub fn skip_serializing(&self) -> bool {
        self.skip_serializing
    }

    pub fn skip_deserializing(&self) -> bool {
        self.skip_deserializing
    }

    pub fn skip_serializing_if(&self) -> Option<&syn::ExprPath> {
        self.skip_serializing_if.as_ref()
    }

    pub fn default(&self) -> &Default {
        &self.default
    }

    pub fn serialize_with(&self) -> Option<&syn::ExprPath> {
        self.serialize_with.as_ref()
    }

    pub fn deserialize_with(&self) -> Option<&syn::ExprPath> {
        self.deserialize_with.as_ref()
    }

    pub fn ser_bound(&self) -> Option<&[syn::WherePredicate]> {
        self.ser_bound.as_ref().map(|vec| &vec[..])
    }

    pub fn de_bound(&self) -> Option<&[syn::WherePredicate]> {
        self.de_bound.as_ref().map(|vec| &vec[..])
    }

    pub fn borrowed_lifetimes(&self) -> &BTreeSet<syn::Lifetime> {
        &self.borrowed_lifetimes
    }

    pub fn getter(&self) -> Option<&syn::ExprPath> {
        self.getter.as_ref()
    }

    pub fn flatten(&self) -> bool {
        self.flatten
    }

    pub fn transparent(&self) -> bool {
        self.transparent
    }

    pub fn mark_transparent(&mut self) {
        self.transparent = true;
    }
}

type SerAndDe<T> = (Option<T>, Option<T>);

fn get_ser_and_de<'c, T, F, R>(
    cx: &'c Ctxt,
    attr_name: Symbol,
    meta: &ParseNestedMeta,
    f: F,
) -> syn::Result<(VecAttr<'c, T>, VecAttr<'c, T>)>
where
    T: Clone,
    F: Fn(&Ctxt, Symbol, Symbol, &ParseNestedMeta) -> syn::Result<R>,
    R: Into<Option<T>>,
{
    let mut ser_meta = VecAttr::none(cx, attr_name);
    let mut de_meta = VecAttr::none(cx, attr_name);

    let lookahead = meta.input.lookahead1();
    if lookahead.peek(Token![=]) {
        if let Some(both) = f(cx, attr_name, attr_name, meta)?.into() {
            ser_meta.insert(&meta.path, both.clone());
            de_meta.insert(&meta.path, both);
        }
    } else if lookahead.peek(token::Paren) {
        meta.parse_nested_meta(|meta| {
            if meta.path == SERIALIZE {
                if let Some(v) = f(cx, attr_name, SERIALIZE, &meta)?.into() {
                    ser_meta.insert(&meta.path, v);
                }
            } else if meta.path == DESERIALIZE {
                if let Some(v) = f(cx, attr_name, DESERIALIZE, &meta)?.into() {
                    de_meta.insert(&meta.path, v);
                }
            } else {
                return Err(meta.error(format_args!(
                    "malformed {0} attribute, expected `{0}(serialize = ..., deserialize = ...)`",
                    attr_name,
                )));
            }
            Ok(())
        })?;
    } else {
        return Err(lookahead.error());
    }

    Ok((ser_meta, de_meta))
}

fn get_renames(
    cx: &Ctxt,
    attr_name: Symbol,
    meta: &ParseNestedMeta,
) -> syn::Result<SerAndDe<syn::LitStr>> {
    let (ser, de) = get_ser_and_de(cx, attr_name, meta, get_lit_str2)?;
    Ok((ser.at_most_one(), de.at_most_one()))
}

fn get_multiple_renames(
    cx: &Ctxt,
    meta: &ParseNestedMeta,
) -> syn::Result<(Option<syn::LitStr>, Vec<syn::LitStr>)> {
    let (ser, de) = get_ser_and_de(cx, RENAME, meta, get_lit_str2)?;
    Ok((ser.at_most_one(), de.get()))
}

fn get_where_predicates(
    cx: &Ctxt,
    meta: &ParseNestedMeta,
) -> syn::Result<SerAndDe<Vec<syn::WherePredicate>>> {
    let (ser, de) = get_ser_and_de(cx, BOUND, meta, parse_lit_into_where)?;
    Ok((ser.at_most_one(), de.at_most_one()))
}

fn get_lit_str(
    cx: &Ctxt,
    attr_name: Symbol,
    meta: &ParseNestedMeta,
) -> syn::Result<Option<syn::LitStr>> {
    get_lit_str2(cx, attr_name, attr_name, meta)
}

fn get_lit_str2(
    cx: &Ctxt,
    attr_name: Symbol,
    meta_item_name: Symbol,
    meta: &ParseNestedMeta,
) -> syn::Result<Option<syn::LitStr>> {
    let expr: syn::Expr = meta.value()?.parse()?;
    let mut value = &expr;
    while let syn::Expr::Group(e) = value {
        value = &e.expr;
    }
    if let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Str(lit),
        ..
    }) = value
    {
        let suffix = lit.suffix();
        if !suffix.is_empty() {
            cx.error_spanned_by(
                lit,
                format!("unexpected suffix `{}` on string literal", suffix),
            );
        }
        Ok(Some(lit.clone()))
    } else {
        cx.error_spanned_by(
            expr,
            format!(
                "expected serde {} attribute to be a string: `{} = \"...\"`",
                attr_name, meta_item_name
            ),
        );
        Ok(None)
    }
}

fn parse_lit_into_path(
    cx: &Ctxt,
    attr_name: Symbol,
    meta: &ParseNestedMeta,
) -> syn::Result<Option<syn::Path>> {
    let string = match get_lit_str(cx, attr_name, meta)? {
        Some(string) => string,
        None => return Ok(None),
    };

    Ok(match string.parse() {
        Ok(path) => Some(path),
        Err(_) => {
            cx.error_spanned_by(
                &string,
                format!("failed to parse path: {:?}", string.value()),
            );
            None
        }
    })
}

fn parse_lit_into_expr_path(
    cx: &Ctxt,
    attr_name: Symbol,
    meta: &ParseNestedMeta,
) -> syn::Result<Option<syn::ExprPath>> {
    let string = match get_lit_str(cx, attr_name, meta)? {
        Some(string) => string,
        None => return Ok(None),
    };

    Ok(match string.parse() {
        Ok(expr) => Some(expr),
        Err(_) => {
            cx.error_spanned_by(
                &string,
                format!("failed to parse path: {:?}", string.value()),
            );
            None
        }
    })
}

fn parse_lit_into_where(
    cx: &Ctxt,
    attr_name: Symbol,
    meta_item_name: Symbol,
    meta: &ParseNestedMeta,
) -> syn::Result<Vec<syn::WherePredicate>> {
    let string = match get_lit_str2(cx, attr_name, meta_item_name, meta)? {
        Some(string) => string,
        None => return Ok(Vec::new()),
    };

    Ok(
        match string.parse_with(Punctuated::<syn::WherePredicate, Token![,]>::parse_terminated) {
            Ok(predicates) => Vec::from_iter(predicates),
            Err(err) => {
                cx.error_spanned_by(string, err);
                Vec::new()
            }
        },
    )
}

fn parse_lit_into_ty(
    cx: &Ctxt,
    attr_name: Symbol,
    meta: &ParseNestedMeta,
) -> syn::Result<Option<syn::Type>> {
    let string = match get_lit_str(cx, attr_name, meta)? {
        Some(string) => string,
        None => return Ok(None),
    };

    Ok(match string.parse() {
        Ok(ty) => Some(ty),
        Err(_) => {
            cx.error_spanned_by(
                &string,
                format!("failed to parse type: {} = {:?}", attr_name, string.value()),
            );
            None
        }
    })
}

// Parses a string literal like "'a + 'b + 'c" containing a nonempty list of
// lifetimes separated by `+`.
fn parse_lit_into_lifetimes(
    cx: &Ctxt,
    meta: &ParseNestedMeta,
) -> syn::Result<BTreeSet<syn::Lifetime>> {
    let string = match get_lit_str(cx, BORROW, meta)? {
        Some(string) => string,
        None => return Ok(BTreeSet::new()),
    };

    if let Ok(lifetimes) = string.parse_with(|input: ParseStream| {
        let mut set = BTreeSet::new();
        while !input.is_empty() {
            let lifetime: Lifetime = input.parse()?;
            if !set.insert(lifetime.clone()) {
                cx.error_spanned_by(
                    &string,
                    format!("duplicate borrowed lifetime `{}`", lifetime),
                );
            }
            if input.is_empty() {
                break;
            }
            input.parse::<Token![+]>()?;
        }
        Ok(set)
    }) {
        if lifetimes.is_empty() {
            cx.error_spanned_by(string, "at least one lifetime must be borrowed");
        }
        return Ok(lifetimes);
    }

    cx.error_spanned_by(
        &string,
        format!("failed to parse borrowed lifetimes: {:?}", string.value()),
    );
    Ok(BTreeSet::new())
}

fn is_implicitly_borrowed(ty: &syn::Type) -> bool {
    is_implicitly_borrowed_reference(ty) || is_option(ty, is_implicitly_borrowed_reference)
}

fn is_implicitly_borrowed_reference(ty: &syn::Type) -> bool {
    is_reference(ty, is_str) || is_reference(ty, is_slice_u8)
}

// Whether the type looks like it might be `std::borrow::Cow<T>` where elem="T".
// This can have false negatives and false positives.
//
// False negative:
//
//     use std::borrow::Cow as Pig;
//
//     #[derive(Deserialize)]
//     struct S<'a> {
//         #[serde(borrow)]
//         pig: Pig<'a, str>,
//     }
//
// False positive:
//
//     type str = [i16];
//
//     #[derive(Deserialize)]
//     struct S<'a> {
//         #[serde(borrow)]
//         cow: Cow<'a, str>,
//     }
fn is_cow(ty: &syn::Type, elem: fn(&syn::Type) -> bool) -> bool {
    let path = match ungroup(ty) {
        syn::Type::Path(ty) => &ty.path,
        _ => {
            return false;
        }
    };
    let seg = match path.segments.last() {
        Some(seg) => seg,
        None => {
            return false;
        }
    };
    let args = match &seg.arguments {
        syn::PathArguments::AngleBracketed(bracketed) => &bracketed.args,
        _ => {
            return false;
        }
    };
    seg.ident == "Cow"
        && args.len() == 2
        && match (&args[0], &args[1]) {
            (syn::GenericArgument::Lifetime(_), syn::GenericArgument::Type(arg)) => elem(arg),
            _ => false,
        }
}

fn is_option(ty: &syn::Type, elem: fn(&syn::Type) -> bool) -> bool {
    let path = match ungroup(ty) {
        syn::Type::Path(ty) => &ty.path,
        _ => {
            return false;
        }
    };
    let seg = match path.segments.last() {
        Some(seg) => seg,
        None => {
            return false;
        }
    };
    let args = match &seg.arguments {
        syn::PathArguments::AngleBracketed(bracketed) => &bracketed.args,
        _ => {
            return false;
        }
    };
    seg.ident == "Option"
        && args.len() == 1
        && match &args[0] {
            syn::GenericArgument::Type(arg) => elem(arg),
            _ => false,
        }
}

// Whether the type looks like it might be `&T` where elem="T". This can have
// false negatives and false positives.
//
// False negative:
//
//     type Yarn = str;
//
//     #[derive(Deserialize)]
//     struct S<'a> {
//         r: &'a Yarn,
//     }
//
// False positive:
//
//     type str = [i16];
//
//     #[derive(Deserialize)]
//     struct S<'a> {
//         r: &'a str,
//     }
fn is_reference(ty: &syn::Type, elem: fn(&syn::Type) -> bool) -> bool {
    match ungroup(ty) {
        syn::Type::Reference(ty) => ty.mutability.is_none() && elem(&ty.elem),
        _ => false,
    }
}

fn is_str(ty: &syn::Type) -> bool {
    is_primitive_type(ty, "str")
}

fn is_slice_u8(ty: &syn::Type) -> bool {
    match ungroup(ty) {
        syn::Type::Slice(ty) => is_primitive_type(&ty.elem, "u8"),
        _ => false,
    }
}

fn is_primitive_type(ty: &syn::Type, primitive: &str) -> bool {
    match ungroup(ty) {
        syn::Type::Path(ty) => ty.qself.is_none() && is_primitive_path(&ty.path, primitive),
        _ => false,
    }
}

fn is_primitive_path(path: &syn::Path, primitive: &str) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == 1
        && path.segments[0].ident == primitive
        && path.segments[0].arguments.is_empty()
}

// All lifetimes that this type could borrow from a Deserializer.
//
// For example a type `S<'a, 'b>` could borrow `'a` and `'b`. On the other hand
// a type `for<'a> fn(&'a str)` could not borrow `'a` from the Deserializer.
//
// This is used when there is an explicit or implicit `#[serde(borrow)]`
// attribute on the field so there must be at least one borrowable lifetime.
fn borrowable_lifetimes(
    cx: &Ctxt,
    name: &Name,
    field: &syn::Field,
) -> Result<BTreeSet<syn::Lifetime>, ()> {
    let mut lifetimes = BTreeSet::new();
    collect_lifetimes(&field.ty, &mut lifetimes);
    if lifetimes.is_empty() {
        let msg = format!("field `{}` has no lifetimes to borrow", name);
        cx.error_spanned_by(field, msg);
        Err(())
    } else {
        Ok(lifetimes)
    }
}

fn collect_lifetimes(ty: &syn::Type, out: &mut BTreeSet<syn::Lifetime>) {
    match ty {
        #![cfg_attr(all(test, exhaustive), deny(non_exhaustive_omitted_patterns))]
        syn::Type::Slice(ty) => {
            collect_lifetimes(&ty.elem, out);
        }
        syn::Type::Array(ty) => {
            collect_lifetimes(&ty.elem, out);
        }
        syn::Type::Ptr(ty) => {
            collect_lifetimes(&ty.elem, out);
        }
        syn::Type::Reference(ty) => {
            out.extend(ty.lifetime.iter().cloned());
            collect_lifetimes(&ty.elem, out);
        }
        syn::Type::Tuple(ty) => {
            for elem in &ty.elems {
                collect_lifetimes(elem, out);
            }
        }
        syn::Type::Path(ty) => {
            if let Some(qself) = &ty.qself {
                collect_lifetimes(&qself.ty, out);
            }
            for seg in &ty.path.segments {
                if let syn::PathArguments::AngleBracketed(bracketed) = &seg.arguments {
                    for arg in &bracketed.args {
                        match arg {
                            syn::GenericArgument::Lifetime(lifetime) => {
                                out.insert(lifetime.clone());
                            }
                            syn::GenericArgument::Type(ty) => {
                                collect_lifetimes(ty, out);
                            }
                            syn::GenericArgument::AssocType(binding) => {
                                collect_lifetimes(&binding.ty, out);
                            }
                            syn::GenericArgument::Const(_)
                            | syn::GenericArgument::AssocConst(_)
                            | syn::GenericArgument::Constraint(_)
                            | _ => {}
                        }
                    }
                }
            }
        }
        syn::Type::Paren(ty) => {
            collect_lifetimes(&ty.elem, out);
        }
        syn::Type::Group(ty) => {
            collect_lifetimes(&ty.elem, out);
        }
        syn::Type::Macro(ty) => {
            collect_lifetimes_from_tokens(ty.mac.tokens.clone(), out);
        }
        syn::Type::BareFn(_)
        | syn::Type::Never(_)
        | syn::Type::TraitObject(_)
        | syn::Type::ImplTrait(_)
        | syn::Type::Infer(_)
        | syn::Type::Verbatim(_) => {}

        _ => {}
    }
}

fn collect_lifetimes_from_tokens(tokens: TokenStream, out: &mut BTreeSet<syn::Lifetime>) {
    let mut iter = tokens.into_iter();
    while let Some(tt) = iter.next() {
        match &tt {
            TokenTree::Punct(op) if op.as_char() == '\'' && op.spacing() == Spacing::Joint => {
                if let Some(TokenTree::Ident(ident)) = iter.next() {
                    out.insert(syn::Lifetime {
                        apostrophe: op.span(),
                        ident,
                    });
                }
            }
            TokenTree::Group(group) => {
                let tokens = group.stream();
                collect_lifetimes_from_tokens(tokens, out);
            }
            _ => {}
        }
    }
}
