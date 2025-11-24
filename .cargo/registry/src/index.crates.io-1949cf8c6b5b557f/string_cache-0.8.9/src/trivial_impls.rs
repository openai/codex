// Copyright 2014 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::{Atom, StaticAtomSet};
#[cfg(feature = "serde_support")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Cow;
use std::fmt;

impl<Static: StaticAtomSet> ::precomputed_hash::PrecomputedHash for Atom<Static> {
    fn precomputed_hash(&self) -> u32 {
        self.get_hash()
    }
}

impl<'a, Static: StaticAtomSet> From<&'a Atom<Static>> for Atom<Static> {
    fn from(atom: &'a Self) -> Self {
        atom.clone()
    }
}

impl<Static: StaticAtomSet> PartialEq<str> for Atom<Static> {
    fn eq(&self, other: &str) -> bool {
        &self[..] == other
    }
}

impl<Static: StaticAtomSet> PartialEq<Atom<Static>> for str {
    fn eq(&self, other: &Atom<Static>) -> bool {
        self == &other[..]
    }
}

impl<Static: StaticAtomSet> PartialEq<String> for Atom<Static> {
    fn eq(&self, other: &String) -> bool {
        self[..] == other[..]
    }
}

impl<'a, Static: StaticAtomSet> From<&'a str> for Atom<Static> {
    #[inline]
    fn from(string_to_add: &str) -> Self {
        Atom::from(Cow::Borrowed(string_to_add))
    }
}

impl<Static: StaticAtomSet> From<String> for Atom<Static> {
    #[inline]
    fn from(string_to_add: String) -> Self {
        Atom::from(Cow::Owned(string_to_add))
    }
}

impl<Static: StaticAtomSet> fmt::Display for Atom<Static> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        <str as fmt::Display>::fmt(self, f)
    }
}

impl<Static: StaticAtomSet> AsRef<str> for Atom<Static> {
    fn as_ref(&self) -> &str {
        self
    }
}

#[cfg(feature = "serde_support")]
impl<Static: StaticAtomSet> Serialize for Atom<Static> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let string: &str = self.as_ref();
        string.serialize(serializer)
    }
}

#[cfg(feature = "serde_support")]
impl<'a, Static: StaticAtomSet> Deserialize<'a> for Atom<Static> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'a>,
    {
        use serde::de;
        use std::marker::PhantomData;

        struct AtomVisitor<Static: StaticAtomSet>(PhantomData<Static>);

        impl<'de, Static: StaticAtomSet> de::Visitor<'de> for AtomVisitor<Static> {
            type Value = Atom<Static>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "an Atom")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Atom::from(v))
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Atom::from(v))
            }
        }

        deserializer.deserialize_str(AtomVisitor(PhantomData))
    }
}
