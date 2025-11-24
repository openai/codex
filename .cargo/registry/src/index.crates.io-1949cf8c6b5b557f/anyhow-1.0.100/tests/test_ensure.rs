#![allow(
    clippy::bool_to_int_with_if,
    clippy::char_lit_as_u8,
    clippy::deref_addrof,
    clippy::diverging_sub_expression,
    clippy::erasing_op,
    clippy::extra_unused_type_parameters,
    clippy::if_same_then_else,
    clippy::ifs_same_cond,
    clippy::ignored_unit_patterns,
    clippy::items_after_statements,
    clippy::let_and_return,
    clippy::let_underscore_untyped,
    clippy::literal_string_with_formatting_args,
    clippy::match_bool,
    clippy::needless_else,
    clippy::never_loop,
    clippy::overly_complex_bool_expr,
    clippy::redundant_closure_call,
    clippy::redundant_pattern_matching,
    clippy::too_many_lines,
    clippy::unit_arg,
    clippy::unnecessary_cast,
    clippy::while_immutable_condition,
    clippy::zero_ptr,
    irrefutable_let_patterns
)]

use self::Enum::Generic;
use anyhow::{anyhow, ensure, Chain, Error, Result};
use std::fmt::{self, Debug};
use std::iter;
use std::marker::{PhantomData, PhantomData as P};
use std::mem;
use std::ops::Add;
use std::ptr;

struct S;

impl<T> Add<T> for S {
    type Output = bool;
    fn add(self, rhs: T) -> Self::Output {
        let _ = rhs;
        false
    }
}

trait Trait: Sized {
    const V: usize = 0;
    fn t(self, i: i32) -> i32 {
        i
    }
}

impl<T> Trait for T {}

enum Enum<T: ?Sized> {
    #[allow(dead_code)]
    Thing(PhantomData<T>),
    Generic,
}

impl<T: ?Sized> PartialEq for Enum<T> {
    fn eq(&self, rhs: &Self) -> bool {
        mem::discriminant(self) == mem::discriminant(rhs)
    }
}

impl<T: ?Sized> Debug for Enum<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("Generic")
    }
}

#[track_caller]
fn assert_err<T: Debug>(result: impl FnOnce() -> Result<T>, expected: &'static str) {
    let actual = result().unwrap_err().to_string();

    // In general different rustc versions will format the interpolated lhs and
    // rhs $:expr fragment with insignificant differences in whitespace or
    // punctuation, so we check the message in full against nightly and do just
    // a cursory test on older toolchains.
    if rustversion::cfg!(nightly) && !cfg!(miri) {
        assert_eq!(actual, expected);
    } else {
        assert_eq!(actual.contains(" vs "), expected.contains(" vs "));
    }
}

#[test]
fn test_recursion() {
    // Must not blow the default #[recursion_limit], which is 128.
    #[rustfmt::skip]
    let test = || Ok(ensure!(
        false | false | false | false | false | false | false | false | false |
        false | false | false | false | false | false | false | false | false |
        false | false | false | false | false | false | false | false | false |
        false | false | false | false | false | false | false | false | false |
        false | false | false | false | false | false | false | false | false |
        false | false | false | false | false | false | false | false | false |
        false | false | false | false | false | false | false | false | false
    ));

    test().unwrap_err();
}

#[test]
fn test_low_precedence_control_flow() {
    #[allow(unreachable_code)]
    let test = || {
        let val = loop {
            // Break has lower precedence than the comparison operators so the
            // expression here is `S + (break (1 == 1))`. It would be bad if the
            // ensure macro partitioned this input into `(S + break 1) == (1)`
            // because that means a different thing than what was written.
            ensure!(S + break 1 == 1);
        };
        Ok(val)
    };

    assert!(test().unwrap());
}

#[test]
fn test_low_precedence_binary_operator() {
    // Must not partition as `false == (true && false)`.
    let test = || Ok(ensure!(false == true && false));
    assert_err(test, "Condition failed: `false == true && false`");

    // But outside the root level, it is fine.
    let test = || Ok(ensure!(while false == true && false {} < ()));
    assert_err(
        test,
        "Condition failed: `while false == true && false {} < ()` (() vs ())",
    );

    let a = 15;
    let b = 3;
    let test = || Ok(ensure!(a <= b || a - b <= 10));
    assert_err(test, "Condition failed: `a <= b || a - b <= 10`");
}

#[test]
fn test_high_precedence_binary_operator() {
    let a = 15;
    let b = 3;
    let test = || Ok(ensure!(a - b <= 10));
    assert_err(test, "Condition failed: `a - b <= 10` (12 vs 10)");
}

#[test]
fn test_closure() {
    // Must not partition as `(S + move) || (1 == 1)` by treating move as an
    // identifier, nor as `(S + move || 1) == (1)` by misinterpreting the
    // closure precedence.
    let test = || Ok(ensure!(S + move || 1 == 1));
    assert_err(test, "Condition failed: `S + move || 1 == 1`");

    let test = || Ok(ensure!(S + || 1 == 1));
    assert_err(test, "Condition failed: `S + || 1 == 1`");

    // Must not partition as `S + ((move | ()) | 1) == 1` by treating those
    // pipes as bitwise-or.
    let test = || Ok(ensure!(S + move |()| 1 == 1));
    assert_err(test, "Condition failed: `S + move |()| 1 == 1`");

    let test = || Ok(ensure!(S + |()| 1 == 1));
    assert_err(test, "Condition failed: `S + |()| 1 == 1`");
}

#[test]
fn test_unary() {
    let mut x = &1;
    let test = || Ok(ensure!(*x == 2));
    assert_err(test, "Condition failed: `*x == 2` (1 vs 2)");

    let test = || Ok(ensure!(!x == 1));
    assert_err(test, "Condition failed: `!x == 1` (-2 vs 1)");

    let test = || Ok(ensure!(-x == 1));
    assert_err(test, "Condition failed: `-x == 1` (-1 vs 1)");

    let test = || Ok(ensure!(&x == &&2));
    assert_err(test, "Condition failed: `&x == &&2` (1 vs 2)");

    let test = || Ok(ensure!(&mut x == *&&mut &2));
    assert_err(test, "Condition failed: `&mut x == *&&mut &2` (1 vs 2)");
}

#[rustversion::since(1.82)]
#[test]
fn test_raw_addr() {
    let mut x = 1;
    let test = || Ok(ensure!(S + &raw const x != S + &raw mut x));
    assert_err(
        test,
        "Condition failed: `S + &raw const x != S + &raw mut x` (false vs false)",
    );
}

#[test]
fn test_if() {
    #[rustfmt::skip]
    let test = || Ok(ensure!(if false {}.t(1) == 2));
    assert_err(test, "Condition failed: `if false {}.t(1) == 2` (1 vs 2)");

    #[rustfmt::skip]
    let test = || Ok(ensure!(if false {} else {}.t(1) == 2));
    assert_err(
        test,
        "Condition failed: `if false {} else {}.t(1) == 2` (1 vs 2)",
    );

    #[rustfmt::skip]
    let test = || Ok(ensure!(if false {} else if false {}.t(1) == 2));
    assert_err(
        test,
        "Condition failed: `if false {} else if false {}.t(1) == 2` (1 vs 2)",
    );

    #[rustfmt::skip]
    let test = || Ok(ensure!(if let 1 = 2 {}.t(1) == 2));
    assert_err(
        test,
        "Condition failed: `if let 1 = 2 {}.t(1) == 2` (1 vs 2)",
    );

    #[rustfmt::skip]
    let test = || Ok(ensure!(if let 1 | 2 = 2 {}.t(1) == 2));
    assert_err(
        test,
        "Condition failed: `if let 1 | 2 = 2 {}.t(1) == 2` (1 vs 2)",
    );

    #[rustfmt::skip]
    let test = || Ok(ensure!(if let | 1 | 2 = 2 {}.t(1) == 2));
    assert_err(
        test,
        "Condition failed: `if let | 1 | 2 = 2 {}.t(1) == 2` (1 vs 2)",
    );
}

#[test]
fn test_loop() {
    #[rustfmt::skip]
    let test = || Ok(ensure!(1 + loop { break 1 } == 1));
    assert_err(
        test,
        "Condition failed: `1 + loop { break 1 } == 1` (2 vs 1)",
    );

    #[rustfmt::skip]
    let test = || Ok(ensure!(1 + 'a: loop { break 'a 1 } == 1));
    assert_err(
        test,
        "Condition failed: `1 + 'a: loop { break 'a 1 } == 1` (2 vs 1)",
    );

    #[rustfmt::skip]
    let test = || Ok(ensure!(while false {}.t(1) == 2));
    assert_err(
        test,
        "Condition failed: `while false {}.t(1) == 2` (1 vs 2)",
    );

    #[rustfmt::skip]
    let test = || Ok(ensure!(while let None = Some(1) {}.t(1) == 2));
    assert_err(
        test,
        "Condition failed: `while let None = Some(1) {}.t(1) == 2` (1 vs 2)",
    );

    #[rustfmt::skip]
    let test = || Ok(ensure!(for _x in iter::once(0) {}.t(1) == 2));
    assert_err(
        test,
        "Condition failed: `for _x in iter::once(0) {}.t(1) == 2` (1 vs 2)",
    );

    #[rustfmt::skip]
    let test = || Ok(ensure!(for | _x in iter::once(0) {}.t(1) == 2));
    assert_err(
        test,
        "Condition failed: `for | _x in iter::once(0) {}.t(1) == 2` (1 vs 2)",
    );

    #[rustfmt::skip]
    let test = || Ok(ensure!(for true | false in iter::empty() {}.t(1) == 2));
    assert_err(
        test,
        "Condition failed: `for true | false in iter::empty() {}.t(1) == 2` (1 vs 2)",
    );
}

#[test]
fn test_match() {
    #[rustfmt::skip]
    let test = || Ok(ensure!(match 1 == 1 { true => 1, false => 0 } == 2));
    assert_err(
        test,
        "Condition failed: `match 1 == 1 { true => 1, false => 0 } == 2` (1 vs 2)",
    );
}

#[test]
fn test_atom() {
    let test = || Ok(ensure!([false, false].len() > 3));
    assert_err(
        test,
        "Condition failed: `[false, false].len() > 3` (2 vs 3)",
    );

    #[rustfmt::skip]
    let test = || Ok(ensure!({ let x = 1; x } >= 3));
    assert_err(test, "Condition failed: `{ let x = 1; x } >= 3` (1 vs 3)");

    let test = || Ok(ensure!(S + async { 1 } == true));
    assert_err(
        test,
        "Condition failed: `S + async { 1 } == true` (false vs true)",
    );

    let test = || Ok(ensure!(S + async move { 1 } == true));
    assert_err(
        test,
        "Condition failed: `S + async move { 1 } == true` (false vs true)",
    );

    let x = &1;
    let test = || Ok(ensure!(S + unsafe { ptr::read(x) } == true));
    assert_err(
        test,
        "Condition failed: `S + unsafe { ptr::read(x) } == true` (false vs true)",
    );
}

#[test]
fn test_path() {
    let test = || Ok(ensure!(crate::S.t(1) == 2));
    assert_err(test, "Condition failed: `crate::S.t(1) == 2` (1 vs 2)");

    let test = || Ok(ensure!(::anyhow::Error::root_cause.t(1) == 2));
    assert_err(
        test,
        "Condition failed: `::anyhow::Error::root_cause.t(1) == 2` (1 vs 2)",
    );

    let test = || Ok(ensure!(Error::msg::<&str>.t(1) == 2));
    assert_err(
        test,
        "Condition failed: `Error::msg::<&str>.t(1) == 2` (1 vs 2)",
    );

    #[rustfmt::skip]
    let test = || Ok(ensure!(Error::msg::<&str,>.t(1) == 2));
    assert_err(
        test,
        "Condition failed: `Error::msg::<&str,>.t(1) == 2` (1 vs 2)",
    );

    let test = || Ok(ensure!(Error::msg::<<str as ToOwned>::Owned>.t(1) == 2));
    assert_err(
        test,
        "Condition failed: `Error::msg::<<str as ToOwned>::Owned>.t(1) == 2` (1 vs 2)",
    );

    let test = || Ok(ensure!(Chain::<'static>::new.t(1) == 2));
    assert_err(
        test,
        "Condition failed: `Chain::<'static>::new.t(1) == 2` (1 vs 2)",
    );

    #[rustfmt::skip]
    let test = || Ok(ensure!(Chain::<'static,>::new.t(1) == 2));
    assert_err(
        test,
        "Condition failed: `Chain::<'static,>::new.t(1) == 2` (1 vs 2)",
    );

    fn f<const I: isize>() {}
    let test = || Ok(ensure!(f::<1>() != ()));
    assert_err(test, "Condition failed: `f::<1>() != ()` (() vs ())");
    let test = || Ok(ensure!(f::<-1>() != ()));
    assert_err(test, "Condition failed: `f::<-1>() != ()` (() vs ())");

    fn g<T, const I: isize>() {}
    let test = || Ok(ensure!(g::<u8, 1>() != ()));
    assert_err(test, "Condition failed: `g::<u8, 1>() != ()` (() vs ())");
    let test = || Ok(ensure!(g::<u8, -1>() != ()));
    assert_err(test, "Condition failed: `g::<u8, -1>() != ()` (() vs ())");

    #[derive(PartialOrd, PartialEq, Debug)]
    enum E<'a, T> {
        #[allow(dead_code)]
        T(&'a T),
        U,
    }

    #[rustfmt::skip]
    let test = || Ok(ensure!(E::U::<>>E::U::<u8>));
    assert_err(test, "Condition failed: `E::U::<> > E::U::<u8>` (U vs U)");

    #[rustfmt::skip]
    let test = || Ok(ensure!(E::U::<u8>>E::U));
    assert_err(test, "Condition failed: `E::U::<u8> > E::U` (U vs U)");

    #[rustfmt::skip]
    let test = || Ok(ensure!(E::U::<u8,>>E::U));
    assert_err(test, "Condition failed: `E::U::<u8,> > E::U` (U vs U)");

    let test = || Ok(ensure!(Generic::<dyn Debug + Sync> != Generic));
    assert_err(
        test,
        "Condition failed: `Generic::<dyn Debug + Sync> != Generic` (Generic vs Generic)",
    );

    let test = || Ok(ensure!(Generic::<dyn Fn() + Sync> != Generic));
    assert_err(
        test,
        "Condition failed: `Generic::<dyn Fn() + Sync> != Generic` (Generic vs Generic)",
    );

    #[rustfmt::skip]
    let test = || {
        Ok(ensure!(
            Generic::<dyn Fn::() + ::std::marker::Sync> != Generic
        ))
    };
    assert_err(
        test,
        "Condition failed: `Generic::<dyn Fn::() + ::std::marker::Sync> != Generic` (Generic vs Generic)",
    );
}

#[test]
fn test_macro() {
    let test = || Ok(ensure!(anyhow!("...").to_string().len() <= 1));
    assert_err(
        test,
        "Condition failed: `anyhow!(\"...\").to_string().len() <= 1` (3 vs 1)",
    );

    let test = || Ok(ensure!(vec![1].len() < 1));
    assert_err(test, "Condition failed: `vec![1].len() < 1` (1 vs 1)");

    let test = || Ok(ensure!(stringify! {} != ""));
    assert_err(
        test,
        "Condition failed: `stringify! {} != \"\"` (\"\" vs \"\")",
    );
}

#[test]
fn test_trailer() {
    let test = || Ok(ensure!((|| 1)() == 2));
    assert_err(test, "Condition failed: `(|| 1)() == 2` (1 vs 2)");

    let test = || Ok(ensure!(b"hmm"[1] == b'c'));
    assert_err(test, "Condition failed: `b\"hmm\"[1] == b'c'` (109 vs 99)");

    let test = || Ok(ensure!(PhantomData::<u8> {} != PhantomData));
    assert_err(
        test,
        "Condition failed: `PhantomData::<u8> {} != PhantomData` (PhantomData<u8> vs PhantomData<u8>)",
    );

    let result = Ok::<_, Error>(1);
    let test = || Ok(ensure!(result? == 2));
    assert_err(test, "Condition failed: `result? == 2` (1 vs 2)");

    let test = || Ok(ensure!((2, 3).1 == 2));
    assert_err(test, "Condition failed: `(2, 3).1 == 2` (3 vs 2)");

    #[rustfmt::skip]
    let test = || Ok(ensure!((2, (3, 4)). 1.1 == 2));
    assert_err(test, "Condition failed: `(2, (3, 4)).1.1 == 2` (4 vs 2)");

    let err = anyhow!("");
    let test = || Ok(ensure!(err.is::<&str>() == false));
    assert_err(
        test,
        "Condition failed: `err.is::<&str>() == false` (true vs false)",
    );

    let test = || Ok(ensure!(err.is::<<str as ToOwned>::Owned>() == true));
    assert_err(
        test,
        "Condition failed: `err.is::<<str as ToOwned>::Owned>() == true` (false vs true)",
    );
}

#[test]
fn test_whitespace() {
    #[derive(Debug)]
    pub struct Point {
        #[allow(dead_code)]
        pub x: i32,
        #[allow(dead_code)]
        pub y: i32,
    }

    let point = Point { x: 0, y: 0 };
    let test = || Ok(ensure!("" == format!("{:#?}", point)));
    assert_err(
        test,
        "Condition failed: `\"\" == format!(\"{:#?}\", point)`",
    );
}

#[test]
fn test_too_long() {
    let test = || Ok(ensure!("" == "x".repeat(10)));
    assert_err(
        test,
        "Condition failed: `\"\" == \"x\".repeat(10)` (\"\" vs \"xxxxxxxxxx\")",
    );

    let test = || Ok(ensure!("" == "x".repeat(80)));
    assert_err(test, "Condition failed: `\"\" == \"x\".repeat(80)`");
}

#[test]
fn test_as() {
    let test = || Ok(ensure!('\0' as u8 > 1));
    assert_err(test, "Condition failed: `'\\0' as u8 > 1` (0 vs 1)");

    let test = || Ok(ensure!('\0' as ::std::primitive::u8 > 1));
    assert_err(
        test,
        "Condition failed: `'\\0' as ::std::primitive::u8 > 1` (0 vs 1)",
    );

    let test = || Ok(ensure!(&[0] as &[i32] == [1]));
    assert_err(
        test,
        "Condition failed: `&[0] as &[i32] == [1]` ([0] vs [1])",
    );

    let test = || Ok(ensure!(0 as *const () as *mut _ == 1 as *mut ()));
    assert_err(
        test,
        "Condition failed: `0 as *const () as *mut _ == 1 as *mut ()` (0x0 vs 0x1)",
    );

    let s = "";
    let test = || Ok(ensure!(s as &str != s));
    assert_err(test, "Condition failed: `s as &str != s` (\"\" vs \"\")");

    let test = || Ok(ensure!(&s as &&str != &s));
    assert_err(test, "Condition failed: `&s as &&str != &s` (\"\" vs \"\")");

    let test = || Ok(ensure!(s as &'static str != s));
    assert_err(
        test,
        "Condition failed: `s as &'static str != s` (\"\" vs \"\")",
    );

    let test = || Ok(ensure!(&s as &&'static str != &s));
    assert_err(
        test,
        "Condition failed: `&s as &&'static str != &s` (\"\" vs \"\")",
    );

    let m: &mut str = Default::default();
    let test = || Ok(ensure!(m as &mut str != s));
    assert_err(
        test,
        "Condition failed: `m as &mut str != s` (\"\" vs \"\")",
    );

    let test = || Ok(ensure!(&m as &&mut str != &s));
    assert_err(
        test,
        "Condition failed: `&m as &&mut str != &s` (\"\" vs \"\")",
    );

    let test = || Ok(ensure!(&m as &&'static mut str != &s));
    assert_err(
        test,
        "Condition failed: `&m as &&'static mut str != &s` (\"\" vs \"\")",
    );

    let f = || {};
    let test = || Ok(ensure!(f as fn() as usize * 0 != 0));
    assert_err(
        test,
        "Condition failed: `f as fn() as usize * 0 != 0` (0 vs 0)",
    );

    let test = || Ok(ensure!(f as fn() -> () as usize * 0 != 0));
    assert_err(
        test,
        "Condition failed: `f as fn() -> () as usize * 0 != 0` (0 vs 0)",
    );

    let test = || Ok(ensure!(f as for<'a> fn() as usize * 0 != 0));
    assert_err(
        test,
        "Condition failed: `f as for<'a> fn() as usize * 0 != 0` (0 vs 0)",
    );

    let test = || Ok(ensure!(f as unsafe fn() as usize * 0 != 0));
    assert_err(
        test,
        "Condition failed: `f as unsafe fn() as usize * 0 != 0` (0 vs 0)",
    );

    #[rustfmt::skip]
    let test = || Ok(ensure!(f as extern "Rust" fn() as usize * 0 != 0));
    assert_err(
        test,
        "Condition failed: `f as extern \"Rust\" fn() as usize * 0 != 0` (0 vs 0)",
    );

    extern "C" fn extern_fn() {}
    #[rustfmt::skip]
    #[allow(missing_abi)]
    let test = || Ok(ensure!(extern_fn as extern fn() as usize * 0 != 0));
    assert_err(
        test,
        "Condition failed: `extern_fn as extern fn() as usize * 0 != 0` (0 vs 0)",
    );

    let f = || -> ! { panic!() };
    let test = || Ok(ensure!(f as fn() -> ! as usize * 0 != 0));
    assert_err(
        test,
        "Condition failed: `f as fn() -> ! as usize * 0 != 0` (0 vs 0)",
    );

    trait EqDebug<T>: PartialEq<T> + Debug {
        type Assoc;
    }

    impl<S, T> EqDebug<T> for S
    where
        S: PartialEq<T> + Debug,
    {
        type Assoc = bool;
    }

    let test = || Ok(ensure!(&0 as &dyn EqDebug<i32, Assoc = bool> != &0));
    assert_err(
        test,
        "Condition failed: `&0 as &dyn EqDebug<i32, Assoc = bool> != &0` (0 vs 0)",
    );

    let test = || {
        Ok(ensure!(
            PhantomData as PhantomData<<i32 as ToOwned>::Owned> != PhantomData
        ))
    };
    assert_err(
        test,
        "Condition failed: `PhantomData as PhantomData<<i32 as ToOwned>::Owned> != PhantomData` (PhantomData<i32> vs PhantomData<i32>)",
    );

    macro_rules! int {
        (...) => {
            u8
        };
    }

    let test = || Ok(ensure!(0 as int!(...) != 0));
    assert_err(test, "Condition failed: `0 as int!(...) != 0` (0 vs 0)");

    let test = || Ok(ensure!(0 as int![...] != 0));
    assert_err(test, "Condition failed: `0 as int![...] != 0` (0 vs 0)");

    let test = || Ok(ensure!(0 as int! {...} != 0));
    assert_err(test, "Condition failed: `0 as int! {...} != 0` (0 vs 0)");
}

#[test]
fn test_pat() {
    let test = || Ok(ensure!(if let ref mut _x @ 0 = 0 { 0 } else { 1 } == 1));
    assert_err(
        test,
        "Condition failed: `if let ref mut _x @ 0 = 0 { 0 } else { 1 } == 1` (0 vs 1)",
    );

    let test = || Ok(ensure!(if let -1..=1 = 0 { 0 } else { 1 } == 1));
    assert_err(
        test,
        "Condition failed: `if let -1..=1 = 0 { 0 } else { 1 } == 1` (0 vs 1)",
    );

    let test = || Ok(ensure!(if let &0 = &0 { 0 } else { 1 } == 1));
    assert_err(
        test,
        "Condition failed: `if let &0 = &0 { 0 } else { 1 } == 1` (0 vs 1)",
    );

    let test = || Ok(ensure!(if let &&0 = &&0 { 0 } else { 1 } == 1));
    assert_err(
        test,
        "Condition failed: `if let &&0 = &&0 { 0 } else { 1 } == 1` (0 vs 1)",
    );

    let test = || Ok(ensure!(if let &mut 0 = &mut 0 { 0 } else { 1 } == 1));
    assert_err(
        test,
        "Condition failed: `if let &mut 0 = &mut 0 { 0 } else { 1 } == 1` (0 vs 1)",
    );

    let test = || Ok(ensure!(if let &&mut 0 = &&mut 0 { 0 } else { 1 } == 1));
    assert_err(
        test,
        "Condition failed: `if let &&mut 0 = &&mut 0 { 0 } else { 1 } == 1` (0 vs 1)",
    );

    let test = || Ok(ensure!(if let (0, 1) = (0, 1) { 0 } else { 1 } == 1));
    assert_err(
        test,
        "Condition failed: `if let (0, 1) = (0, 1) { 0 } else { 1 } == 1` (0 vs 1)",
    );

    let test = || Ok(ensure!(if let [0] = b"\0" { 0 } else { 1 } == 1));
    assert_err(
        test,
        "Condition failed: `if let [0] = b\"\\0\" { 0 } else { 1 } == 1` (0 vs 1)",
    );

    let p = PhantomData::<u8>;
    let test = || Ok(ensure!(if let P::<u8> {} = p { 0 } else { 1 } == 1));
    assert_err(
        test,
        "Condition failed: `if let P::<u8> {} = p { 0 } else { 1 } == 1` (0 vs 1)",
    );

    let test = || Ok(ensure!(if let ::std::marker::PhantomData = p {} != ()));
    assert_err(
        test,
        "Condition failed: `if let ::std::marker::PhantomData = p {} != ()` (() vs ())",
    );

    let test = || Ok(ensure!(if let <S as Trait>::V = 0 { 0 } else { 1 } == 1));
    assert_err(
        test,
        "Condition failed: `if let <S as Trait>::V = 0 { 0 } else { 1 } == 1` (0 vs 1)",
    );

    let test = || Ok(ensure!(for _ in iter::once(()) {} != ()));
    assert_err(
        test,
        "Condition failed: `for _ in iter::once(()) {} != ()` (() vs ())",
    );

    let test = || Ok(ensure!(if let stringify!(x) = "x" { 0 } else { 1 } == 1));
    assert_err(
        test,
        "Condition failed: `if let stringify!(x) = \"x\" { 0 } else { 1 } == 1` (0 vs 1)",
    );
}
