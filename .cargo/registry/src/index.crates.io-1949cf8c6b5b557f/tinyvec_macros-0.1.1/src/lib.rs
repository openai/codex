#![no_std]
#![forbid(unsafe_code)]

#[macro_export]
macro_rules! impl_mirrored {
    {
    type Mirror = $tinyname:ident;
    $(
        $(#[$attr:meta])*
        $v:vis fn $fname:ident ($seif:ident : $seifty:ty $(,$argname:ident : $argtype:ty)*) $(-> $ret:ty)? ;
    )*
    } => {
        $(
        $(#[$attr])*
        #[inline(always)]
        $v fn $fname($seif : $seifty, $($argname: $argtype),*) $(-> $ret)? {
            match $seif {
                $tinyname::Inline(i) => i.$fname($($argname),*),
                $tinyname::Heap(h) => h.$fname($($argname),*),
            }
        }
        )*
    };
}

