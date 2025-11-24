/// Pins a value on the stack.
///
/// # Example
///
/// ```rust
/// # use pin_utils::pin_mut;
/// # use core::pin::Pin;
/// # struct Foo {}
/// let foo = Foo { /* ... */ };
/// pin_mut!(foo);
/// let _: Pin<&mut Foo> = foo;
/// ```
#[macro_export]
macro_rules! pin_mut {
    ($($x:ident),* $(,)?) => { $(
        // Move the value to ensure that it is owned
        let mut $x = $x;
        // Shadow the original binding so that it can't be directly accessed
        // ever again.
        #[allow(unused_mut)]
        let mut $x = unsafe {
            $crate::core_reexport::pin::Pin::new_unchecked(&mut $x)
        };
    )* }
}
