#![no_std]
/// `LanguageFn` wraps a C function that returns a pointer to a tree-sitter grammar.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct LanguageFn(unsafe extern "C" fn() -> *const ());

impl LanguageFn {
    /// Creates a [`LanguageFn`].
    ///
    /// # Safety
    ///
    /// Only call this with language functions generated from grammars
    /// by the Tree-sitter CLI.
    pub const unsafe fn from_raw(f: unsafe extern "C" fn() -> *const ()) -> Self {
        Self(f)
    }

    /// Gets the function wrapped by this [`LanguageFn`].
    #[must_use]
    pub const fn into_raw(self) -> unsafe extern "C" fn() -> *const () {
        self.0
    }
}
