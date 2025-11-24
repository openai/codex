#[cfg(any(
    feature = "util",
    feature = "spawn-ready",
    feature = "filter",
    feature = "make"
))]
macro_rules! opaque_future {
    ($(#[$m:meta])* pub type $name:ident<$($param:ident),+> = $actual:ty;) => {
        pin_project_lite::pin_project! {
            $(#[$m])*
            pub struct $name<$($param),+> {
                #[pin]
                inner: $actual
            }
        }

        impl<$($param),+> $name<$($param),+> {
            pub(crate) fn new(inner: $actual) -> Self {
                Self {
                    inner
                }
            }
        }

        impl<$($param),+> std::fmt::Debug for $name<$($param),+> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_tuple(stringify!($name)).field(&format_args!("...")).finish()
            }
        }

        impl<$($param),+> std::future::Future for $name<$($param),+>
        where
            $actual: std::future::Future,
        {
            type Output = <$actual as std::future::Future>::Output;
            #[inline]
            fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
                self.project().inner.poll(cx)
            }
        }
    }
}
