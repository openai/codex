//! This module defines a macro that lets you go from a raw pointer to a struct
//! to a raw pointer to a field of the struct.

macro_rules! generate_addr_of_methods {
    (
    impl<$($gen:ident)*> $struct_name:ty {$(
        $(#[$attrs:meta])*
        $vis:vis unsafe fn $fn_name:ident(self: NonNull<Self>) -> NonNull<$field_type:ty> {
            &self$(.$field_name:tt)+
        }
    )*}
    ) => {
        impl<$($gen)*> $struct_name {$(
            $(#[$attrs])*
            $vis unsafe fn $fn_name(me: ::core::ptr::NonNull<Self>) -> ::core::ptr::NonNull<$field_type> {
                let me = me.as_ptr();
                let field = ::std::ptr::addr_of_mut!((*me) $(.$field_name)+ );
                ::core::ptr::NonNull::new_unchecked(field)
            }
        )*}
    };
}
