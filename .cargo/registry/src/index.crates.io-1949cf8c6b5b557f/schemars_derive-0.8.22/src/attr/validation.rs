use super::{expr_as_lit_str, get_meta_items, parse_lit_into_path, parse_lit_str};
use proc_macro2::TokenStream;
use quote::ToTokens;
use serde_derive_internals::Ctxt;
use syn::{
    parse::Parser, punctuated::Punctuated, Expr, ExprPath, Lit, Meta, MetaList, MetaNameValue, Path,
};

pub(crate) static VALIDATION_KEYWORDS: &[&str] = &[
    "range", "regex", "contains", "email", "phone", "url", "length", "required",
];

#[derive(Debug, Clone, Copy, PartialEq)]
enum Format {
    Email,
    Uri,
    Phone,
}

impl Format {
    fn attr_str(self) -> &'static str {
        match self {
            Format::Email => "email",
            Format::Uri => "url",
            Format::Phone => "phone",
        }
    }

    fn schema_str(self) -> &'static str {
        match self {
            Format::Email => "email",
            Format::Uri => "uri",
            Format::Phone => "phone",
        }
    }
}

#[derive(Debug, Default)]
pub struct ValidationAttrs {
    length_min: Option<Expr>,
    length_max: Option<Expr>,
    length_equal: Option<Expr>,
    range_min: Option<Expr>,
    range_max: Option<Expr>,
    regex: Option<Expr>,
    contains: Option<String>,
    required: bool,
    format: Option<Format>,
    inner: Option<Box<ValidationAttrs>>,
}

impl ValidationAttrs {
    pub fn new(attrs: &[syn::Attribute], errors: &Ctxt) -> Self {
        let schemars_items = get_meta_items(attrs, "schemars", errors, false);
        let validate_items = get_meta_items(attrs, "validate", errors, true);

        ValidationAttrs::default()
            .populate(schemars_items, "schemars", false, errors)
            .populate(validate_items, "validate", true, errors)
    }

    pub fn required(&self) -> bool {
        self.required
    }

    fn populate(
        mut self,
        meta_items: Vec<Meta>,
        attr_type: &'static str,
        ignore_errors: bool,
        errors: &Ctxt,
    ) -> Self {
        let duplicate_error = |path: &Path| {
            if !ignore_errors {
                let msg = format!(
                    "duplicate schemars attribute `{}`",
                    path.get_ident().unwrap()
                );
                errors.error_spanned_by(path, msg)
            }
        };
        let mutual_exclusive_error = |path: &Path, other: &str| {
            if !ignore_errors {
                let msg = format!(
                    "schemars attribute cannot contain both `{}` and `{}`",
                    path.get_ident().unwrap(),
                    other,
                );
                errors.error_spanned_by(path, msg)
            }
        };
        let duplicate_format_error = |existing: Format, new: Format, path: &syn::Path| {
            if !ignore_errors {
                let msg = if existing == new {
                    format!("duplicate schemars attribute `{}`", existing.attr_str())
                } else {
                    format!(
                        "schemars attribute cannot contain both `{}` and `{}`",
                        existing.attr_str(),
                        new.attr_str(),
                    )
                };
                errors.error_spanned_by(path, msg)
            }
        };
        let parse_nested_meta = |meta_list: MetaList| {
            let parser = Punctuated::<syn::Meta, Token![,]>::parse_terminated;
            match parser.parse2(meta_list.tokens) {
                Ok(p) => p,
                Err(e) => {
                    if !ignore_errors {
                        errors.syn_error(e);
                    }
                    Default::default()
                }
            }
        };

        for meta_item in meta_items {
            match meta_item {
                Meta::List(meta_list) if meta_list.path.is_ident("length") => {
                    for nested in parse_nested_meta(meta_list) {
                        match nested {
                            Meta::NameValue(nv) if nv.path.is_ident("min") => {
                                if self.length_min.is_some() {
                                    duplicate_error(&nv.path)
                                } else if self.length_equal.is_some() {
                                    mutual_exclusive_error(&nv.path, "equal")
                                } else {
                                    self.length_min = str_or_num_to_expr(errors, "min", nv.value);
                                }
                            }
                            Meta::NameValue(nv) if nv.path.is_ident("max") => {
                                if self.length_max.is_some() {
                                    duplicate_error(&nv.path)
                                } else if self.length_equal.is_some() {
                                    mutual_exclusive_error(&nv.path, "equal")
                                } else {
                                    self.length_max = str_or_num_to_expr(errors, "max", nv.value);
                                }
                            }
                            Meta::NameValue(nv) if nv.path.is_ident("equal") => {
                                if self.length_equal.is_some() {
                                    duplicate_error(&nv.path)
                                } else if self.length_min.is_some() {
                                    mutual_exclusive_error(&nv.path, "min")
                                } else if self.length_max.is_some() {
                                    mutual_exclusive_error(&nv.path, "max")
                                } else {
                                    self.length_equal =
                                        str_or_num_to_expr(errors, "equal", nv.value);
                                }
                            }
                            meta => {
                                if !ignore_errors {
                                    errors.error_spanned_by(
                                        meta,
                                        "unknown item in schemars length attribute".to_string(),
                                    );
                                }
                            }
                        }
                    }
                }

                Meta::List(meta_list) if meta_list.path.is_ident("range") => {
                    for nested in parse_nested_meta(meta_list) {
                        match nested {
                            Meta::NameValue(nv) if nv.path.is_ident("min") => {
                                if self.range_min.is_some() {
                                    duplicate_error(&nv.path)
                                } else {
                                    self.range_min = str_or_num_to_expr(errors, "min", nv.value);
                                }
                            }
                            Meta::NameValue(nv) if nv.path.is_ident("max") => {
                                if self.range_max.is_some() {
                                    duplicate_error(&nv.path)
                                } else {
                                    self.range_max = str_or_num_to_expr(errors, "max", nv.value);
                                }
                            }
                            meta => {
                                if !ignore_errors {
                                    errors.error_spanned_by(
                                        meta,
                                        "unknown item in schemars range attribute".to_string(),
                                    );
                                }
                            }
                        }
                    }
                }

                Meta::Path(m) if m.is_ident("required") || m.is_ident("required_nested") => {
                    self.required = true;
                }

                Meta::Path(p) if p.is_ident(Format::Email.attr_str()) => match self.format {
                    Some(f) => duplicate_format_error(f, Format::Email, &p),
                    None => self.format = Some(Format::Email),
                },
                Meta::Path(p) if p.is_ident(Format::Uri.attr_str()) => match self.format {
                    Some(f) => duplicate_format_error(f, Format::Uri, &p),
                    None => self.format = Some(Format::Uri),
                },
                Meta::Path(p) if p.is_ident(Format::Phone.attr_str()) => match self.format {
                    Some(f) => duplicate_format_error(f, Format::Phone, &p),
                    None => self.format = Some(Format::Phone),
                },

                Meta::NameValue(nv) if nv.path.is_ident("regex") => {
                    match (&self.regex, &self.contains) {
                        (Some(_), _) => duplicate_error(&nv.path),
                        (None, Some(_)) => mutual_exclusive_error(&nv.path, "contains"),
                        (None, None) => {
                            self.regex =
                                parse_lit_into_expr_path(errors, attr_type, "regex", &nv.value).ok()
                        }
                    }
                }

                Meta::List(meta_list) if meta_list.path.is_ident("regex") => {
                    match (&self.regex, &self.contains) {
                        (Some(_), _) => duplicate_error(&meta_list.path),
                        (None, Some(_)) => mutual_exclusive_error(&meta_list.path, "contains"),
                        (None, None) => {
                            for x in parse_nested_meta(meta_list) {
                                match x {
                                    Meta::NameValue(MetaNameValue { path, value, .. })
                                        if path.is_ident("path") =>
                                    {
                                        self.regex = parse_lit_into_expr_path(
                                            errors, attr_type, "path", &value,
                                        )
                                        .ok()
                                    }
                                    Meta::NameValue(MetaNameValue { path, value, .. })
                                        if path.is_ident("pattern") =>
                                    {
                                        self.regex =
                                            expr_as_lit_str(errors, attr_type, "pattern", &value)
                                                .ok()
                                                .map(|litstr| {
                                                    Expr::Lit(syn::ExprLit {
                                                        attrs: Vec::new(),
                                                        lit: Lit::Str(litstr.clone()),
                                                    })
                                                })
                                    }
                                    meta => {
                                        if !ignore_errors {
                                            errors.error_spanned_by(
                                                meta,
                                                "unknown item in schemars regex attribute"
                                                    .to_string(),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                Meta::NameValue(MetaNameValue { path, value, .. }) if path.is_ident("contains") => {
                    match (&self.contains, &self.regex) {
                        (Some(_), _) => duplicate_error(&path),
                        (None, Some(_)) => mutual_exclusive_error(&path, "regex"),
                        (None, None) => {
                            self.contains = expr_as_lit_str(errors, attr_type, "contains", &value)
                                .map(|litstr| litstr.value())
                                .ok()
                        }
                    }
                }

                Meta::List(meta_list) if meta_list.path.is_ident("contains") => {
                    match (&self.contains, &self.regex) {
                        (Some(_), _) => duplicate_error(&meta_list.path),
                        (None, Some(_)) => mutual_exclusive_error(&meta_list.path, "regex"),
                        (None, None) => {
                            for x in parse_nested_meta(meta_list) {
                                match x {
                                    Meta::NameValue(MetaNameValue { path, value, .. })
                                        if path.is_ident("pattern") =>
                                    {
                                        self.contains =
                                            expr_as_lit_str(errors, attr_type, "contains", &value)
                                                .ok()
                                                .map(|litstr| litstr.value())
                                    }
                                    meta => {
                                        if !ignore_errors {
                                            errors.error_spanned_by(
                                                meta,
                                                "unknown item in schemars contains attribute"
                                                    .to_string(),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                Meta::List(meta_list) if meta_list.path.is_ident("inner") => match self.inner {
                    Some(_) => duplicate_error(&meta_list.path),
                    None => {
                        let inner_attrs = ValidationAttrs::default().populate(
                            parse_nested_meta(meta_list).into_iter().collect(),
                            attr_type,
                            ignore_errors,
                            errors,
                        );
                        self.inner = Some(Box::new(inner_attrs));
                    }
                },

                _ => {}
            }
        }
        self
    }

    pub fn apply_to_schema(&self, schema_expr: &mut TokenStream) {
        if let Some(apply_expr) = self.apply_to_schema_expr() {
            *schema_expr = quote! {
                {
                    let mut schema = #schema_expr;
                    #apply_expr
                    schema
                }
            }
        }
    }

    fn apply_to_schema_expr(&self) -> Option<TokenStream> {
        let mut array_validation = Vec::new();
        let mut number_validation = Vec::new();
        let mut object_validation = Vec::new();
        let mut string_validation = Vec::new();

        if let Some(length_min) = self.length_min.as_ref().or(self.length_equal.as_ref()) {
            string_validation.push(quote! {
                validation.min_length = Some(#length_min as u32);
            });
            array_validation.push(quote! {
                validation.min_items = Some(#length_min as u32);
            });
        }

        if let Some(length_max) = self.length_max.as_ref().or(self.length_equal.as_ref()) {
            string_validation.push(quote! {
                validation.max_length = Some(#length_max as u32);
            });
            array_validation.push(quote! {
                validation.max_items = Some(#length_max as u32);
            });
        }

        if let Some(range_min) = &self.range_min {
            number_validation.push(quote! {
                validation.minimum = Some(#range_min as f64);
            });
        }

        if let Some(range_max) = &self.range_max {
            number_validation.push(quote! {
                validation.maximum = Some(#range_max as f64);
            });
        }

        if let Some(regex) = &self.regex {
            string_validation.push(quote! {
                validation.pattern = Some(#regex.to_string());
            });
        }

        if let Some(contains) = &self.contains {
            object_validation.push(quote! {
                validation.required.insert(#contains.to_string());
            });

            if self.regex.is_none() {
                let pattern = crate::regex_syntax::escape(contains);
                string_validation.push(quote! {
                    validation.pattern = Some(#pattern.to_string());
                });
            }
        }

        let format = self.format.as_ref().map(|f| {
            let f = f.schema_str();
            quote! {
                schema_object.format = Some(#f.to_string());
            }
        });

        let inner_validation = self
            .inner
            .as_deref()
            .and_then(|inner| inner.apply_to_schema_expr())
            .map(|apply_expr| {
                quote! {
                    if schema_object.has_type(schemars::schema::InstanceType::Array) {
                        if let Some(schemars::schema::SingleOrVec::Single(inner_schema)) = &mut schema_object.array().items {
                            let mut schema = &mut **inner_schema;
                            #apply_expr
                        }
                    }
                }
            });

        let array_validation = wrap_array_validation(array_validation);
        let number_validation = wrap_number_validation(number_validation);
        let object_validation = wrap_object_validation(object_validation);
        let string_validation = wrap_string_validation(string_validation);

        if array_validation.is_some()
            || number_validation.is_some()
            || object_validation.is_some()
            || string_validation.is_some()
            || format.is_some()
            || inner_validation.is_some()
        {
            Some(quote! {
                if let schemars::schema::Schema::Object(schema_object) = &mut schema {
                    #array_validation
                    #number_validation
                    #object_validation
                    #string_validation
                    #format
                    #inner_validation
                }
            })
        } else {
            None
        }
    }
}

fn parse_lit_into_expr_path(
    cx: &Ctxt,
    attr_type: &'static str,
    meta_item_name: &'static str,
    lit: &Expr,
) -> Result<Expr, ()> {
    parse_lit_into_path(cx, attr_type, meta_item_name, lit).map(|path| {
        Expr::Path(ExprPath {
            attrs: Vec::new(),
            qself: None,
            path,
        })
    })
}

fn wrap_array_validation(v: Vec<TokenStream>) -> Option<TokenStream> {
    if v.is_empty() {
        None
    } else {
        Some(quote! {
            if schema_object.has_type(schemars::schema::InstanceType::Array) {
                let validation = schema_object.array();
                #(#v)*
            }
        })
    }
}

fn wrap_number_validation(v: Vec<TokenStream>) -> Option<TokenStream> {
    if v.is_empty() {
        None
    } else {
        Some(quote! {
            if schema_object.has_type(schemars::schema::InstanceType::Integer)
                || schema_object.has_type(schemars::schema::InstanceType::Number) {
                let validation = schema_object.number();
                #(#v)*
            }
        })
    }
}

fn wrap_object_validation(v: Vec<TokenStream>) -> Option<TokenStream> {
    if v.is_empty() {
        None
    } else {
        Some(quote! {
            if schema_object.has_type(schemars::schema::InstanceType::Object) {
                let validation = schema_object.object();
                #(#v)*
            }
        })
    }
}

fn wrap_string_validation(v: Vec<TokenStream>) -> Option<TokenStream> {
    if v.is_empty() {
        None
    } else {
        Some(quote! {
            if schema_object.has_type(schemars::schema::InstanceType::String) {
                let validation = schema_object.string();
                #(#v)*
            }
        })
    }
}

fn str_or_num_to_expr(cx: &Ctxt, meta_item_name: &str, expr: Expr) -> Option<Expr> {
    // this odd double-parsing is to make `-10` parsed as an Lit instead of an Expr::Unary
    let lit: Lit = match syn::parse2(expr.to_token_stream()) {
        Ok(l) => l,
        Err(err) => {
            cx.syn_error(err);
            return None;
        }
    };

    match lit {
        Lit::Str(s) => parse_lit_str::<ExprPath>(&s).ok().map(Expr::Path),
        Lit::Int(_) | Lit::Float(_) => Some(expr),
        _ => {
            cx.error_spanned_by(
                &expr,
                format!(
                    "expected `{}` to be a string or number literal, not {:?}",
                    meta_item_name, &expr
                ),
            );
            None
        }
    }
}
