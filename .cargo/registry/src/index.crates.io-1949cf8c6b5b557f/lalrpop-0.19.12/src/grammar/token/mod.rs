use std::collections::HashMap;

use grammar::parse_tree::TypeRef;
use string_cache::DefaultAtom as Atom;

#[cfg(test)]
mod test;

pub struct TokenDefinition {
    // if the enum type is `foo::bar::baz<X,Y>` then:
    enum_type: TypeRef,

    // map from a custom string, like `"("` to a variant name like LPAREN
    token_map: HashMap<Atom, Atom>,
}

impl TokenDefinition {
    pub fn new(enum_type: TypeRef, token_map: Vec<(Atom, Atom)>) -> TokenDefinition {
        TokenDefinition {
            enum_type,
            token_map: token_map.into_iter().collect(),
        }
    }

    pub fn enum_type(&self) -> &TypeRef {
        &self.enum_type
    }
}
