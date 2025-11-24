// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::{maps::ZeroMapBorrowed, maps::ZeroMapKV, ZeroMap};
use databake::*;

impl<'a, K, V> Bake for ZeroMap<'a, K, V>
where
    K: ZeroMapKV<'a> + ?Sized,
    V: ZeroMapKV<'a> + ?Sized,
    K::Container: Bake,
    V::Container: Bake,
{
    fn bake(&self, env: &CrateEnv) -> TokenStream {
        env.insert("zerovec");
        let keys = self.keys.bake(env);
        let values = self.values.bake(env);
        quote! { unsafe { #[allow(unused_unsafe)] zerovec::ZeroMap::from_parts_unchecked(#keys, #values) } }
    }
}

impl<'a, K, V> BakeSize for ZeroMap<'a, K, V>
where
    K: ZeroMapKV<'a> + ?Sized,
    V: ZeroMapKV<'a> + ?Sized,
    K::Container: BakeSize,
    V::Container: BakeSize,
{
    fn borrows_size(&self) -> usize {
        self.keys.borrows_size() + self.values.borrows_size()
    }
}

impl<'a, K, V> Bake for ZeroMapBorrowed<'a, K, V>
where
    K: ZeroMapKV<'a> + ?Sized,
    V: ZeroMapKV<'a> + ?Sized,
    &'a K::Slice: Bake,
    &'a V::Slice: Bake,
{
    fn bake(&self, env: &CrateEnv) -> TokenStream {
        env.insert("zerovec");
        let keys = self.keys.bake(env);
        let values = self.values.bake(env);
        quote! { unsafe { #[allow(unused_unsafe)] zerovec::maps::ZeroMapBorrowed::from_parts_unchecked(#keys, #values) } }
    }
}

impl<'a, K, V> BakeSize for ZeroMapBorrowed<'a, K, V>
where
    K: ZeroMapKV<'a> + ?Sized,
    V: ZeroMapKV<'a> + ?Sized,
    &'a K::Slice: BakeSize,
    &'a V::Slice: BakeSize,
{
    fn borrows_size(&self) -> usize {
        self.keys.borrows_size() + self.values.borrows_size()
    }
}

#[test]
fn test_baked_map() {
    test_bake!(
        ZeroMap<str, str>,
        const,
        unsafe {
            #[allow(unused_unsafe)]
            crate::ZeroMap::from_parts_unchecked(
                unsafe {
                    crate::vecs::VarZeroVec16::from_bytes_unchecked(
                        b"\x02\0\0\0\0\0\0\0\x02\0\0\0adbc"
                    )
                },
                unsafe {
                    crate::vecs::VarZeroVec16::from_bytes_unchecked(
                        b"\x02\0\0\0\0\0\0\0\x04\0\0\0ERA1ERA0"
                    )
                },
            )
        },
        zerovec
    );
}

#[test]
fn test_baked_borrowed_map() {
    test_bake!(
        ZeroMapBorrowed<str, str>,
        const,
        unsafe {
            #[allow(unused_unsafe)]
            crate::maps::ZeroMapBorrowed::from_parts_unchecked(
                unsafe {
                    crate::vecs::VarZeroSlice16::from_bytes_unchecked(
                        b"\x02\0\0\0\0\0\0\0\x02\0\0\0adbc"
                    )
                },
                unsafe {
                    crate::vecs::VarZeroSlice16::from_bytes_unchecked(
                        b"\x02\0\0\0\0\0\0\0\x04\0\0\0ERA1ERA0"
                    )
                },
            )
        },
        zerovec
    );
}
