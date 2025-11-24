use syn::Generics;

use crate::usage::{IdentSet, LifetimeSet};

/// Extension trait for pulling specific generics data from a generics AST representation.
pub trait GenericsExt {
    /// Get the set of all lifetimes declared by the syntax element.
    /// This does not look for usage of the lifetime; see `UsesLifetimes` for that.
    fn declared_lifetimes(&self) -> LifetimeSet;

    /// Get the set of all type parameters declared by the syntax element.
    /// This does not look for usage of the type parameter; see `UsesTypeParams` for that.
    fn declared_type_params(&self) -> IdentSet;
}

impl GenericsExt for Generics {
    fn declared_lifetimes(&self) -> LifetimeSet {
        self.lifetimes().map(|lt| lt.lifetime.clone()).collect()
    }

    fn declared_type_params(&self) -> IdentSet {
        self.type_params().map(|tp| tp.ident.clone()).collect()
    }
}
