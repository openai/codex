/// Define a trait as usual, and a macro that can be used to instantiate
/// implementations of it.
///
/// There *must* be section markers in the trait definition:
/// @section type for associated types
/// @section self for methods
/// @section nodelegate for arbitrary tail that is not forwarded.
macro_rules! trait_template {
    ($(#[$doc:meta])* pub trait $name:ident $($methods:tt)*) => {
        macro_rules! $name {
            ($m:ident $extra:tt) => {
                $m! {
                    $extra
                    pub trait $name $($methods)*
                }
            }
        }

        remove_sections! { []
            $(#[$doc])*
            pub trait $name $($methods)*

            // This is where the trait definition is reproduced by the macro.
            // It makes the source links point to this place!
            //
            // I'm sorry, you'll have to find the source by looking at the
            // source of the module the trait is defined in.
            //
            // We use this nifty macro so that we can automatically generate
            // delegation trait impls and implement the graph traits for more
            // types and combinators.
        }
    }
}

macro_rules! remove_sections_inner {
    ([$($stack:tt)*]) => {
        $($stack)*
    };
    // escape the following tt
    ([$($stack:tt)*] @escape $_x:tt $($t:tt)*) => {
        remove_sections_inner!([$($stack)*] $($t)*);
    };
    ([$($stack:tt)*] @section $x:ident $($t:tt)*) => {
        remove_sections_inner!([$($stack)*] $($t)*);
    };
    ([$($stack:tt)*] $t:tt $($tail:tt)*) => {
        remove_sections_inner!([$($stack)* $t] $($tail)*);
    };
}

// This is the outer layer, just find the { } of the actual trait definition
// recurse once into { }, but not more.
macro_rules! remove_sections {
    ([$($stack:tt)*]) => {
        $($stack)*
    };
    ([$($stack:tt)*] { $($tail:tt)* }) => {
        $($stack)* {
            remove_sections_inner!([] $($tail)*);
        }
    };
    ([$($stack:tt)*] $t:tt $($tail:tt)*) => {
        remove_sections!([$($stack)* $t] $($tail)*);
    };
}

macro_rules! deref {
    ($e:expr) => {
        *$e
    };
}
macro_rules! deref_twice {
    ($e:expr) => {
        **$e
    };
}

/// Implement a trait by delegation. By default as if we are delegating
/// from &G to G.
macro_rules! delegate_impl {
    ([] $($rest:tt)*) => {
        delegate_impl! { [['a, G], G, &'a G, deref] $($rest)* }
    };
    ([[$($param:tt)*], $self_type:ident, $self_wrap:ty, $self_map:ident]
     pub trait $name:ident $(: $sup:ident)* $(+ $more_sup:ident)* {

        // "Escaped" associated types. Stripped before making the `trait`
        // itself, but forwarded when delegating impls.
        $(
        @escape [type $assoc_name_ext:ident]
        // Associated types. Forwarded.
        )*
        $(
        @section type
        $(
            $(#[$_assoc_attr:meta])*
            type $assoc_name:ident $(: $assoc_bound:ty)*;
        )+
        )*
        // Methods. Forwarded. Using $self_map!(self) around the self argument.
        // Methods must use receiver `self` or explicit type like `self: &Self`
        // &self and &mut self are _not_ supported.
        $(
        @section self
        $(
            $(#[$_method_attr:meta])*
            fn $method_name:ident(self $(: $self_selftype:ty)* $(,$marg:ident : $marg_ty:ty)*) $(-> $mret:ty)?;
        )+
        )*
        // Arbitrary tail that is ignored when forwarding.
        $(
        @section nodelegate
        $($tail:tt)*
        )*
    }) => {
        impl<$($param)*> $name for $self_wrap where $self_type: $name {
            $(
            $(
                type $assoc_name = $self_type::$assoc_name;
            )*
            )*
            $(
                type $assoc_name_ext = $self_type::$assoc_name_ext;
            )*
            $(
            $(
                fn $method_name(self $(: $self_selftype)* $(,$marg: $marg_ty)*) $(-> $mret)? {
                    $self_map!(self).$method_name($($marg),*)
                }
            )*
            )*
        }
    }
}
