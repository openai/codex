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

use quote::quote_spanned;
use syn::spanned::Spanned;

use crate::util::GenericsUtil;
use crate::v_lifetime::find_v_lifetime;
use crate::vtable::vtable_has_field_name;

pub(crate) fn derive_starlark_value(
    attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let attr = syn::parse_macro_input!(attr as StarlarkValueAttrs);
    let input = syn::parse_macro_input!(input as syn::ItemImpl);
    match derive_starlark_value_impl(attr, input) {
        Ok(gen) => gen.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

struct StarlarkValueAttrs {
    typ: syn::Expr,
    /// Implement `UnpackValue` for `&T`.
    /// Note we are implementing `UnpackValue` for `&T` instead of `T`,
    /// therefore we use proc macro on impl trait instead of `#[derive]` on struct.
    unpack_value: bool,
    /// Implement `StarlarkTypeRepr` for `&T`.
    starlark_type_repr: bool,
}

impl syn::parse::Parse for StarlarkValueAttrs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        input.parse::<syn::Token![type]>()?;
        input.parse::<syn::Token![=]>()?;
        let typ = input.parse::<syn::Expr>()?;
        let mut attrs = StarlarkValueAttrs {
            typ,
            unpack_value: false,
            starlark_type_repr: false,
        };

        loop {
            if input.is_empty() {
                break;
            }
            input.parse::<syn::Token![,]>()?;
            if input.is_empty() {
                // Allow trailing comma.
                break;
            }
            let name = input.parse::<syn::Ident>()?;
            if name == "UnpackValue" {
                attrs.unpack_value = true;
            } else if name == "StarlarkTypeRepr" {
                attrs.starlark_type_repr = true;
            } else {
                return Err(syn::Error::new_spanned(
                    name,
                    "unknown attribute, allowed attribute is `UnpackValue`, `StarlarkTypeRepr`",
                ));
            }
        }

        Ok(attrs)
    }
}

struct ImplStarlarkValue {
    input: syn::ItemImpl,
    lifetime_param: syn::Lifetime,
    attrs: StarlarkValueAttrs,
}

fn is_impl_starlark_value(
    input: syn::ItemImpl,
    attrs: StarlarkValueAttrs,
) -> syn::Result<ImplStarlarkValue> {
    let err = "expected `impl StarlarkValue for ...`";
    let Some((_, path, _)) = &input.trait_ else {
        return Err(syn::Error::new_spanned(input, err));
    };
    let Some(last) = path.segments.last() else {
        return Err(syn::Error::new_spanned(path, err));
    };
    if last.ident != "StarlarkValue" {
        return Err(syn::Error::new_spanned(&last.ident, err));
    }
    let lifetime_param = match find_v_lifetime(&input.generics)? {
        Some(lt) => lt.clone(),
        None => {
            return Err(syn::Error::new_spanned(
                input,
                "expected a lifetime parameter",
            ));
        }
    };
    Ok(ImplStarlarkValue {
        input,
        lifetime_param,
        attrs,
    })
}

impl ImplStarlarkValue {
    fn span(&self) -> proc_macro2::Span {
        self.input.span()
    }

    /// Impl `UnpackValue for &T`.
    fn impl_unpack_value(&self) -> syn::Result<proc_macro2::TokenStream> {
        if !self.attrs.unpack_value && !self.attrs.starlark_type_repr {
            return Ok(proc_macro2::TokenStream::new());
        }

        if !self.attrs.unpack_value || !self.attrs.starlark_type_repr {
            return Err(syn::Error::new_spanned(
                &self.input,
                "`UnpackValue` and `StarlarkTypeRepr` can only be specified together",
            ));
        }

        GenericsUtil::new(&self.input.generics).assert_only_lifetime_params()?;

        let lt = &self.lifetime_param;
        let params = &self.input.generics.params;
        // TODO(nga): where clause is incorrect:
        //   if there's something `Self: Xxx` constraint, it should be `*Self: Xxx`.
        let where_clause = &self.input.generics.where_clause;
        let self_ty = &self.input.self_ty;
        Ok(quote_spanned! {
            self.span() =>

            impl<#params> starlark::values::type_repr::StarlarkTypeRepr for &#lt #self_ty
            #where_clause
            {
                type Canonical = #self_ty;

                fn starlark_type_repr() -> starlark::typing::Ty {
                    <#self_ty as starlark::values::type_repr::StarlarkTypeRepr>::starlark_type_repr()
                }
            }

            impl<#params> starlark::values::UnpackValue<#lt> for &#lt #self_ty
            #where_clause
            {
                type Error = std::convert::Infallible;

                fn unpack_value_impl(value: starlark::values::Value<#lt>) -> Result<Option<&#lt #self_ty>, Self::Error> {
                    std::result::Result::Ok(starlark::values::ValueLike::downcast_ref(value))
                }
            }
        })
    }

    fn please_use_starlark_type_macro(&self) -> syn::Result<syn::ImplItem> {
        syn::parse2(quote_spanned! { self.span() =>
            fn please_use_starlark_type_macro() {}
        })
    }

    fn const_type(&self) -> syn::Result<syn::ImplItem> {
        let typ = &self.attrs.typ;
        syn::parse2(quote_spanned! { self.span() =>
            const TYPE: &'static str = #typ;
        })
    }

    fn get_type_value_static(&self) -> syn::Result<syn::ImplItem> {
        let typ = &self.attrs.typ;
        syn::parse2(quote_spanned! { self.span() =>
            #[inline]
            fn get_type_value_static() -> starlark::values::FrozenStringValue {
                starlark::const_frozen_string!(#typ)
            }
        })
    }

    /// Generate `HAS_foo` flags for each implemented function, including generated by this macro.
    fn has_fn_flags(&self, fixed_trait: &syn::ItemImpl) -> syn::Result<Vec<syn::ImplItem>> {
        let mut flags = Vec::new();
        for item in &fixed_trait.items {
            if let syn::ImplItem::Fn(f) = item {
                let has_name = vtable_has_field_name(&f.sig.ident);
                flags.push(syn::parse_quote_spanned! { f.span() =>
                    const #has_name: bool = true;
                });
            }
        }
        Ok(flags)
    }

    /// Find a function with given name.
    fn find_fn(&self, name: &str) -> Option<syn::ImplItemFn> {
        self.input.items.iter().find_map(|item| {
            if let syn::ImplItem::Fn(fn_) = item {
                if fn_.sig.ident == name {
                    Some(fn_.clone())
                } else {
                    None
                }
            } else {
                None
            }
        })
    }

    /// Find a type with given name.
    fn find_ty(&self, name: &str) -> Option<&syn::ImplItemType> {
        self.input.items.iter().find_map(|item| {
            if let syn::ImplItem::Type(ty) = item {
                if ty.ident == name { Some(ty) } else { None }
            } else {
                None
            }
        })
    }

    /// There's an function with given name.
    fn has_fn(&self, name: &str) -> bool {
        self.find_fn(name).is_some()
    }

    fn bin_op_arm(&self, bin_op: &str, impl_name: &str) -> Option<syn::Arm> {
        let bin_op = syn::Ident::new(bin_op, self.span());
        if self.has_fn(impl_name) || (impl_name == "bit_or" && self.has_fn("eval_type")) {
            Some(syn::parse_quote_spanned! {
                self.span()=>
                starlark::typing::TypingBinOp::#bin_op => {
                    Some(starlark::typing::Ty::any())
                }
            })
        } else {
            None
        }
    }

    fn bin_op_ty_impl(&self) -> syn::Result<Option<syn::ImplItem>> {
        let arms = [
            self.bin_op_arm("Add", "add"),
            self.bin_op_arm("Sub", "sub"),
            self.bin_op_arm("Mul", "mul"),
            self.bin_op_arm("Div", "div"),
            self.bin_op_arm("FloorDiv", "floor_div"),
            self.bin_op_arm("Percent", "percent"),
            self.bin_op_arm("In", "is_in"),
            self.bin_op_arm("BitOr", "bit_or"),
            self.bin_op_arm("BitAnd", "bit_and"),
            self.bin_op_arm("BitXor", "bit_xor"),
            self.bin_op_arm("Less", "compare"),
            self.bin_op_arm("LeftShift", "left_shift"),
            self.bin_op_arm("RightShift", "right_shift"),
        ];
        if arms.iter().all(Option::is_none) {
            // Use default implementation.
            return Ok(None);
        }
        let default_arm: Option<syn::Arm> = if arms.iter().all(Option::is_some) {
            None
        } else {
            Some(syn::parse_quote_spanned! {self.span()=>
                _ => { None }
            })
        };
        let arms = arms.into_iter().flatten();
        Ok(Some(syn::parse_quote_spanned! {self.span()=>
            fn bin_op_ty(op: starlark::typing::TypingBinOp, _rhs: &starlark::typing::TyBasic) -> Option<starlark::typing::Ty> {
                match op {
                    #( #arms )*
                    #default_arm
                }
            }
        }))
    }

    fn rbin_op_ty_impl(&self) -> syn::Result<Option<syn::ImplItem>> {
        let arms = [
            self.bin_op_arm("Add", "radd"),
            self.bin_op_arm("Mul", "rmul"),
        ];
        if arms.iter().all(Option::is_none) {
            // Use default implementation.
            return Ok(None);
        }
        Ok(Some(syn::parse_quote_spanned! {self.span()=>
            fn rbin_op_ty(_lhs: &starlark::typing::TyBasic, op: starlark::typing::TypingBinOp) -> Option<starlark::typing::Ty> {
                match op {
                    #( #arms )*
                    _ => {
                        None
                    }
                }
            }
        }))
    }

    /// Generate `fn bin_op_ty()`.
    fn bin_op_ty(&self) -> syn::Result<Option<syn::ImplItem>> {
        if self.has_fn("bin_op_ty") {
            Ok(None)
        } else {
            self.bin_op_ty_impl()
        }
    }

    /// Generate `fn rbin_op_ty()`.
    fn rbin_op_ty(&self) -> syn::Result<Option<syn::ImplItem>> {
        if self.has_fn("rbin_op_ty") {
            Ok(None)
        } else {
            self.rbin_op_ty_impl()
        }
    }

    /// `fn attr_ty()`.
    fn attr_ty(&self) -> syn::Result<Option<syn::ImplItem>> {
        if self.has_fn("attr_ty") {
            // User has custom `attr_ty` implementation.
            Ok(None)
        } else if !self.has_fn("get_attr") {
            Ok(Some(syn::parse2(quote_spanned! { self.span() =>
                fn attr_ty(_attr: &str) -> ::std::option::Option<starlark::typing::Ty> {
                    ::std::option::Option::None
                }
            })?))
        } else {
            // Use default implementation which returns `Any`.
            Ok(None)
        }
    }

    /// `fn bit_or()` for values which are types.
    fn bit_or(&self) -> syn::Result<Option<syn::ImplItem>> {
        if !self.has_fn("eval_type") {
            return Ok(None);
        }
        if let Some(bit_or) = self.find_fn("bit_or") {
            return Err(syn::Error::new_spanned(
                bit_or.sig.ident,
                "types with `eval_type` implemented can only have generated `bit_or`",
            ));
        }
        Ok(Some(syn::parse2(quote_spanned! { self.span() =>
            fn bit_or(&self, other: starlark::values::Value<'v>, heap: &'v starlark::values::Heap) -> starlark::Result<starlark::values::Value<'v>> {
                starlark::values::typing::macro_refs::starlark_value_bit_or_for_type(self, other, heap)
            }
        })?))
    }

    /// `ValueLike<'v>`?
    fn path_is_value_like(&self, path: &syn::Path) -> syn::Result<bool> {
        let Some(last) = path.segments.last() else {
            return Ok(false);
        };
        if last.ident != "ValueLike" {
            return Ok(false);
        }
        let syn::PathArguments::AngleBracketed(args) = &last.arguments else {
            return Ok(false);
        };
        if args.args.len() != 1 {
            return Ok(false);
        }
        let Some(arg) = args.args.first() else {
            // Unreachable.
            return Ok(false);
        };
        let syn::GenericArgument::Lifetime(lt) = arg else {
            return Ok(false);
        };
        Ok(lt == &self.lifetime_param)
    }

    /// `V: ValueLike<'v>`?
    fn type_param_is_value_like(&self, type_param: &syn::TypeParam) -> syn::Result<bool> {
        for bound in &type_param.bounds {
            if let syn::TypeParamBound::Trait(t) = bound {
                if self.path_is_value_like(&t.path)? {
                    return Ok(true);
                }
            }
        }
        // TODO(nga): also check `where` clause.
        Ok(false)
    }

    /// Make `Canonical` type.
    /// Replace type arguments with `Value<'v>`.
    fn do_make_canonical_type(&self) -> syn::Result<syn::Type> {
        let syn::Type::Path(type_path) = &*self.input.self_ty else {
            return Err(syn::Error::new_spanned(
                &self.input.self_ty,
                "self type is not path",
            ));
        };
        let syn::TypePath { qself, path } = type_path;
        if qself.is_some() {
            return Err(syn::Error::new_spanned(
                type_path,
                "qualified self not supported",
            ));
        }

        // If type name is `FrozenXxx`, it is likely that it should have `Canonical`
        // pointing to `Xxx`. Make it an error and force user to specify it explicitly to be safe.
        if let Some(last) = path.segments.last() {
            if last.ident.to_string().starts_with("Frozen") {
                return Err(syn::Error::new_spanned(
                    last,
                    "Type name starts with `Frozen`, please specify `Canonical` explicitly",
                ));
            }
        }

        let mut path = path.clone();

        struct PatchTypesVisitor<'a> {
            lifetime: &'a syn::Lifetime,
        }

        impl<'a> syn::visit_mut::VisitMut for PatchTypesVisitor<'a> {
            fn visit_type_mut(&mut self, i: &mut syn::Type) {
                let lifetime = self.lifetime;
                *i = syn::parse_quote_spanned! { i.span() =>
                    starlark::values::Value< #lifetime >
                };
            }
        }

        syn::visit_mut::VisitMut::visit_path_mut(
            &mut PatchTypesVisitor {
                lifetime: &self.lifetime_param,
            },
            &mut path,
        );

        Ok(syn::parse_quote_spanned! { self.span() => #path })
    }

    fn make_canonical_type(&self) -> syn::Result<syn::Type> {
        // Impl has `V: ValueLike<'v>` constraint.
        let mut value_like_param = false;
        for type_param in &self.input.generics.params {
            match type_param {
                syn::GenericParam::Lifetime(_) => {}
                syn::GenericParam::Const(_) => {
                    return Err(syn::Error::new_spanned(
                        type_param,
                        "cannot infer `Canonical` type for type with const param",
                    ));
                }
                syn::GenericParam::Type(p) => {
                    if self.type_param_is_value_like(p)? {
                        if value_like_param {
                            return Err(syn::Error::new_spanned(
                                p,
                                "multiple generic parameters are `ValueLike`",
                            ));
                        }
                        value_like_param = true;
                    } else {
                        return Err(syn::Error::new_spanned(
                            p,
                            "cannot infer `Canonical` type for type with non-`ValueLike` param",
                        ));
                    }
                }
            }
        }
        self.do_make_canonical_type()
    }

    /// `type Canonical = ...`.
    fn canonical_member(&self) -> syn::Result<Option<syn::ImplItem>> {
        if self.find_ty("Canonical").is_some() {
            Ok(None)
        } else {
            let ty = self.make_canonical_type()?;
            Ok(Some(syn::parse_quote_spanned! { self.span() =>
                type Canonical = #ty;
            }))
        }
    }
}

fn derive_starlark_value_impl(
    attr: StarlarkValueAttrs,
    mut input: syn::ItemImpl,
) -> syn::Result<proc_macro2::TokenStream> {
    let impl_starlark_value: ImplStarlarkValue = is_impl_starlark_value(input.clone(), attr)?;

    let impl_unpack_value = impl_starlark_value.impl_unpack_value()?;

    let please_use_starlark_type_macro = impl_starlark_value.please_use_starlark_type_macro()?;
    let const_type = impl_starlark_value.const_type()?;
    let get_type_value_static = impl_starlark_value.get_type_value_static()?;
    let bin_op_ty = impl_starlark_value.bin_op_ty()?;
    let rbin_op_ty = impl_starlark_value.rbin_op_ty()?;
    let attr_ty = impl_starlark_value.attr_ty()?;
    let bit_or = impl_starlark_value.bit_or()?;
    let canonical = impl_starlark_value.canonical_member()?;

    input.items.splice(
        0..0,
        [
            const_type,
            get_type_value_static,
            please_use_starlark_type_macro,
        ]
        .into_iter()
        .chain(attr_ty)
        .chain(bit_or)
        .chain(bin_op_ty)
        .chain(rbin_op_ty)
        .chain(canonical),
    );

    let has_fn_flags = impl_starlark_value.has_fn_flags(&input)?;
    input.items.extend(has_fn_flags);

    Ok(quote_spanned! {
        input.span() =>

        #impl_unpack_value

        #input
    })
}
