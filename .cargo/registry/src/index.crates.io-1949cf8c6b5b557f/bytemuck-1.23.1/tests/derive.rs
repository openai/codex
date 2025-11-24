#![cfg(feature = "derive")]
#![allow(dead_code)]

use bytemuck::{ByteEq, ByteHash, Pod, TransparentWrapper, Zeroable};
use std::marker::PhantomData;

#[derive(Copy, Clone, Pod, Zeroable, ByteEq, ByteHash)]
#[repr(C)]
struct Test {
  a: u16,
  b: u16,
}

#[derive(TransparentWrapper)]
#[repr(transparent)]
struct TransparentSingle {
  a: u16,
}

#[derive(TransparentWrapper)]
#[repr(transparent)]
#[transparent(u16)]
struct TransparentWithZeroSized {
  a: u16,
  b: (),
}

#[derive(TransparentWrapper)]
#[repr(transparent)]
struct TransparentWithGeneric<T: ?Sized> {
  a: T,
}

/// Ensuring that no additional bounds are emitted.
/// See https://github.com/Lokathor/bytemuck/issues/145
fn test_generic<T>(x: T) -> TransparentWithGeneric<T> {
  TransparentWithGeneric::wrap(x)
}

#[derive(TransparentWrapper)]
#[repr(transparent)]
#[transparent(T)]
struct TransparentWithGenericAndZeroSized<T: ?Sized> {
  a: (),
  b: T,
}

/// Ensuring that no additional bounds are emitted.
/// See https://github.com/Lokathor/bytemuck/issues/145
fn test_generic_with_zst<T>(x: T) -> TransparentWithGenericAndZeroSized<T> {
  TransparentWithGenericAndZeroSized::wrap(x)
}

#[derive(TransparentWrapper)]
#[repr(transparent)]
struct TransparentUnsized {
  a: dyn std::fmt::Debug,
}

type DynDebug = dyn std::fmt::Debug;

#[derive(TransparentWrapper)]
#[repr(transparent)]
#[transparent(DynDebug)]
struct TransparentUnsizedWithZeroSized {
  a: (),
  b: DynDebug,
}

#[derive(TransparentWrapper)]
#[repr(transparent)]
#[transparent(DynDebug)]
struct TransparentUnsizedWithGenericZeroSizeds<T: ?Sized, U: ?Sized> {
  a: PhantomData<T>,
  b: PhantomData<U>,
  c: DynDebug,
}
