use std::{collections::HashSet, rc::Rc};

use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{Path, Type};

pub struct Dependencies {
    crate_rename: Rc<Path>,
    dependencies: HashSet<Dependency>,
    types: HashSet<Rc<Type>>,
}

#[derive(Hash, Eq, PartialEq)]
enum Dependency {
    // A dependency on all dependencies of `ty`.
    // This does not include a dependency on `ty` itself - only its dependencies!
    Transitive {
        crate_rename: Rc<Path>,
        ty: Rc<Type>,
    },
    // A dependency on all type parameters of `ty`, as returned by `TS::generics()`.
    // This does not include a dependency on `ty` itself.
    Generics {
        crate_rename: Rc<Path>,
        ty: Rc<Type>,
    },
    Type(Rc<Type>),
}

impl Dependencies {
    pub fn new(crate_rename: Path) -> Self {
        Self {
            dependencies: HashSet::new(),
            crate_rename: Rc::new(crate_rename),
            types: HashSet::new(),
        }
    }

    pub fn used_types(&self) -> impl Iterator<Item = &Type> {
        self.types.iter().map(Rc::as_ref)
    }

    /// Adds all dependencies from the given type
    pub fn append_from(&mut self, ty: &Type) {
        let ty = self.push_type(ty);
        self.dependencies.insert(Dependency::Transitive {
            crate_rename: self.crate_rename.clone(),
            ty: ty.clone(),
        });
    }

    /// Adds the given type.
    pub fn push(&mut self, ty: &Type) {
        let ty = self.push_type(ty);
        self.dependencies.insert(Dependency::Type(ty.clone()));
        self.dependencies.insert(Dependency::Generics {
            crate_rename: self.crate_rename.clone(),
            ty: ty.clone(),
        });
    }

    pub fn append(&mut self, other: Dependencies) {
        self.dependencies.extend(other.dependencies);
        self.types.extend(other.types);
    }

    fn push_type(&mut self, ty: &Type) -> Rc<Type> {
        // this can be replaces with `get_or_insert_owned` once #60896 is stabilized
        match self.types.get(ty) {
            None => {
                let ty = Rc::new(ty.clone());
                self.types.insert(ty.clone());
                ty
            }
            Some(ty) => ty.clone(),
        }
    }
}

impl ToTokens for Dependencies {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let lines = self.dependencies.iter();

        tokens.extend(quote![
            #(#lines;)*
        ]);
    }
}

impl ToTokens for Dependency {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.extend(match self {
            Dependency::Transitive { crate_rename, ty } => {
                quote![<#ty as #crate_rename::TS>::visit_dependencies(v)]
            }
            Dependency::Generics { crate_rename, ty } => {
                quote![<#ty as #crate_rename::TS>::visit_generics(v)]
            }
            Dependency::Type(ty) => quote![v.visit::<#ty>()],
        });
    }
}
