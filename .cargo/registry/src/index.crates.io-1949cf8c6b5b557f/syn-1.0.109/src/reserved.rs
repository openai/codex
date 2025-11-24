// Type for a syntax tree node that is reserved for future use.
//
// For example ExprReference contains a field `raw` of type Reserved. If `&raw
// place` syntax becomes a thing as per https://github.com/rust-lang/rfcs/pull/2582,
// we can backward compatibly change `raw`'s type to Option<Token![raw]> without
// the possibility of breaking any code.

use proc_macro2::Span;
use std::marker::PhantomData;

#[cfg(feature = "extra-traits")]
use std::fmt::{self, Debug};

ast_struct! {
    pub struct Reserved {
        _private: PhantomData<Span>,
    }
}

impl Default for Reserved {
    fn default() -> Self {
        Reserved {
            _private: PhantomData,
        }
    }
}

#[cfg(feature = "clone-impls")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "clone-impls")))]
impl Clone for Reserved {
    fn clone(&self) -> Self {
        Reserved {
            _private: self._private,
        }
    }
}

#[cfg(feature = "extra-traits")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "extra-traits")))]
impl Debug for Reserved {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.debug_struct("Reserved").finish()
    }
}
