use syn::Attribute;

use crate::Result;

/// Create an instance by parsing a list of attributes.
///
/// This trait is useful when dealing with items such as traits on traits and impl blocks,
/// for which `darling` does not provide dedicated traits.
pub trait FromAttributes: Sized {
    /// Create an instance by parsing a list of attributes.
    ///
    /// By convention, `FromAttributes` implementations should merge item
    /// declarations across attributes, so that the following forms are
    /// equivalent:
    ///
    /// ```rust,ignore
    /// #[derive(Serialize)]
    /// #[serde(rename_all = "camel_case")]
    /// #[serde(borrow)]
    /// pub struct SplitExample {}
    ///
    /// #[derive(Serialize)]
    /// #[serde(borrow, rename_all = "camel_case")]
    /// pub struct JoinedExample {}
    /// ```
    fn from_attributes(attrs: &[Attribute]) -> Result<Self>;
}
