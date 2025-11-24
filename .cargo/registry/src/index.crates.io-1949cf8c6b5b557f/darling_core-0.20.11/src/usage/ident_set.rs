use fnv::FnvHashSet;
use syn::Ident;

/// A set of idents.
pub type IdentSet = FnvHashSet<Ident>;

/// A set of references to idents.
pub type IdentRefSet<'a> = FnvHashSet<&'a Ident>;
