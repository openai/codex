// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use zerovec::ule::AsULE;
use zerovec::ule::EncodeAsVarULE;
use zerovec::*;

#[repr(C, packed)]
#[derive(ule::ULE, Copy, Clone)]
pub struct FooULE {
    a: u8,
    b: <u32 as AsULE>::ULE,
    c: <char as AsULE>::ULE,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Foo {
    a: u8,
    b: u32,
    c: char,
}

impl AsULE for Foo {
    type ULE = FooULE;
    fn to_unaligned(self) -> FooULE {
        FooULE {
            a: self.a,
            b: self.b.to_unaligned(),
            c: self.c.to_unaligned(),
        }
    }

    fn from_unaligned(other: FooULE) -> Self {
        Self {
            a: other.a,
            b: AsULE::from_unaligned(other.b),
            c: AsULE::from_unaligned(other.c),
        }
    }
}

#[repr(C, packed)]
#[derive(ule::VarULE)]
pub struct RelationULE {
    /// This maps to (AndOr, Polarity, Operand),
    /// with the first bit mapping to AndOr (1 == And), the second bit
    /// to Polarity (1 == Positive), and the remaining bits to Operand
    /// encoded via Operand::encode. It is unsound for the Operand bits to
    /// not be a valid encoded Operand.
    andor_polarity_operand: u8,
    modulo: <u32 as AsULE>::ULE,
    range_list: ZeroSlice<Foo>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Relation<'a> {
    andor_polarity_operand: u8,
    modulo: u32,
    range_list: ZeroVec<'a, Foo>,
}

unsafe impl EncodeAsVarULE<RelationULE> for Relation<'_> {
    fn encode_var_ule_as_slices<R>(&self, cb: impl FnOnce(&[&[u8]]) -> R) -> R {
        cb(&[
            &[self.andor_polarity_operand],
            ule::ULE::slice_as_bytes(&[self.modulo.to_unaligned()]),
            self.range_list.as_bytes(),
        ])
    }
}

impl RelationULE {
    pub fn as_relation(&self) -> Relation {
        Relation {
            andor_polarity_operand: self.andor_polarity_operand,
            modulo: u32::from_unaligned(self.modulo),
            range_list: self.range_list.as_zerovec(),
        }
    }
}

const TEST_SLICE: &[Foo] = &[
    Foo {
        a: 101,
        b: 924,
        c: '⸘',
    },
    Foo {
        a: 217,
        b: 4228,
        c: 'ə',
    },
    Foo {
        a: 117,
        b: 9090,
        c: 'ø',
    },
];

const TEST_SLICE2: &[Foo] = &[
    Foo {
        a: 92,
        b: 4,
        c: 'å',
    },
    Foo {
        a: 9,
        b: 49993,
        c: '±',
    },
];
fn test_zerovec() {
    let zerovec: ZeroVec<Foo> = TEST_SLICE.iter().copied().collect();

    assert_eq!(zerovec, TEST_SLICE);

    let bytes = zerovec.as_bytes();
    let reparsed: ZeroVec<Foo> = ZeroVec::parse_bytes(bytes).expect("Parsing should succeed");

    assert_eq!(reparsed, TEST_SLICE);
}

fn test_varzerovec() {
    let relation1 = Relation {
        andor_polarity_operand: 1,
        modulo: 5004,
        range_list: TEST_SLICE.iter().copied().collect(),
    };
    let relation2 = Relation {
        andor_polarity_operand: 5,
        modulo: 909,
        range_list: TEST_SLICE2.iter().copied().collect(),
    };

    let relations = &[relation1, relation2];

    let vzv = VarZeroVec::<_>::from(relations);

    for (ule, stack) in vzv.iter().zip(relations.iter()) {
        assert_eq!(*stack, ule.as_relation());
    }

    let bytes = vzv.as_bytes();

    let recovered: VarZeroVec<RelationULE> =
        VarZeroVec::parse_bytes(bytes).expect("Parsing should succeed");

    for (ule, stack) in recovered.iter().zip(relations.iter()) {
        assert_eq!(*stack, ule.as_relation());
    }
}

fn main() {
    test_zerovec();
    test_varzerovec();
}
