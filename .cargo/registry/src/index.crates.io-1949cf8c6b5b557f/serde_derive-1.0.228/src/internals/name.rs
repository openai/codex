use crate::internals::attr::{Attr, VecAttr};
use proc_macro2::{Ident, Span, TokenStream};
use quote::ToTokens;
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fmt::{self, Display};
use syn::LitStr;

pub struct MultiName {
    pub(crate) serialize: Name,
    pub(crate) serialize_renamed: bool,
    pub(crate) deserialize: Name,
    pub(crate) deserialize_renamed: bool,
    pub(crate) deserialize_aliases: BTreeSet<Name>,
}

impl MultiName {
    pub(crate) fn from_attrs(
        source_name: Name,
        ser_name: Attr<Name>,
        de_name: Attr<Name>,
        de_aliases: Option<VecAttr<Name>>,
    ) -> Self {
        let mut alias_set = BTreeSet::new();
        if let Some(de_aliases) = de_aliases {
            for alias_name in de_aliases.get() {
                alias_set.insert(alias_name);
            }
        }

        let ser_name = ser_name.get();
        let ser_renamed = ser_name.is_some();
        let de_name = de_name.get();
        let de_renamed = de_name.is_some();
        MultiName {
            serialize: ser_name.unwrap_or_else(|| source_name.clone()),
            serialize_renamed: ser_renamed,
            deserialize: de_name.unwrap_or(source_name),
            deserialize_renamed: de_renamed,
            deserialize_aliases: alias_set,
        }
    }

    /// Return the container name for the container when serializing.
    pub fn serialize_name(&self) -> &Name {
        &self.serialize
    }

    /// Return the container name for the container when deserializing.
    pub fn deserialize_name(&self) -> &Name {
        &self.deserialize
    }

    pub(crate) fn deserialize_aliases(&self) -> &BTreeSet<Name> {
        &self.deserialize_aliases
    }
}

#[derive(Clone)]
pub struct Name {
    pub value: String,
    pub span: Span,
}

impl ToTokens for Name {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        LitStr::new(&self.value, self.span).to_tokens(tokens);
    }
}

impl Ord for Name {
    fn cmp(&self, other: &Self) -> Ordering {
        Ord::cmp(&self.value, &other.value)
    }
}

impl PartialOrd for Name {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(Ord::cmp(self, other))
    }
}

impl Eq for Name {}

impl PartialEq for Name {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl From<&Ident> for Name {
    fn from(ident: &Ident) -> Self {
        Name {
            value: ident.to_string(),
            span: ident.span(),
        }
    }
}

impl From<&LitStr> for Name {
    fn from(lit: &LitStr) -> Self {
        Name {
            value: lit.value(),
            span: lit.span(),
        }
    }
}

impl Display for Name {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.value, formatter)
    }
}
