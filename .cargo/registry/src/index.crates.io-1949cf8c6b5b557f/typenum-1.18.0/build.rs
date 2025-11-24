use std::{cmp, env, fmt, fs::File, io::Write, path::PathBuf};

enum UIntCode {
    Term,
    Zero(Box<UIntCode>),
    One(Box<UIntCode>),
}

enum IntCode {
    Zero,
    Pos(Box<UIntCode>),
    Neg(Box<UIntCode>),
}

impl fmt::Display for UIntCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            UIntCode::Term => write!(f, "UTerm"),
            UIntCode::Zero(ref inner) => write!(f, "UInt<{}, B0>", inner),
            UIntCode::One(ref inner) => write!(f, "UInt<{}, B1>", inner),
        }
    }
}

impl fmt::Display for IntCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            IntCode::Zero => write!(f, "Z0"),
            IntCode::Pos(ref inner) => write!(f, "PInt<{}>", inner),
            IntCode::Neg(ref inner) => write!(f, "NInt<{}>", inner),
        }
    }
}

fn gen_uint(u: u64) -> UIntCode {
    let mut result = UIntCode::Term;
    let mut x = 1u64 << 63;
    while x > u {
        x >>= 1
    }
    while x > 0 {
        result = if x & u > 0 {
            UIntCode::One(Box::new(result))
        } else {
            UIntCode::Zero(Box::new(result))
        };
        x >>= 1;
    }
    result
}

fn gen_int(i: i64) -> IntCode {
    use std::cmp::Ordering::{Equal, Greater, Less};

    match i.cmp(&0) {
        Greater => IntCode::Pos(Box::new(gen_uint(i as u64))),
        Less => IntCode::Neg(Box::new(gen_uint(i.abs() as u64))),
        Equal => IntCode::Zero,
    }
}

/// Computes the greatest common divisor of two integers.
fn gcdi(mut a: i64, mut b: i64) -> i64 {
    a = a.abs();
    b = b.abs();

    while a != 0 {
        let tmp = b % a;
        b = a;
        a = tmp;
    }

    b
}

fn gcdu(mut a: u64, mut b: u64) -> u64 {
    while a != 0 {
        let tmp = b % a;
        b = a;
        a = tmp;
    }

    b
}

fn sign(i: i64) -> char {
    use std::cmp::Ordering::*;
    match i.cmp(&0) {
        Greater => 'P',
        Less => 'N',
        Equal => '_',
    }
}

struct UIntTest {
    a: u64,
    op: &'static str,
    b: Option<u64>,
    r: u64,
}

impl fmt::Display for UIntTest {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.b {
            Some(b) => write!(
                f,
                "
#[test]
#[allow(non_snake_case)]
fn test_{a}_{op}_{b}() {{
    type A = {gen_a};
    type B = {gen_b};
    type U{r} = {result};

    #[allow(non_camel_case_types)]
    type U{a}{op}U{b} = <<A as {op}<B>>::Output as Same<U{r}>>::Output;

    assert_eq!(<U{a}{op}U{b} as Unsigned>::to_u64(), <U{r} as Unsigned>::to_u64());
}}",
                gen_a = gen_uint(self.a),
                gen_b = gen_uint(b),
                r = self.r,
                result = gen_uint(self.r),
                a = self.a,
                b = b,
                op = self.op
            ),
            None => write!(
                f,
                "
#[test]
#[allow(non_snake_case)]
fn test_{a}_{op}() {{
    type A = {gen_a};
    type U{r} = {result};

    #[allow(non_camel_case_types)]
    type {op}U{a} = <<A as {op}>::Output as Same<U{r}>>::Output;
    assert_eq!(<{op}U{a} as Unsigned>::to_u64(), <U{r} as Unsigned>::to_u64());
}}",
                gen_a = gen_uint(self.a),
                r = self.r,
                result = gen_uint(self.r),
                a = self.a,
                op = self.op
            ),
        }
    }
}

fn uint_binary_test(left: u64, operator: &'static str, right: u64, result: u64) -> UIntTest {
    UIntTest {
        a: left,
        op: operator,
        b: Option::Some(right),
        r: result,
    }
}

struct IntBinaryTest {
    a: i64,
    op: &'static str,
    b: i64,
    r: i64,
}

impl fmt::Display for IntBinaryTest {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "
#[test]
#[allow(non_snake_case)]
fn test_{sa}{a}_{op}_{sb}{b}() {{
    type A = {gen_a};
    type B = {gen_b};
    type {sr}{r} = {result};

    #[allow(non_camel_case_types)]
    type {sa}{a}{op}{sb}{b} = <<A as {op}<B>>::Output as Same<{sr}{r}>>::Output;

    assert_eq!(<{sa}{a}{op}{sb}{b} as Integer>::to_i64(), <{sr}{r} as Integer>::to_i64());
}}",
            gen_a = gen_int(self.a),
            gen_b = gen_int(self.b),
            r = self.r.abs(),
            sr = sign(self.r),
            result = gen_int(self.r),
            a = self.a.abs(),
            b = self.b.abs(),
            sa = sign(self.a),
            sb = sign(self.b),
            op = self.op
        )
    }
}

fn int_binary_test(left: i64, operator: &'static str, right: i64, result: i64) -> IntBinaryTest {
    IntBinaryTest {
        a: left,
        op: operator,
        b: right,
        r: result,
    }
}

struct IntUnaryTest {
    op: &'static str,
    a: i64,
    r: i64,
}

impl fmt::Display for IntUnaryTest {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "
#[test]
#[allow(non_snake_case)]
fn test_{sa}{a}_{op}() {{
    type A = {gen_a};
    type {sr}{r} = {result};

    #[allow(non_camel_case_types)]
    type {op}{sa}{a} = <<A as {op}>::Output as Same<{sr}{r}>>::Output;
    assert_eq!(<{op}{sa}{a} as Integer>::to_i64(), <{sr}{r} as Integer>::to_i64());
}}",
            gen_a = gen_int(self.a),
            r = self.r.abs(),
            sr = sign(self.r),
            result = gen_int(self.r),
            a = self.a.abs(),
            sa = sign(self.a),
            op = self.op
        )
    }
}

fn int_unary_test(operator: &'static str, num: i64, result: i64) -> IntUnaryTest {
    IntUnaryTest {
        op: operator,
        a: num,
        r: result,
    }
}

fn uint_cmp_test(a: u64, b: u64) -> String {
    format!(
        "
#[test]
#[allow(non_snake_case)]
fn test_{a}_Cmp_{b}() {{
    type A = {gen_a};
    type B = {gen_b};

    #[allow(non_camel_case_types)]
    type U{a}CmpU{b} = <A as Cmp<B>>::Output;
    assert_eq!(<U{a}CmpU{b} as Ord>::to_ordering(), Ordering::{result:?});
}}",
        a = a,
        b = b,
        gen_a = gen_uint(a),
        gen_b = gen_uint(b),
        result = a.cmp(&b)
    )
}

fn int_cmp_test(a: i64, b: i64) -> String {
    format!(
        "
#[test]
#[allow(non_snake_case)]
fn test_{sa}{a}_Cmp_{sb}{b}() {{
    type A = {gen_a};
    type B = {gen_b};

    #[allow(non_camel_case_types)]
    type {sa}{a}Cmp{sb}{b} = <A as Cmp<B>>::Output;
    assert_eq!(<{sa}{a}Cmp{sb}{b} as Ord>::to_ordering(), Ordering::{result:?});
}}",
        a = a.abs(),
        b = b.abs(),
        sa = sign(a),
        sb = sign(b),
        gen_a = gen_int(a),
        gen_b = gen_int(b),
        result = a.cmp(&b)
    )
}
pub fn gen_tests() -> String {
    // will test all permutations of number pairs up to this (and down to its opposite for ints)
    let high: i64 = 5;

    let uints = (0u64..high as u64 + 1).flat_map(|a| (a..a + 1).cycle().zip(0..high as u64 + 1));
    let ints = (-high..high + 1).flat_map(|a| (a..a + 1).cycle().zip(-high..high + 1));

    let mut result = String::new();

    result.push_str(
        "
use typenum::*;
use core::ops::*;
use core::cmp::Ordering;
",
    );
    // uint operators:
    for (a, b) in uints {
        let mut tests = vec![
            uint_binary_test(a, "BitAnd", b, a & b),
            uint_binary_test(a, "BitOr", b, a | b),
            uint_binary_test(a, "BitXor", b, a ^ b),
            uint_binary_test(a, "Shl", b, a << b),
            uint_binary_test(a, "Shr", b, a >> b),
            uint_binary_test(a, "Add", b, a + b),
            uint_binary_test(a, "Mul", b, a * b),
            uint_binary_test(a, "Pow", b, a.pow(b as u32)),
            uint_binary_test(a, "Min", b, cmp::min(a, b)),
            uint_binary_test(a, "Max", b, cmp::max(a, b)),
            uint_binary_test(a, "Gcd", b, gcdu(a, b)),
        ];
        if a >= b {
            tests.push(uint_binary_test(a, "Sub", b, a - b));
        }
        if b != 0 {
            tests.push(uint_binary_test(a, "Div", b, a / b));
            tests.push(uint_binary_test(a, "Rem", b, a % b));
            if a % b == 0 {
                tests.push(uint_binary_test(a, "PartialDiv", b, a / b));
            }
        }

        for test in tests {
            result.push_str(&test.to_string());
        }
        result.push_str(&uint_cmp_test(a, b));
    }

    // int operators:
    for (a, b) in ints {
        let mut tests = vec![
            int_binary_test(a, "Add", b, a + b),
            int_binary_test(a, "Sub", b, a - b),
            int_binary_test(a, "Mul", b, a * b),
            int_binary_test(a, "Min", b, cmp::min(a, b)),
            int_binary_test(a, "Max", b, cmp::max(a, b)),
            int_binary_test(a, "Gcd", b, gcdi(a, b)),
        ];
        if b != 0 {
            tests.push(int_binary_test(a, "Div", b, a / b));
            tests.push(int_binary_test(a, "Rem", b, a % b));
            if a % b == 0 {
                tests.push(int_binary_test(a, "PartialDiv", b, a / b));
            }
        }
        if b >= 0 || a.abs() == 1 {
            let result = if b < 0 {
                if a == 1 {
                    a
                } else if a == -1 {
                    a.pow((-b) as u32)
                } else {
                    unreachable!()
                }
            } else {
                a.pow(b as u32)
            };
            tests.push(int_binary_test(a, "Pow", b, result));
        }
        for test in tests {
            result.push_str(&test.to_string());
        }
        result.push_str(&int_cmp_test(a, b));
    }

    // int unary operators:
    for n in -high..high + 1 {
        let tests = vec![
            int_unary_test("Neg", n, -n),
            int_unary_test("Abs", n, n.abs()),
        ];
        for test in tests {
            result.push_str(&test.to_string());
        }
    }

    result
}

#[cfg_attr(
    feature = "no_std",
    deprecated(
        since = "1.3.0",
        note = "the `no_std` flag is no longer necessary and will be removed in the future"
    )
)]
pub fn no_std() {}

#[cfg_attr(
    feature = "force_unix_path_separator",
    deprecated(
        since = "1.17.0",
        note = "the `force_unix_path_separator` flag is no longer necessary and will be removed in the future"
    )
)]
pub fn force_unix_path_separator() {}

fn main() {
    no_std();
    force_unix_path_separator();
    println!("cargo:rerun-if-changed=tests");

    let tests = gen_tests();
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest = PathBuf::from(out_dir).join("tests.rs");
    let mut f = File::create(&dest).unwrap();
    f.write_all(tests.as_bytes()).unwrap();
}
