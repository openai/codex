use crate::protocol::Breadcrumb;
/// A helper trait that converts self into an Iterator of Breadcrumbs.
///
/// This is used for the [`add_breadcrumb`] function.
///
/// [`add_breadcrumb`]: fn.add_breadcrumb.html
pub trait IntoBreadcrumbs {
    /// The iterator type for the breadcrumbs.
    type Output: Iterator<Item = Breadcrumb>;

    /// This converts the object into an optional breadcrumb.
    fn into_breadcrumbs(self) -> Self::Output;
}

impl IntoBreadcrumbs for Breadcrumb {
    type Output = std::iter::Once<Breadcrumb>;

    fn into_breadcrumbs(self) -> Self::Output {
        std::iter::once(self)
    }
}

impl IntoBreadcrumbs for Vec<Breadcrumb> {
    type Output = std::vec::IntoIter<Breadcrumb>;

    fn into_breadcrumbs(self) -> Self::Output {
        self.into_iter()
    }
}

impl IntoBreadcrumbs for Option<Breadcrumb> {
    type Output = std::option::IntoIter<Breadcrumb>;

    fn into_breadcrumbs(self) -> Self::Output {
        self.into_iter()
    }
}

impl<F: FnOnce() -> I, I: IntoBreadcrumbs> IntoBreadcrumbs for F {
    type Output = I::Output;

    fn into_breadcrumbs(self) -> Self::Output {
        self().into_breadcrumbs()
    }
}
