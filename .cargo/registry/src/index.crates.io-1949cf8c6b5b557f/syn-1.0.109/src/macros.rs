#[cfg_attr(
    not(any(feature = "full", feature = "derive")),
    allow(unknown_lints, unused_macro_rules)
)]
macro_rules! ast_struct {
    (
        [$($attrs_pub:tt)*]
        struct $name:ident #full $($rest:tt)*
    ) => {
        #[cfg(feature = "full")]
        $($attrs_pub)* struct $name $($rest)*

        #[cfg(not(feature = "full"))]
        $($attrs_pub)* struct $name {
            _noconstruct: ::std::marker::PhantomData<::proc_macro2::Span>,
        }

        #[cfg(all(not(feature = "full"), feature = "printing"))]
        impl ::quote::ToTokens for $name {
            fn to_tokens(&self, _: &mut ::proc_macro2::TokenStream) {
                unreachable!()
            }
        }
    };

    (
        [$($attrs_pub:tt)*]
        struct $name:ident $($rest:tt)*
    ) => {
        $($attrs_pub)* struct $name $($rest)*
    };

    ($($t:tt)*) => {
        strip_attrs_pub!(ast_struct!($($t)*));
    };
}

macro_rules! ast_enum {
    // Drop the `#no_visit` attribute, if present.
    (
        [$($attrs_pub:tt)*]
        enum $name:ident #no_visit $($rest:tt)*
    ) => (
        ast_enum!([$($attrs_pub)*] enum $name $($rest)*);
    );

    (
        [$($attrs_pub:tt)*]
        enum $name:ident $($rest:tt)*
    ) => (
        $($attrs_pub)* enum $name $($rest)*
    );

    ($($t:tt)*) => {
        strip_attrs_pub!(ast_enum!($($t)*));
    };
}

macro_rules! ast_enum_of_structs {
    (
        $(#[$enum_attr:meta])*
        $pub:ident $enum:ident $name:ident $body:tt
        $($remaining:tt)*
    ) => {
        ast_enum!($(#[$enum_attr])* $pub $enum $name $body);
        ast_enum_of_structs_impl!($pub $enum $name $body $($remaining)*);
    };
}

macro_rules! ast_enum_of_structs_impl {
    (
        $pub:ident $enum:ident $name:ident {
            $(
                $(#[cfg $cfg_attr:tt])*
                $(#[doc $($doc_attr:tt)*])*
                $variant:ident $( ($($member:ident)::+) )*,
            )*
        }

        $($remaining:tt)*
    ) => {
        check_keyword_matches!(pub $pub);
        check_keyword_matches!(enum $enum);

        $($(
            ast_enum_from_struct!($name::$variant, $($member)::+);
        )*)*

        #[cfg(feature = "printing")]
        generate_to_tokens! {
            $($remaining)*
            ()
            tokens
            $name {
                $(
                    $(#[cfg $cfg_attr])*
                    $(#[doc $($doc_attr)*])*
                    $variant $($($member)::+)*,
                )*
            }
        }
    };
}

macro_rules! ast_enum_from_struct {
    // No From<TokenStream> for verbatim variants.
    ($name:ident::Verbatim, $member:ident) => {};

    ($name:ident::$variant:ident, $member:ident) => {
        impl From<$member> for $name {
            fn from(e: $member) -> $name {
                $name::$variant(e)
            }
        }
    };
}

#[cfg(feature = "printing")]
#[cfg_attr(
    not(any(feature = "full", feature = "derive")),
    allow(unknown_lints, unused_macro_rules)
)]
macro_rules! generate_to_tokens {
    (do_not_generate_to_tokens $($foo:tt)*) => ();

    (
        ($($arms:tt)*) $tokens:ident $name:ident {
            $(#[cfg $cfg_attr:tt])*
            $(#[doc $($doc_attr:tt)*])*
            $variant:ident,
            $($next:tt)*
        }
    ) => {
        generate_to_tokens!(
            ($($arms)* $(#[cfg $cfg_attr])* $name::$variant => {})
            $tokens $name { $($next)* }
        );
    };

    (
        ($($arms:tt)*) $tokens:ident $name:ident {
            $(#[cfg $cfg_attr:tt])*
            $(#[doc $($doc_attr:tt)*])*
            $variant:ident $member:ident,
            $($next:tt)*
        }
    ) => {
        generate_to_tokens!(
            ($($arms)* $(#[cfg $cfg_attr])* $name::$variant(_e) => _e.to_tokens($tokens),)
            $tokens $name { $($next)* }
        );
    };

    (($($arms:tt)*) $tokens:ident $name:ident {}) => {
        #[cfg_attr(doc_cfg, doc(cfg(feature = "printing")))]
        impl ::quote::ToTokens for $name {
            fn to_tokens(&self, $tokens: &mut ::proc_macro2::TokenStream) {
                match self {
                    $($arms)*
                }
            }
        }
    };
}

macro_rules! strip_attrs_pub {
    ($mac:ident!($(#[$m:meta])* $pub:ident $($t:tt)*)) => {
        check_keyword_matches!(pub $pub);

        $mac!([$(#[$m])* $pub] $($t)*);
    };
}

macro_rules! check_keyword_matches {
    (enum enum) => {};
    (pub pub) => {};
}
