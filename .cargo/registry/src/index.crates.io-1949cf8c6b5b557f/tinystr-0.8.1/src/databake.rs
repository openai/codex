// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::TinyAsciiStr;
use crate::UnvalidatedTinyAsciiStr;
use databake::*;

impl<const N: usize> Bake for TinyAsciiStr<N> {
    fn bake(&self, env: &CrateEnv) -> TokenStream {
        env.insert("tinystr");
        let string = self.as_str();
        quote! {
            tinystr::tinystr!(#N, #string)
        }
    }
}

impl<const N: usize> BakeSize for TinyAsciiStr<N> {
    fn borrows_size(&self) -> usize {
        0
    }
}

impl<const N: usize> databake::Bake for UnvalidatedTinyAsciiStr<N> {
    fn bake(&self, env: &databake::CrateEnv) -> databake::TokenStream {
        match self.try_into_tinystr() {
            Ok(tiny) => {
                let tiny = tiny.bake(env);
                databake::quote! {
                    #tiny.to_unvalidated()
                }
            }
            Err(_) => {
                let bytes = self.0.bake(env);
                env.insert("tinystr");
                databake::quote! {
                    tinystr::UnvalidatedTinyAsciiStr::from_utf8_unchecked(#bytes)
                }
            }
        }
    }
}

impl<const N: usize> databake::BakeSize for UnvalidatedTinyAsciiStr<N> {
    fn borrows_size(&self) -> usize {
        0
    }
}

#[test]
fn test() {
    test_bake!(
        TinyAsciiStr<10>,
        const,
        crate::tinystr!(10usize, "foo"),
        tinystr
    );
}

#[test]
fn test_unvalidated() {
    test_bake!(
        UnvalidatedTinyAsciiStr<10>,
        const,
        crate::tinystr!(10usize, "foo").to_unvalidated(),
        tinystr
    );
    test_bake!(
        UnvalidatedTinyAsciiStr<3>,
        const,
        crate::UnvalidatedTinyAsciiStr::from_utf8_unchecked(*b"AB\xCD"),
        tinystr
    );
}
