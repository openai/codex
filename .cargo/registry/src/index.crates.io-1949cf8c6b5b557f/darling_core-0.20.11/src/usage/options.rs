/// The goal of tracing generic parameter usage.
///
/// Not all uses of type parameters imply a need to add bounds to a generated trait impl.
/// For example, a field of type `<Vec<T> as a::b::Trait>::Associated` does not need a
/// `where T: Serialize` bound in `serde`.
/// However, a proc macro that is attempting to generate a helper struct _would_ need to
/// know about this usage, or else the generated code would reference an unknown type `T`
/// and fail to compile.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Purpose {
    /// The tracing is being used to generate an `impl` block.
    ///
    /// Uses such as `syn::TypePath.qself` will _not_ be returned.
    BoundImpl,
    /// The tracing is being used to generate a new struct or enum.
    ///
    /// All uses will be returned.
    Declare,
}

/// Control struct for searching type parameters.
///
/// This acts as the search context, preserving information that might have been
/// kept on a visitor in a different implementation.
/// Trait implementers are required to pass this through on any invocations they make.
///
/// # Usage
/// For extensibility, `Options` hides all of its fields from consumers.
/// To create an instance, use the `From<Purpose>` trait implementation:
///
/// ```rust
/// # use darling_core::usage::{Options, Purpose};
/// let opts: Options = Purpose::BoundImpl.into();
/// assert!(!opts.include_type_path_qself());
/// ```
#[derive(Debug, Clone)]
pub struct Options {
    purpose: Purpose,
    #[doc(hidden)]
    __nonexhaustive: (),
}

impl From<Purpose> for Options {
    fn from(purpose: Purpose) -> Self {
        Self {
            purpose,
            __nonexhaustive: (),
        }
    }
}

impl Options {
    /// Returns `true` if the implementer of `UseTypeParams` should search
    /// `<___ as ...>::...` when looking for type parameter uses.
    pub fn include_type_path_qself(&self) -> bool {
        self.purpose == Purpose::Declare
    }
}
