/*
 * Copyright 2019 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use proc_macro2::Ident;
use proc_macro2::Span;
use syn::parse::ParseStream;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::Attribute;
use syn::Expr;
use syn::FnArg;
use syn::GenericArgument;
use syn::Generics;
use syn::ItemFn;
use syn::Lifetime;
use syn::PathArguments;
use syn::ReturnType;
use syn::Token;
use syn::Type;

use crate::module::parse::is_attribute_docstring;
use crate::module::parse::is_mut_something;
use crate::module::parse::is_ref_something;
use crate::module::parse::parse_visibility;
use crate::module::parse::ModuleKind;
use crate::module::simple_param::SimpleParam;
use crate::module::typ::RegularParams;
use crate::module::typ::SpecialParam;
use crate::module::typ::StarArg;
use crate::module::typ::StarArgPassStyle;
use crate::module::typ::StarArgSource;
use crate::module::typ::StarArguments;
use crate::module::typ::StarAttr;
use crate::module::typ::StarFun;
use crate::module::typ::StarFunSource;
use crate::module::typ::StarStmt;
use crate::module::typ::ThisParam;
use crate::util::GenericsUtil;

#[derive(Default)]
struct FnAttrs {
    is_attribute: bool,
    as_type: Option<syn::Path>,
    starlark_ty_custom_function: Option<Expr>,
    special_builtin_function: Option<Expr>,
    speculative_exec_safe: bool,
    docstring: Option<String>,
    /// Rest attributes
    attrs: Vec<Attribute>,
}

#[derive(Default)]
struct FnParamAttrs {
    default: Option<Expr>,
    this: bool,
    pos_only: bool,
    named_only: bool,
    args: bool,
    kwargs: bool,
}

impl FnParamAttrs {
    fn is_empty(&self) -> bool {
        let FnParamAttrs {
            default,
            this,
            pos_only,
            named_only,
            args,
            kwargs,
        } = self;
        default.is_none() && !*this && !*pos_only && !*named_only && !*args && !*kwargs
    }
}

/// Parse `#[starlark(...)]` fn param attribute.
fn parse_starlark_fn_param_attr(
    tokens: &Attribute,
    param_attrs: &mut FnParamAttrs,
) -> syn::Result<()> {
    assert!(tokens.path().is_ident("starlark"));
    let parse = |parser: ParseStream| -> syn::Result<()> {
        let mut first = true;
        while !parser.is_empty() {
            if !first {
                parser.parse::<Token![,]>()?;
                // Allow trailing comma.
                if parser.is_empty() {
                    break;
                }
            }
            first = false;

            let ident = parser.parse::<Ident>()?;
            if ident == "default" {
                parser.parse::<Token![=]>()?;
                param_attrs.default = Some(parser.parse::<Expr>()?);
                continue;
            } else if ident == "this" {
                param_attrs.this = true;
                continue;
            } else if ident == "args" {
                param_attrs.args = true;
                continue;
            } else if ident == "kwargs" {
                param_attrs.kwargs = true;
                continue;
            } else if ident == "require" {
                parser.parse::<Token!(=)>()?;
                let require = parser.parse::<Ident>()?;
                if require == "pos" {
                    param_attrs.pos_only = true;
                    continue;
                } else if require == "named" {
                    param_attrs.named_only = true;
                    continue;
                }
            }

            return Err(syn::Error::new(
                ident.span(),
                "Expecting \
                `#[starlark(default = expr)]`, \
                `#[starlark(require = pos)]`, \
                `#[starlark(require = named)]`, \
                `#[starlark(this)]` attribute",
            ));
        }
        Ok(())
    };
    tokens.parse_args_with(parse)
}

/// Parse fn param attributes: parse `#[starlark(...)]` and take others as is.
fn parse_fn_param_attrs(attrs: SimpleParam) -> syn::Result<(FnParamAttrs, SimpleParam)> {
    let SimpleParam {
        attrs,
        mutability,
        ident,
        ty,
    } = attrs;
    let mut param_attrs = FnParamAttrs::default();
    let mut other_attrs = Vec::new();
    for attr in attrs {
        if attr.path().is_ident("starlark") {
            parse_starlark_fn_param_attr(&attr, &mut param_attrs)?;
        } else {
            other_attrs.push(attr);
        }
    }
    Ok((
        param_attrs,
        SimpleParam {
            attrs: other_attrs,
            mutability,
            ident,
            ty,
        },
    ))
}

/// Parse `#[starlark(...)]` fn attribute.
fn parse_starlark_fn_attr(tokens: &Attribute, attrs: &mut FnAttrs) -> syn::Result<()> {
    assert!(tokens.path().is_ident("starlark"));
    let parse = |parser: ParseStream| -> syn::Result<()> {
        let mut first = true;
        while !parser.is_empty() {
            if !first {
                parser.parse::<Token![,]>()?;
                // Allow trailing comma.
                if parser.is_empty() {
                    break;
                }
            }
            first = false;

            let ident = parser.parse::<Ident>()?;
            if ident == "as_type" {
                parser.parse::<Token![=]>()?;
                attrs.as_type = Some(parser.parse::<syn::Path>()?);
                continue;
            } else if ident == "attribute" {
                attrs.is_attribute = true;
                continue;
            } else if ident == "speculative_exec_safe" {
                attrs.speculative_exec_safe = true;
                continue;
            } else if ident == "ty_custom_function" {
                parser.parse::<Token![=]>()?;
                attrs.starlark_ty_custom_function = Some(parser.parse::<Expr>()?);
                continue;
            } else if ident == "special_builtin_function" {
                parser.parse::<Token![=]>()?;
                attrs.special_builtin_function = Some(parser.parse::<Expr>()?);
                continue;
            }
            return Err(syn::Error::new(
                ident.span(),
                "Expecting \
                    `#[starlark(as_type = ImplStarlarkValue)]`, \
                    `#[starlark(ty_custom_function = MyTy)]`, \
                    `#[starlark(attribute)]`, \
                    `#[starlark(speculative_exec_safe)]` attribute",
            ));
        }

        Ok(())
    };
    tokens.parse_args_with(parse)
}

/// Parse `#[starlark(...)]` attribute.
fn parse_fn_attrs(span: Span, xs: Vec<Attribute>) -> syn::Result<FnAttrs> {
    let mut res = FnAttrs::default();
    for x in xs {
        if x.path().is_ident("starlark") {
            parse_starlark_fn_attr(&x, &mut res)?;
        } else if let Some(ds) = is_attribute_docstring(&x)? {
            match &mut res.docstring {
                None => res.docstring = Some(ds),
                Some(docstring) => {
                    docstring.push('\n');
                    docstring.push_str(&ds);
                }
            }
            // Important the attributes remain tagged to the function, so the test annotations
            // are present, and thus the doc test works properly.
            res.attrs.push(x);
        } else {
            res.attrs.push(x);
        }
    }
    if res.is_attribute && res.as_type.is_some() {
        return Err(syn::Error::new(span, "Can't be an attribute with a .type"));
    }
    Ok(res)
}

/// Check if given type is `anyhow::Result<T>` or `starlark::Result<T>`
fn is_anyhow_or_starlark_result(t: &Type) -> bool {
    let path = match t {
        Type::Path(p) => p,
        _ => return false,
    };
    if path.qself.is_some() {
        return false;
    }
    let mut segments = path.path.segments.iter();
    match segments.next() {
        None => return false,
        Some(s) if s.ident != "anyhow" && s.ident != "starlark" => return false,
        _ => {}
    };
    let result = match segments.next() {
        None => return false,
        Some(s) if s.ident != "Result" => return false,
        Some(result) => result,
    };
    if segments.next().is_some() {
        return false;
    }
    let result_arguments = match &result.arguments {
        PathArguments::AngleBracketed(args) => args,
        _ => return false,
    };
    let mut result_arguments = result_arguments.args.iter();
    let t = match result_arguments.next() {
        None => return false,
        Some(t) => t,
    };
    matches!(t, GenericArgument::Type(_))
}

// Add a function to the `GlobalsModule` named `globals_builder`.
pub(crate) fn parse_fun(func: ItemFn, module_kind: ModuleKind) -> syn::Result<StarStmt> {
    parse_visibility(&func.vis)?;

    let sig_span = func.sig.span();

    let FnAttrs {
        is_attribute,
        as_type,
        speculative_exec_safe,
        docstring,
        starlark_ty_custom_function,
        special_builtin_function,
        attrs,
    } = parse_fn_attrs(func.span(), func.attrs)?;

    let has_v = parse_fn_generics(&func.sig.generics)?;

    let return_type = parse_fn_output(&func.sig.output, func.sig.span(), has_v)?;

    let mut this = None;
    let mut eval = None;
    let mut heap = None;

    // Seen an equivalent of `*` or `*args`. Meaning default parameters are named-only after this.
    let mut seen_star = false;
    let mut args: Option<RegularParams> = None;
    for (i, arg) in func.sig.inputs.into_iter().enumerate() {
        let span = arg.span();
        let parsed_arg = parse_arg(arg, has_v, seen_star, module_kind, i)?;
        match parsed_arg {
            StarArgOrSpecial::Heap(special) => {
                if heap.is_some() {
                    return Err(syn::Error::new(span, "Repeated `&Heap` parameter"));
                }
                heap = Some(special);
            }
            StarArgOrSpecial::Eval(special) => {
                if eval.is_some() {
                    return Err(syn::Error::new(span, "Repeated `&mut Evaluator` parameter"));
                }
                eval = Some(special);
            }
            StarArgOrSpecial::StarArg(arg) => {
                if arg.pass_style == StarArgPassStyle::Args
                    || arg.pass_style == StarArgPassStyle::NamedOnly
                {
                    seen_star = true;
                }
                let args = args.get_or_insert_with(|| RegularParams::Unpack(Vec::new()));
                match args {
                    RegularParams::Arguments(_) => {
                        return Err(syn::Error::new(
                            span,
                            "Cannot mix `&Arguments` and regular params",
                        ));
                    }
                    RegularParams::Unpack(args) => args.push(arg),
                }
            }
            StarArgOrSpecial::Arguments(arguments) => match args {
                None => args = Some(RegularParams::Arguments(arguments)),
                Some(RegularParams::Unpack(_)) => {
                    return Err(syn::Error::new(
                        span,
                        "Cannot mix `&Arguments` and regular params",
                    ));
                }
                Some(RegularParams::Arguments(_)) => {
                    return Err(syn::Error::new(span, "Duplicate `&Arguments` parameter"));
                }
            },
            StarArgOrSpecial::This(this_param) => {
                let expecting_this = module_kind == ModuleKind::Methods && i == 0;
                if !expecting_this {
                    return Err(syn::Error::new(
                        span,
                        "Receiver parameter can be only first",
                    ));
                }
                if this.is_some() {
                    return Err(syn::Error::new(span, "Repeated `this` parameter"));
                }
                this = Some(this_param);
            }
        }
    }

    if eval.is_some() && heap.is_some() {
        return Err(syn::Error::new(
            sig_span,
            "Can't have both `&mut Evaluator` and `&Heap` parameters",
        ));
    }

    if is_attribute {
        if eval.is_some() {
            return Err(syn::Error::new(
                sig_span,
                "Attributes cannot have `&mut Evaluator` parameter",
            ));
        }

        if args.is_some() {
            return Err(syn::Error::new(
                sig_span,
                "Attribute function must have `this` as the only parameter",
            ));
        }
        let Some(this) = this else {
            return Err(syn::Error::new(
                sig_span,
                "Attribute function must have `this` as the only parameter",
            ));
        };
        if starlark_ty_custom_function.is_some() {
            return Err(syn::Error::new(
                sig_span,
                "Attribute function cannot types are not implemented",
            ));
        }
        Ok(StarStmt::Attr(StarAttr {
            name: func.sig.ident,
            this,
            heap,
            attrs,
            return_type,
            speculative_exec_safe,
            body: *func.block,
            docstring,
        }))
    } else {
        let is_method = module_kind == ModuleKind::Methods;
        if is_method && this.is_none() {
            return Err(syn::Error::new(
                sig_span,
                "Methods must have `this` as the first parameter",
            ));
        }

        if is_method && starlark_ty_custom_function.is_some() {
            return Err(syn::Error::new(
                sig_span,
                "Custom types are not implemented for methods",
            ));
        }

        let mut args = args.unwrap_or_else(|| RegularParams::Unpack(Vec::new()));
        let source = match &mut args {
            RegularParams::Arguments(_) => StarFunSource::Arguments,
            RegularParams::Unpack(args) => resolve_args(args)?,
        };

        let fun = StarFun {
            name: func.sig.ident,
            as_type,
            attrs,
            this,
            args,
            heap,
            eval,
            return_type,
            starlark_ty_custom_function,
            special_builtin_function,
            speculative_exec_safe,
            body: *func.block,
            source,
            docstring,
        };
        Ok(StarStmt::Fun(fun))
    }
}

#[allow(clippy::branches_sharing_code)] // False positive
fn resolve_args(args: &mut [StarArg]) -> syn::Result<StarFunSource> {
    let use_arguments = args.iter().any(|x| x.requires_signature());
    if use_arguments {
        let mut count = 0;
        for x in args.iter_mut() {
            x.source = StarArgSource::Argument(count);
            count += 1;
        }
        Ok(StarFunSource::Signature { count })
    } else {
        let mut required = 0;
        let mut optional = 0;
        let mut kwargs = false;
        for x in args.iter_mut() {
            if x.pass_style == StarArgPassStyle::Kwargs {
                if kwargs {
                    return Err(syn::Error::new(x.span, "Duplicate `**kwargs` parameter"));
                }
                x.source = StarArgSource::Kwargs;
                kwargs = true;
            } else if optional == 0 && x.default.is_none() && !x.is_option() {
                x.source = StarArgSource::Required(required);
                required += 1;
            } else {
                x.source = StarArgSource::Optional(optional);
                optional += 1;
            }
        }
        Ok(StarFunSource::Positional {
            required,
            optional,
            kwargs,
        })
    }
}

/// Check there are no undeclared lifetiems in return type.
fn check_lifetimes_in_return_type(return_type: &ReturnType, has_v: bool) -> syn::Result<()> {
    match return_type {
        ReturnType::Default => Ok(()),
        ReturnType::Type(_, ty) => check_lifetimes_in_type(ty, has_v),
    }
}

/// Check there are no undeclared lifetiems in type.
fn check_lifetimes_in_type(ty: &Type, has_v: bool) -> syn::Result<()> {
    struct VisitImpl {
        has_v: bool,
        result: syn::Result<()>,
    }

    impl<'ast> Visit<'ast> for VisitImpl {
        #[allow(clippy::collapsible_if)]
        fn visit_lifetime(&mut self, lifetime: &'ast Lifetime) {
            if self.result.is_ok() {
                if lifetime.ident != "_" && lifetime.ident != "static" {
                    if lifetime.ident != "v" {
                        self.result = Err(syn::Error::new(
                            lifetime.span(),
                            "Only 'v lifetime is allowed",
                        ));
                    }
                    if !self.has_v {
                        self.result = Err(syn::Error::new(lifetime.span(), "Undeclared lifetime"));
                    }
                }
            }
        }
    }

    let mut visit = VisitImpl {
        has_v,
        result: Ok(()),
    };

    visit.visit_type(ty);

    visit.result
}

fn parse_fn_output(return_type: &ReturnType, span: Span, has_v: bool) -> syn::Result<Type> {
    check_lifetimes_in_return_type(return_type, has_v)?;
    match return_type {
        ReturnType::Default => Err(syn::Error::new(span, "Function must have a return type")),
        ReturnType::Type(_, x) => match is_anyhow_or_starlark_result(x) {
            true => Ok((**x).clone()),
            false => Err(syn::Error::new(
                return_type.span(),
                "Function return type must be either `anyhow::Result<...>` or `starlark::Result<...>`",
            )),
        },
    }
}

fn parse_fn_generics(generics: &Generics) -> syn::Result<bool> {
    let generics = GenericsUtil::new(generics);
    let mut seen_v = false;
    for lifetime in generics.assert_only_lifetime_params()? {
        if lifetime.lifetime.ident != "v" {
            return Err(syn::Error::new(
                lifetime.lifetime.span(),
                "Function cannot have lifetime parameters other than `v",
            ));
        }
        if !lifetime.bounds.is_empty() {
            return Err(syn::Error::new(
                lifetime.span(),
                "Function lifetime params must not have bounds",
            ));
        }
        if seen_v {
            return Err(syn::Error::new(lifetime.span(), "Duplicate `v parameters"));
        }
        seen_v = true;
    }
    Ok(seen_v)
}

#[allow(clippy::large_enum_variant)]
enum StarArgOrSpecial {
    /// Receiver.
    This(ThisParam),
    /// Function parameters.
    StarArg(StarArg),
    /// `&Arguments`.
    Arguments(StarArguments),
    /// `&mut Evaluator`.
    Eval(SpecialParam),
    /// `&Heap`.
    Heap(SpecialParam),
}

/// Function parameter is `eval: &mut Evaluator`.
fn is_eval(param: &SimpleParam, attrs: &FnParamAttrs) -> syn::Result<Option<SpecialParam>> {
    if is_mut_something(&param.ty, "Evaluator") {
        if !attrs.is_empty() {
            return Err(syn::Error::new_spanned(
                &param.ident,
                "`&mut Evaluator` parameter cannot have attributes",
            ));
        }

        Ok(Some(SpecialParam {
            param: param.clone(),
        }))
    } else {
        Ok(None)
    }
}

/// Function parameter is `heap: &Heap`.
fn is_heap(param: &SimpleParam, attrs: &FnParamAttrs) -> syn::Result<Option<SpecialParam>> {
    if is_ref_something(&param.ty, "Heap") {
        if !attrs.is_empty() {
            return Err(syn::Error::new_spanned(
                &param.ident,
                "`&Heap` parameter cannot have attributes",
            ));
        }
        Ok(Some(SpecialParam {
            param: param.clone(),
        }))
    } else {
        Ok(None)
    }
}

fn parse_this_param(param: &SimpleParam, attrs: &FnParamAttrs) -> syn::Result<ThisParam> {
    let FnParamAttrs {
        default,
        this,
        pos_only,
        named_only,
        args,
        kwargs,
    } = attrs;

    if !this && &param.ident != "this" {
        return Err(syn::Error::new_spanned(
            param,
            "Receiver parameter must be named `this` \
                            or have `#[starlark(this)]` annotation",
        ));
    }

    if default.is_some() || *pos_only || *named_only || *args || *kwargs {
        return Err(syn::Error::new_spanned(
            param,
            "Attributes are not compatible with receiver parameter",
        ));
    }

    Ok(ThisParam {
        param: param.clone(),
    })
}

fn is_arguments(param: &SimpleParam, attrs: &FnParamAttrs) -> syn::Result<Option<StarArguments>> {
    if is_ref_something(&param.ty, "Arguments") {
        let FnParamAttrs {
            default,
            this,
            pos_only,
            named_only,
            args,
            kwargs,
        } = attrs;
        if default.is_some() || *this || *pos_only || *named_only || *args || *kwargs {
            return Err(syn::Error::new_spanned(
                param,
                "Attributes are not compatible with `&Arguments` parameter",
            ));
        }
        Ok(Some(StarArguments {
            param: param.clone(),
        }))
    } else {
        Ok(None)
    }
}

#[allow(clippy::collapsible_else_if)]
fn parse_arg(
    x: FnArg,
    has_v: bool,
    seen_star: bool,
    module_kind: ModuleKind,
    param_index: usize,
) -> syn::Result<StarArgOrSpecial> {
    let this = module_kind == ModuleKind::Methods && param_index == 0;

    let span = x.span();

    let param = SimpleParam::from_fn_arg(x)?;

    check_lifetimes_in_type(&param.ty, has_v)?;
    let (param_attrs, param) = parse_fn_param_attrs(param)?;

    if let Some(heap) = is_heap(&param, &param_attrs)? {
        if this {
            return Err(syn::Error::new(
                span,
                "Receiver parameter cannot be `&Heap`",
            ));
        }
        return Ok(StarArgOrSpecial::Heap(heap));
    } else if let Some(eval) = is_eval(&param, &param_attrs)? {
        if this {
            return Err(syn::Error::new(
                span,
                "Receiver parameter cannot be `&mut Evaluator`",
            ));
        }
        return Ok(StarArgOrSpecial::Eval(eval));
    }

    if this {
        return Ok(StarArgOrSpecial::This(parse_this_param(
            &param,
            &param_attrs,
        )?));
    } else {
        if param_attrs.this {
            return Err(syn::Error::new(
                span,
                "Receiver parameter can be only first",
            ));
        }
    }

    if let Some(arguments) = is_arguments(&param, &param_attrs)? {
        return Ok(StarArgOrSpecial::Arguments(arguments));
    }

    let pass_style = match (
        param_attrs.args,
        param_attrs.kwargs,
        seen_star,
        param_attrs.pos_only,
        param_attrs.named_only,
    ) {
        (true, _, _, _, _) => StarArgPassStyle::Args,
        (_, true, _, _, _) => StarArgPassStyle::Kwargs,
        (_, _, true, true, _) => {
            return Err(syn::Error::new(
                span,
                "Positional-only arguments cannot follow *args",
            ));
        }
        (_, _, _, true, true) => {
            return Err(syn::Error::new(
                span,
                "Function parameter cannot be both positional-only and named-only",
            ));
        }
        (false, false, true, false, _) => StarArgPassStyle::NamedOnly,
        // TODO(nga): currently, without `#[starlark(require = named)]`
        //   and without `#[starlark(require = pos)]`, parameter is positional-or-named.
        //   We want to change that: either make it positional by default,
        //   or require explicit `#[starlark(pos, named)]`.
        //   Discussion there:
        //   https://fb.workplace.com/groups/1267349253953900/posts/1299495914072567
        (false, false, false, false, false) => StarArgPassStyle::PosOrNamed,
        (false, false, false, true, false) => StarArgPassStyle::PosOnly,
        (false, false, false, false, true) => StarArgPassStyle::NamedOnly,
    };
    Ok(StarArgOrSpecial::StarArg(StarArg {
        span,
        param,
        pass_style,
        default: param_attrs.default,
        source: StarArgSource::Unknown,
    }))
}
