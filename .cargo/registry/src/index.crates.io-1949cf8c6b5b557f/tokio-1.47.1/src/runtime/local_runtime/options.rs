use std::marker::PhantomData;

/// [`LocalRuntime`]-only config options
///
/// Currently, there are no such options, but in the future, things like `!Send + !Sync` hooks may
/// be added.
///
/// Use `LocalOptions::default()` to create the default set of options. This type is used with
/// [`Builder::build_local`].
///
/// [`Builder::build_local`]: crate::runtime::Builder::build_local
/// [`LocalRuntime`]: crate::runtime::LocalRuntime
#[derive(Default, Debug)]
#[non_exhaustive]
pub struct LocalOptions {
    /// Marker used to make this !Send and !Sync.
    _phantom: PhantomData<*mut u8>,
}
