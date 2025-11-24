//! 
//! Implementation's internal macros

macro_rules! debug_fmt_fields {
    ($tyname:ident, $($($field:tt/*TODO ideally we would accept ident or tuple element here*/).+),*) => {
        fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
            f.debug_struct(stringify!($tyname))
                $(
              .field(stringify!($($field).+), &self.$($field).+)
              )*
              .finish()
        }
    }
}

macro_rules! clone_fields {
    ($($field:ident),*) => {
        #[inline] // TODO is this sensible?
        fn clone(&self) -> Self {
            Self {
                $($field: self.$field.clone(),)*
            }
        }
    }
}

macro_rules! ignore_ident{
    ($id:ident, $($t:tt)*) => {$($t)*};
}
