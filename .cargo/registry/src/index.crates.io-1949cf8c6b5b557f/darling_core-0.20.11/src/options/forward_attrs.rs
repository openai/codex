use crate::ast::NestedMeta;
use crate::util::PathList;
use crate::{FromMeta, Result};

/// A rule about which attributes to forward to the generated struct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForwardAttrsFilter {
    All,
    Only(PathList),
}

impl ForwardAttrsFilter {
    /// Returns `true` if this will not forward any attributes.
    pub fn is_empty(&self) -> bool {
        match *self {
            ForwardAttrsFilter::All => false,
            ForwardAttrsFilter::Only(ref list) => list.is_empty(),
        }
    }
}

impl FromMeta for ForwardAttrsFilter {
    fn from_word() -> Result<Self> {
        Ok(ForwardAttrsFilter::All)
    }

    fn from_list(nested: &[NestedMeta]) -> Result<Self> {
        Ok(ForwardAttrsFilter::Only(PathList::from_list(nested)?))
    }
}
