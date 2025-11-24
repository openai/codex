#![allow(unused_macros)]

// vendored from the cfg-if crate to avoid breaking ctest
macro_rules! cfg_if {
    // match if/else chains with a final `else`
    ($(
        if #[cfg($($meta:meta),*)] { $($it:item)* }
    ) else * else {
        $($it2:item)*
    }) => {
        cfg_if! {
            @__items
            () ;
            $( ( ($($meta),*) ($($it)*) ), )*
            ( () ($($it2)*) ),
        }
    };

    // match if/else chains lacking a final `else`
    (
        if #[cfg($($i_met:meta),*)] { $($i_it:item)* }
        $(
            else if #[cfg($($e_met:meta),*)] { $($e_it:item)* }
        )*
    ) => {
        cfg_if! {
            @__items
            () ;
            ( ($($i_met),*) ($($i_it)*) ),
            $( ( ($($e_met),*) ($($e_it)*) ), )*
            ( () () ),
        }
    };

    // Internal and recursive macro to emit all the items
    //
    // Collects all the negated cfgs in a list at the beginning and after the
    // semicolon is all the remaining items
    (@__items ($($not:meta,)*) ; ) => {};
    (@__items ($($not:meta,)*) ; ( ($($m:meta),*) ($($it:item)*) ), $($rest:tt)*) => {
        // Emit all items within one block, applying an appropriate #[cfg]. The
        // #[cfg] will require all `$m` matchers specified and must also negate
        // all previous matchers.
        cfg_if! { @__apply cfg(all($($m,)* not(any($($not),*)))), $($it)* }

        // Recurse to emit all other items in `$rest`, and when we do so add all
        // our `$m` matchers to the list of `$not` matchers as future emissions
        // will have to negate everything we just matched as well.
        cfg_if! { @__items ($($not,)* $($m,)*) ; $($rest)* }
    };

    // Internal macro to Apply a cfg attribute to a list of items
    (@__apply $m:meta, $($it:item)*) => {
        $(#[$m] $it)*
    };
}

macro_rules! stack {
    ($t:ident) => {
        cfg_if! {
            if #[cfg(any(ossl110, libressl390))] {
                pub enum $t {}
            } else {
                #[repr(C)]
                pub struct $t {
                    pub stack: $crate::_STACK,
                }
            }
        }
    };
}

// openssl changes `*mut` to `*const` in certain parameters in certain versions;
// in C this is ABI and (mostly) API compatible.
//
// We need to handle this explicitly, and this macro helps annotate which
// parameter got converted in which version.
//
// Input is:
//    extern "C" {
//        #[attributes...]
//        pub fn name(args) -> rettype; // `-> rettype` optional
//        // more functions...
//    }
//
// This macro replaces `#[const_ptr_if(...)]` in types with `*const` or `*mut`
// (depending on the inner cfg flags)
//
// Walks through all argument and return types, but only finds inner types of
// `*const` and `*mut`; doesn't walk arrays or generics.
//
// NOTE: can't abstract `pub` as `$fn_vis:vis`, as ctest macro handling doesn't
// support it (old syntax crate). But we really only need `pub` anyway.
//
// NOTE: ctest seams to simply ignore macros it can't expand (whatever the
// reason)
macro_rules! const_ptr_api {
    // ----------------------------------------------------------------
    // (partialarg): partial argument, waiting for "final" argument type
    // MAGIC PART 1: hande conditional const ptr in argument type
    ( (partialarg)
        { $(#[$fn_attr:meta])* pub fn $fn_name:ident }
        $args_packed:tt
        [ $($part_arg:tt)* ]
        [ #[const_ptr_if( $($cfg:tt)* )] $($arg_rem:tt)* ]
        $ret_packed:tt
    ) => {
        const_ptr_api!( (partialarg) { #[cfg($($cfg)*)]      $(#[$fn_attr])* pub fn $fn_name } $args_packed [ $($part_arg)* *const ] [ $($arg_rem)* ] $ret_packed );
        const_ptr_api!( (partialarg) { #[cfg(not($($cfg)*))] $(#[$fn_attr])* pub fn $fn_name } $args_packed [ $($part_arg)* *mut   ] [ $($arg_rem)* ] $ret_packed );
    };
    // continue partial argument with `*mut` pointer (might need special const handling in inner type)
    ( (partialarg)
        $def_packed:tt
        $args_packed:tt
        [ $($part_arg:tt)* ]
        [ *mut $($arg_rem:tt)* ]
        $ret_packed:tt
    ) => {
        const_ptr_api!( (partialarg) $def_packed $args_packed [ $($part_arg)* *mut ] [ $($arg_rem)* ] $ret_packed );
    };
    // continue partial argument with `*const` pointer (might need special const handling in inner type)
    ( (partialarg)
        $def_packed:tt
        $args_packed:tt
        [ $($part_arg:tt)* ]
        [ *const $($arg_rem:tt)* ]
        $ret_packed:tt
    ) => {
        const_ptr_api!( (partialarg) $def_packed $args_packed [ $($part_arg)* *const ] [ $($arg_rem)* ] $ret_packed );
    };
    // finish partial argument with trailing comma
    ( (partialarg)
        $def_packed:tt
        { $($args_tt:tt)* }
        [ $($part_arg:tt)* ]
        [ $arg_ty:ty, $($arg_rem:tt)* ]
        $ret_packed:tt
    ) => {
        const_ptr_api!( (parseargs) $def_packed { $($args_tt)* { $($part_arg)* $arg_ty } } [ $($arg_rem)* ] $ret_packed );
    };
    // finish final partial argument (no trailing comma)
    ( (partialarg)
        $def_packed:tt
        { $($args_tt:tt)* }
        [ $($part_arg:tt)* ]
        [ $arg_ty:ty ]
        $ret_packed:tt
    ) => {
        const_ptr_api!( (parseargs) $def_packed { $($args_tt)* { $($part_arg)* $arg_ty } } [ ] $ret_packed );
    };

    // ----------------------------------------------------------------
    // (parseargs): parsing arguments
    // start next argument
    ( (parseargs)
        $def_packed:tt
        $args_packed:tt
        [ $arg_name:ident : $($arg_rem:tt)* ]
        $ret_packed:tt
    ) => {
        const_ptr_api!( (partialarg) $def_packed $args_packed [ $arg_name: ] [ $($arg_rem)* ] $ret_packed );
    };
    // end of arguments, there is a return type; start parsing it
    ( (parseargs)
        $def_packed:tt
        $args_packed:tt
        [ ]
        [ -> $($rem:tt)* ]
    ) => {
        const_ptr_api!( (partialret) $def_packed $args_packed [] [ $($rem)* ] );
    };
    // end of arguments, no return type
    ( (parseargs)
        $def_packed:tt
        $args_packed:tt
        [ ]
        [ ]
    ) => {
        const_ptr_api!( (generate) $def_packed $args_packed { () } );
    };

    // ----------------------------------------------------------------
    // (partialret): have partial return type, waiting for final return type
    // MAGIC PART 2: hande conditional const ptr in return type
    ( (partialret)
        { $(#[$fn_attr:meta])* pub fn $fn_name:ident }
        $args_packed:tt
        [ $($part_ret:tt)* ]
        [ #[const_ptr_if( $($cfg:tt)* )] $($rem:tt)* ]
    ) => {
        const_ptr_api!( (partialret) { #[cfg($($cfg)*)]      $(#[$fn_attr])* pub fn $fn_name } $args_packed [ $($part_ret)* *const ] [ $($rem)* ] );
        const_ptr_api!( (partialret) { #[cfg(not($($cfg)*))] $(#[$fn_attr])* pub fn $fn_name } $args_packed [ $($part_ret)* *mut   ] [ $($rem)* ] );
    };
    // `* mut` part in return type; continue parsing to find inner conditional const ptr
    ( (partialret)
        $def_packed:tt
        $args_packed:tt
        [ $($part_ret:tt)* ]
        [ *mut $($rem:tt)* ]
    ) => {
        const_ptr_api!( (partialret) $def_packed $args_packed [ $($part_ret)* *mut ] [ $($rem)* ] );
    };
    // `* const` part in return type; continue parsing to find inner conditional const ptr
    ( (partialret)
        $def_packed:tt
        $args_packed:tt
        [ $($part_ret:tt)* ]
        [ *const $($rem:tt)* ]
    ) => {
        const_ptr_api!( (partialret) $def_packed $args_packed [ $($part_ret)* *const ] [ $($rem)* ] );
    };
    // final part of return type
    ( (partialret)
        $def_packed:tt
        $args_packed:tt
        [ $($part_ret:tt)* ]
        [ $ret_ty:ty ]
    ) => {
        const_ptr_api!( (generate) $def_packed $args_packed { $($part_ret)* $ret_ty } );
    };

    // ----------------------------------------------------------------
    // generate
    ( (generate)
        { $(#[$fn_attr:meta])* pub fn $fn_name:ident }
        { $({ $arg_name:ident: $($arg_ty:tt)* })* }
        { $ret_ty:ty }
    ) => {
        extern "C" {
            $(#[$fn_attr])*
            pub fn $fn_name( $(
                $arg_name: $($arg_ty)*
            ),* ) -> $ret_ty;
        }
    };

    // ----------------------------------------------------------------
    // (fn): gather tokens for return type until ";"
    // found end; start parsing current function, and parse remaining functions
    ( (fn)
        $def_packed:tt
        $arg_tts_packed:tt
        $ret_packed:tt
        [ ; $($rem:tt)* ]
    ) => {
        const_ptr_api!( (parseargs) $def_packed {} $arg_tts_packed $ret_packed );
        const_ptr_api!( (extern) [ $($rem)* ] );
    };
    // not ";" - all other tokens are part of the return type.
    // don't expand return type yet; otherwise we'd have to remember in which branch `rem` needs
    // to be used to parse further functions.
    ( (fn)
        $def_packed:tt
        $arg_tts_packed:tt
        [ $($ret_tt:tt)* ]
        [ $tt:tt $($rem:tt)* ]
    ) => {
        const_ptr_api!( (fn) $def_packed $arg_tts_packed [ $($ret_tt)* $tt ] [ $($rem)* ] );
    };

    // ----------------------------------------------------------------
    // (extern): in extern block, find next function
    // try to split into functions as fast as possible to reduce recursion depth
    ( (extern) [
        $(#[$fn_attr:meta])*
        pub fn $fn_name:ident( $($arg_rem:tt)* ) $($rem:tt)*
    ] ) => {
        const_ptr_api!( (fn)
            { $(#[$fn_attr])* pub fn $fn_name } [ $($arg_rem)* ] [] [ $($rem)* ]
        );
    };
    // end of extern block
    ( (extern) [] ) => {};

    // ----------------------------------------------------------------
    // macro start; find extern block
    ( extern "C" { $($rem:tt)* } ) => {
        const_ptr_api!( (extern) [ $($rem)* ] );
    };
}
