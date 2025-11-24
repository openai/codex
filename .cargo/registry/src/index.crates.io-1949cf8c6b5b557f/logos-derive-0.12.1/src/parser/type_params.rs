use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::spanned::Spanned;
use syn::{Lifetime, LifetimeDef, Path, Type};

use crate::error::Errors;

#[derive(Default)]
pub struct TypeParams {
    lifetime: bool,
    type_params: Vec<(Ident, Option<Type>)>,
}

impl TypeParams {
    pub fn explicit_lifetime(&mut self, lt: LifetimeDef, errors: &mut Errors) {
        if self.lifetime {
            let span = lt.span();

            errors.err("Logos types can only have one lifetime can be set", span);
        }

        self.lifetime = true;
    }

    pub fn add(&mut self, param: Ident) {
        self.type_params.push((param, None));
    }

    pub fn set(&mut self, param: Ident, ty: TokenStream, errors: &mut Errors) {
        let ty = match syn::parse2::<Type>(ty) {
            Ok(mut ty) => {
                replace_lifetimes(&mut ty);
                ty
            }
            Err(err) => {
                errors.err(err.to_string(), err.span());
                return;
            }
        };

        match self.type_params.iter_mut().find(|(name, _)| *name == param) {
            Some((_, slot)) => {
                if let Some(previous) = slot.replace(ty) {
                    errors
                        .err(
                            format!("{} can only have one type assigned to it", param),
                            param.span(),
                        )
                        .err("Previously assigned here", previous.span());
                }
            }
            None => {
                errors.err(
                    format!("{} is not a declared type parameter", param),
                    param.span(),
                );
            }
        }
    }

    pub fn find(&self, path: &Path) -> Option<Type> {
        for (ident, ty) in &self.type_params {
            if path.is_ident(ident) {
                return ty.clone();
            }
        }

        None
    }

    pub fn generics(&self, errors: &mut Errors) -> Option<TokenStream> {
        if !self.lifetime && self.type_params.is_empty() {
            return None;
        }

        let mut generics = Vec::new();

        if self.lifetime {
            generics.push(quote!('s));
        }

        for (ty, replace) in self.type_params.iter() {
            match replace {
                Some(ty) => generics.push(quote!(#ty)),
                None => {
                    errors.err(
                        format!(
                            "Generic type parameter without a concrete type\n\
                            \n\
                            Define a concrete type Logos can use: #[logos(type {} = Type)]",
                            ty,
                        ),
                        ty.span(),
                    );
                }
            }
        }

        if generics.is_empty() {
            None
        } else {
            Some(quote!(<#(#generics),*>))
        }
    }
}

pub fn replace_lifetimes(ty: &mut Type) {
    traverse_type(ty, &mut replace_lifetime)
}

pub fn replace_lifetime(ty: &mut Type) {
    use syn::{GenericArgument, PathArguments};

    match ty {
        Type::Path(p) => {
            p.path
                .segments
                .iter_mut()
                .filter_map(|segment| match &mut segment.arguments {
                    PathArguments::AngleBracketed(ab) => Some(ab),
                    _ => None,
                })
                .flat_map(|ab| ab.args.iter_mut())
                .for_each(|arg| {
                    if let GenericArgument::Lifetime(lt) = arg {
                        *lt = Lifetime::new("'s", lt.span());
                    }
                });
        }
        Type::Reference(r) => {
            let span = match r.lifetime.take() {
                Some(lt) => lt.span(),
                None => Span::call_site(),
            };

            r.lifetime = Some(Lifetime::new("'s", span));
        }
        _ => (),
    }
}

pub fn traverse_type(ty: &mut Type, f: &mut impl FnMut(&mut Type)) {
    f(ty);
    match ty {
        Type::Array(array) => traverse_type(&mut array.elem, f),
        Type::BareFn(bare_fn) => {
            for input in &mut bare_fn.inputs {
                traverse_type(&mut input.ty, f);
            }
            if let syn::ReturnType::Type(_, ty) = &mut bare_fn.output {
                traverse_type(ty, f);
            }
        }
        Type::Group(group) => traverse_type(&mut group.elem, f),
        Type::Paren(paren) => traverse_type(&mut paren.elem, f),
        Type::Path(path) => traverse_path(&mut path.path, f),
        Type::Ptr(p) => traverse_type(&mut p.elem, f),
        Type::Reference(r) => traverse_type(&mut r.elem, f),
        Type::Slice(slice) => traverse_type(&mut slice.elem, f),
        Type::TraitObject(object) => object.bounds.iter_mut().for_each(|bound| {
            if let syn::TypeParamBound::Trait(trait_bound) = bound {
                traverse_path(&mut trait_bound.path, f);
            }
        }),
        Type::Tuple(tuple) => tuple
            .elems
            .iter_mut()
            .for_each(|elem| traverse_type(elem, f)),
        _ => (),
    }
}

fn traverse_path(path: &mut Path, f: &mut impl FnMut(&mut Type)) {
    for segment in &mut path.segments {
        match &mut segment.arguments {
            syn::PathArguments::None => (),
            syn::PathArguments::AngleBracketed(args) => {
                for arg in &mut args.args {
                    match arg {
                        syn::GenericArgument::Type(ty) => {
                            traverse_type(ty, f);
                        }
                        syn::GenericArgument::Binding(bind) => {
                            traverse_type(&mut bind.ty, f);
                        }
                        _ => (),
                    }
                }
            }
            syn::PathArguments::Parenthesized(args) => {
                for arg in &mut args.inputs {
                    traverse_type(arg, f);
                }
                if let syn::ReturnType::Type(_, ty) = &mut args.output {
                    traverse_type(ty, f);
                }
            }
        }
    }
}
