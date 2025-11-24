use backtrace::BacktraceSymbol;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ptr;

/// A symbol in a backtrace.
///
/// This wrapper type serves two purposes. The first is that it provides a
/// representation of a symbol that can be inserted into hashmaps and hashsets;
/// the [`backtrace`] crate does not define [`Hash`], [`PartialEq`], or [`Eq`]
/// on [`BacktraceSymbol`], and recommends that users define their own wrapper
/// which implements these traits.
///
/// Second, this wrapper includes a `parent_hash` field that uniquely
/// identifies this symbol's position in its trace. Otherwise, e.g., our code
/// would not be able to distinguish between recursive calls of a function at
/// different depths.
#[derive(Clone)]
pub(super) struct Symbol {
    pub(super) symbol: BacktraceSymbol,
    pub(super) parent_hash: u64,
}

impl Hash for Symbol {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        if let Some(name) = self.symbol.name() {
            name.as_bytes().hash(state);
        }

        if let Some(addr) = self.symbol.addr() {
            ptr::hash(addr, state);
        }

        self.symbol.filename().hash(state);
        self.symbol.lineno().hash(state);
        self.symbol.colno().hash(state);
        self.parent_hash.hash(state);
    }
}

impl PartialEq for Symbol {
    fn eq(&self, other: &Self) -> bool {
        (self.parent_hash == other.parent_hash)
            && match (self.symbol.name(), other.symbol.name()) {
                (None, None) => true,
                (Some(lhs_name), Some(rhs_name)) => lhs_name.as_bytes() == rhs_name.as_bytes(),
                _ => false,
            }
            && match (self.symbol.addr(), other.symbol.addr()) {
                (None, None) => true,
                (Some(lhs_addr), Some(rhs_addr)) => ptr::eq(lhs_addr, rhs_addr),
                _ => false,
            }
            && (self.symbol.filename() == other.symbol.filename())
            && (self.symbol.lineno() == other.symbol.lineno())
            && (self.symbol.colno() == other.symbol.colno())
    }
}

impl Eq for Symbol {}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(name) = self.symbol.name() {
            let name = name.to_string();
            let name = if let Some((name, _)) = name.rsplit_once("::") {
                name
            } else {
                &name
            };
            fmt::Display::fmt(&name, f)?;
        }

        if let Some(filename) = self.symbol.filename() {
            f.write_str(" at ")?;
            filename.to_string_lossy().fmt(f)?;
            if let Some(lineno) = self.symbol.lineno() {
                f.write_str(":")?;
                fmt::Display::fmt(&lineno, f)?;
                if let Some(colno) = self.symbol.colno() {
                    f.write_str(":")?;
                    fmt::Display::fmt(&colno, f)?;
                }
            }
        }

        Ok(())
    }
}
