// THIS IS GENERATED CODE
/**
Convenient type operations.

Any types representing values must be able to be expressed as `ident`s. That means they need to be
in scope.

For example, `P5` is okay, but `typenum::P5` is not.

You may combine operators arbitrarily, although doing so excessively may require raising the
recursion limit.

# Example
```rust
#![recursion_limit="128"]
#[macro_use] extern crate typenum;
use typenum::consts::*;

fn main() {
    assert_type!(
        op!(min((P1 - P2) * (N3 + N7), P5 * (P3 + P4)) == P10)
    );
}
```
Operators are evaluated based on the operator precedence outlined
[here](https://doc.rust-lang.org/reference.html#operator-precedence).

The full list of supported operators and functions is as follows:

`*`, `/`, `%`, `+`, `-`, `<<`, `>>`, `&`, `^`, `|`, `==`, `!=`, `<=`, `>=`, `<`, `>`, `cmp`, `sqr`, `sqrt`, `abs`, `cube`, `pow`, `min`, `max`, `log2`, `gcd`

They all expand to type aliases defined in the `operator_aliases` module. Here is an expanded list,
including examples:

---
Operator `*`. Expands to `Prod`.

```rust
# #[macro_use] extern crate typenum;
# use typenum::*;
# fn main() {
assert_type_eq!(op!(P2 * P3), P6);
# }
```

---
Operator `/`. Expands to `Quot`.

```rust
# #[macro_use] extern crate typenum;
# use typenum::*;
# fn main() {
assert_type_eq!(op!(P6 / P2), P3);
# }
```

---
Operator `%`. Expands to `Mod`.

```rust
# #[macro_use] extern crate typenum;
# use typenum::*;
# fn main() {
assert_type_eq!(op!(P5 % P3), P2);
# }
```

---
Operator `+`. Expands to `Sum`.

```rust
# #[macro_use] extern crate typenum;
# use typenum::*;
# fn main() {
assert_type_eq!(op!(P2 + P3), P5);
# }
```

---
Operator `-`. Expands to `Diff`.

```rust
# #[macro_use] extern crate typenum;
# use typenum::*;
# fn main() {
assert_type_eq!(op!(P2 - P3), N1);
# }
```

---
Operator `<<`. Expands to `Shleft`.

```rust
# #[macro_use] extern crate typenum;
# use typenum::*;
# fn main() {
assert_type_eq!(op!(U1 << U5), U32);
# }
```

---
Operator `>>`. Expands to `Shright`.

```rust
# #[macro_use] extern crate typenum;
# use typenum::*;
# fn main() {
assert_type_eq!(op!(U32 >> U5), U1);
# }
```

---
Operator `&`. Expands to `And`.

```rust
# #[macro_use] extern crate typenum;
# use typenum::*;
# fn main() {
assert_type_eq!(op!(U5 & U3), U1);
# }
```

---
Operator `^`. Expands to `Xor`.

```rust
# #[macro_use] extern crate typenum;
# use typenum::*;
# fn main() {
assert_type_eq!(op!(U5 ^ U3), U6);
# }
```

---
Operator `|`. Expands to `Or`.

```rust
# #[macro_use] extern crate typenum;
# use typenum::*;
# fn main() {
assert_type_eq!(op!(U5 | U3), U7);
# }
```

---
Operator `==`. Expands to `Eq`.

```rust
# #[macro_use] extern crate typenum;
# use typenum::*;
# fn main() {
assert_type_eq!(op!(P5 == P3 + P2), True);
# }
```

---
Operator `!=`. Expands to `NotEq`.

```rust
# #[macro_use] extern crate typenum;
# use typenum::*;
# fn main() {
assert_type_eq!(op!(P5 != P3 + P2), False);
# }
```

---
Operator `<=`. Expands to `LeEq`.

```rust
# #[macro_use] extern crate typenum;
# use typenum::*;
# fn main() {
assert_type_eq!(op!(P6 <= P3 + P2), False);
# }
```

---
Operator `>=`. Expands to `GrEq`.

```rust
# #[macro_use] extern crate typenum;
# use typenum::*;
# fn main() {
assert_type_eq!(op!(P6 >= P3 + P2), True);
# }
```

---
Operator `<`. Expands to `Le`.

```rust
# #[macro_use] extern crate typenum;
# use typenum::*;
# fn main() {
assert_type_eq!(op!(P4 < P3 + P2), True);
# }
```

---
Operator `>`. Expands to `Gr`.

```rust
# #[macro_use] extern crate typenum;
# use typenum::*;
# fn main() {
assert_type_eq!(op!(P5 < P3 + P2), False);
# }
```

---
Operator `cmp`. Expands to `Compare`.

```rust
# #[macro_use] extern crate typenum;
# use typenum::*;
# fn main() {
assert_type_eq!(op!(cmp(P2, P3)), Less);
# }
```

---
Operator `sqr`. Expands to `Square`.

```rust
# #[macro_use] extern crate typenum;
# use typenum::*;
# fn main() {
assert_type_eq!(op!(sqr(P2)), P4);
# }
```

---
Operator `sqrt`. Expands to `Sqrt`.

```rust
# #[macro_use] extern crate typenum;
# use typenum::*;
# fn main() {
assert_type_eq!(op!(sqrt(U9)), U3);
# }
```

---
Operator `abs`. Expands to `AbsVal`.

```rust
# #[macro_use] extern crate typenum;
# use typenum::*;
# fn main() {
assert_type_eq!(op!(abs(N2)), P2);
# }
```

---
Operator `cube`. Expands to `Cube`.

```rust
# #[macro_use] extern crate typenum;
# use typenum::*;
# fn main() {
assert_type_eq!(op!(cube(P2)), P8);
# }
```

---
Operator `pow`. Expands to `Exp`.

```rust
# #[macro_use] extern crate typenum;
# use typenum::*;
# fn main() {
assert_type_eq!(op!(pow(P2, P3)), P8);
# }
```

---
Operator `min`. Expands to `Minimum`.

```rust
# #[macro_use] extern crate typenum;
# use typenum::*;
# fn main() {
assert_type_eq!(op!(min(P2, P3)), P2);
# }
```

---
Operator `max`. Expands to `Maximum`.

```rust
# #[macro_use] extern crate typenum;
# use typenum::*;
# fn main() {
assert_type_eq!(op!(max(P2, P3)), P3);
# }
```

---
Operator `log2`. Expands to `Log2`.

```rust
# #[macro_use] extern crate typenum;
# use typenum::*;
# fn main() {
assert_type_eq!(op!(log2(U9)), U3);
# }
```

---
Operator `gcd`. Expands to `Gcf`.

```rust
# #[macro_use] extern crate typenum;
# use typenum::*;
# fn main() {
assert_type_eq!(op!(gcd(U9, U21)), U3);
# }
```

*/
#[macro_export(local_inner_macros)]
macro_rules! op {
    ($($tail:tt)*) => ( __op_internal__!($($tail)*) );
}

#[doc(hidden)]
#[macro_export(local_inner_macros)]
macro_rules! __op_internal__ {

(@stack[$($stack:ident,)*] @queue[$($queue:ident,)*] @tail: cmp $($tail:tt)*) => (
    __op_internal__!(@stack[Compare, $($stack,)*] @queue[$($queue,)*] @tail: $($tail)*)
);
(@stack[$($stack:ident,)*] @queue[$($queue:ident,)*] @tail: sqr $($tail:tt)*) => (
    __op_internal__!(@stack[Square, $($stack,)*] @queue[$($queue,)*] @tail: $($tail)*)
);
(@stack[$($stack:ident,)*] @queue[$($queue:ident,)*] @tail: sqrt $($tail:tt)*) => (
    __op_internal__!(@stack[Sqrt, $($stack,)*] @queue[$($queue,)*] @tail: $($tail)*)
);
(@stack[$($stack:ident,)*] @queue[$($queue:ident,)*] @tail: abs $($tail:tt)*) => (
    __op_internal__!(@stack[AbsVal, $($stack,)*] @queue[$($queue,)*] @tail: $($tail)*)
);
(@stack[$($stack:ident,)*] @queue[$($queue:ident,)*] @tail: cube $($tail:tt)*) => (
    __op_internal__!(@stack[Cube, $($stack,)*] @queue[$($queue,)*] @tail: $($tail)*)
);
(@stack[$($stack:ident,)*] @queue[$($queue:ident,)*] @tail: pow $($tail:tt)*) => (
    __op_internal__!(@stack[Exp, $($stack,)*] @queue[$($queue,)*] @tail: $($tail)*)
);
(@stack[$($stack:ident,)*] @queue[$($queue:ident,)*] @tail: min $($tail:tt)*) => (
    __op_internal__!(@stack[Minimum, $($stack,)*] @queue[$($queue,)*] @tail: $($tail)*)
);
(@stack[$($stack:ident,)*] @queue[$($queue:ident,)*] @tail: max $($tail:tt)*) => (
    __op_internal__!(@stack[Maximum, $($stack,)*] @queue[$($queue,)*] @tail: $($tail)*)
);
(@stack[$($stack:ident,)*] @queue[$($queue:ident,)*] @tail: log2 $($tail:tt)*) => (
    __op_internal__!(@stack[Log2, $($stack,)*] @queue[$($queue,)*] @tail: $($tail)*)
);
(@stack[$($stack:ident,)*] @queue[$($queue:ident,)*] @tail: gcd $($tail:tt)*) => (
    __op_internal__!(@stack[Gcf, $($stack,)*] @queue[$($queue,)*] @tail: $($tail)*)
);
(@stack[LParen, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: , $($tail:tt)*) => (
    __op_internal__!(@stack[LParen, $($stack,)*] @queue[$($queue,)*] @tail: $($tail)*)
);
(@stack[$stack_top:ident, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: , $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[$stack_top, $($queue,)*] @tail: , $($tail)*)
);
(@stack[Prod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: * $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Prod, $($queue,)*] @tail: * $($tail)*)
);
(@stack[Quot, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: * $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Quot, $($queue,)*] @tail: * $($tail)*)
);
(@stack[Mod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: * $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Mod, $($queue,)*] @tail: * $($tail)*)
);
(@stack[$($stack:ident,)*] @queue[$($queue:ident,)*] @tail: * $($tail:tt)*) => (
    __op_internal__!(@stack[Prod, $($stack,)*] @queue[$($queue,)*] @tail: $($tail)*)
);
(@stack[Prod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: / $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Prod, $($queue,)*] @tail: / $($tail)*)
);
(@stack[Quot, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: / $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Quot, $($queue,)*] @tail: / $($tail)*)
);
(@stack[Mod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: / $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Mod, $($queue,)*] @tail: / $($tail)*)
);
(@stack[$($stack:ident,)*] @queue[$($queue:ident,)*] @tail: / $($tail:tt)*) => (
    __op_internal__!(@stack[Quot, $($stack,)*] @queue[$($queue,)*] @tail: $($tail)*)
);
(@stack[Prod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: % $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Prod, $($queue,)*] @tail: % $($tail)*)
);
(@stack[Quot, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: % $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Quot, $($queue,)*] @tail: % $($tail)*)
);
(@stack[Mod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: % $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Mod, $($queue,)*] @tail: % $($tail)*)
);
(@stack[$($stack:ident,)*] @queue[$($queue:ident,)*] @tail: % $($tail:tt)*) => (
    __op_internal__!(@stack[Mod, $($stack,)*] @queue[$($queue,)*] @tail: $($tail)*)
);
(@stack[Prod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: + $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Prod, $($queue,)*] @tail: + $($tail)*)
);
(@stack[Quot, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: + $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Quot, $($queue,)*] @tail: + $($tail)*)
);
(@stack[Mod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: + $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Mod, $($queue,)*] @tail: + $($tail)*)
);
(@stack[Sum, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: + $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Sum, $($queue,)*] @tail: + $($tail)*)
);
(@stack[Diff, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: + $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Diff, $($queue,)*] @tail: + $($tail)*)
);
(@stack[$($stack:ident,)*] @queue[$($queue:ident,)*] @tail: + $($tail:tt)*) => (
    __op_internal__!(@stack[Sum, $($stack,)*] @queue[$($queue,)*] @tail: $($tail)*)
);
(@stack[Prod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: - $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Prod, $($queue,)*] @tail: - $($tail)*)
);
(@stack[Quot, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: - $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Quot, $($queue,)*] @tail: - $($tail)*)
);
(@stack[Mod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: - $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Mod, $($queue,)*] @tail: - $($tail)*)
);
(@stack[Sum, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: - $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Sum, $($queue,)*] @tail: - $($tail)*)
);
(@stack[Diff, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: - $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Diff, $($queue,)*] @tail: - $($tail)*)
);
(@stack[$($stack:ident,)*] @queue[$($queue:ident,)*] @tail: - $($tail:tt)*) => (
    __op_internal__!(@stack[Diff, $($stack,)*] @queue[$($queue,)*] @tail: $($tail)*)
);
(@stack[Prod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: << $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Prod, $($queue,)*] @tail: << $($tail)*)
);
(@stack[Quot, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: << $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Quot, $($queue,)*] @tail: << $($tail)*)
);
(@stack[Mod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: << $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Mod, $($queue,)*] @tail: << $($tail)*)
);
(@stack[Sum, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: << $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Sum, $($queue,)*] @tail: << $($tail)*)
);
(@stack[Diff, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: << $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Diff, $($queue,)*] @tail: << $($tail)*)
);
(@stack[Shleft, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: << $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Shleft, $($queue,)*] @tail: << $($tail)*)
);
(@stack[Shright, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: << $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Shright, $($queue,)*] @tail: << $($tail)*)
);
(@stack[$($stack:ident,)*] @queue[$($queue:ident,)*] @tail: << $($tail:tt)*) => (
    __op_internal__!(@stack[Shleft, $($stack,)*] @queue[$($queue,)*] @tail: $($tail)*)
);
(@stack[Prod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: >> $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Prod, $($queue,)*] @tail: >> $($tail)*)
);
(@stack[Quot, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: >> $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Quot, $($queue,)*] @tail: >> $($tail)*)
);
(@stack[Mod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: >> $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Mod, $($queue,)*] @tail: >> $($tail)*)
);
(@stack[Sum, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: >> $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Sum, $($queue,)*] @tail: >> $($tail)*)
);
(@stack[Diff, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: >> $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Diff, $($queue,)*] @tail: >> $($tail)*)
);
(@stack[Shleft, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: >> $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Shleft, $($queue,)*] @tail: >> $($tail)*)
);
(@stack[Shright, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: >> $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Shright, $($queue,)*] @tail: >> $($tail)*)
);
(@stack[$($stack:ident,)*] @queue[$($queue:ident,)*] @tail: >> $($tail:tt)*) => (
    __op_internal__!(@stack[Shright, $($stack,)*] @queue[$($queue,)*] @tail: $($tail)*)
);
(@stack[Prod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: & $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Prod, $($queue,)*] @tail: & $($tail)*)
);
(@stack[Quot, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: & $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Quot, $($queue,)*] @tail: & $($tail)*)
);
(@stack[Mod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: & $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Mod, $($queue,)*] @tail: & $($tail)*)
);
(@stack[Sum, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: & $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Sum, $($queue,)*] @tail: & $($tail)*)
);
(@stack[Diff, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: & $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Diff, $($queue,)*] @tail: & $($tail)*)
);
(@stack[Shleft, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: & $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Shleft, $($queue,)*] @tail: & $($tail)*)
);
(@stack[Shright, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: & $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Shright, $($queue,)*] @tail: & $($tail)*)
);
(@stack[And, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: & $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[And, $($queue,)*] @tail: & $($tail)*)
);
(@stack[$($stack:ident,)*] @queue[$($queue:ident,)*] @tail: & $($tail:tt)*) => (
    __op_internal__!(@stack[And, $($stack,)*] @queue[$($queue,)*] @tail: $($tail)*)
);
(@stack[Prod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: ^ $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Prod, $($queue,)*] @tail: ^ $($tail)*)
);
(@stack[Quot, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: ^ $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Quot, $($queue,)*] @tail: ^ $($tail)*)
);
(@stack[Mod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: ^ $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Mod, $($queue,)*] @tail: ^ $($tail)*)
);
(@stack[Sum, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: ^ $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Sum, $($queue,)*] @tail: ^ $($tail)*)
);
(@stack[Diff, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: ^ $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Diff, $($queue,)*] @tail: ^ $($tail)*)
);
(@stack[Shleft, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: ^ $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Shleft, $($queue,)*] @tail: ^ $($tail)*)
);
(@stack[Shright, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: ^ $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Shright, $($queue,)*] @tail: ^ $($tail)*)
);
(@stack[And, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: ^ $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[And, $($queue,)*] @tail: ^ $($tail)*)
);
(@stack[Xor, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: ^ $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Xor, $($queue,)*] @tail: ^ $($tail)*)
);
(@stack[$($stack:ident,)*] @queue[$($queue:ident,)*] @tail: ^ $($tail:tt)*) => (
    __op_internal__!(@stack[Xor, $($stack,)*] @queue[$($queue,)*] @tail: $($tail)*)
);
(@stack[Prod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: | $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Prod, $($queue,)*] @tail: | $($tail)*)
);
(@stack[Quot, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: | $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Quot, $($queue,)*] @tail: | $($tail)*)
);
(@stack[Mod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: | $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Mod, $($queue,)*] @tail: | $($tail)*)
);
(@stack[Sum, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: | $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Sum, $($queue,)*] @tail: | $($tail)*)
);
(@stack[Diff, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: | $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Diff, $($queue,)*] @tail: | $($tail)*)
);
(@stack[Shleft, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: | $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Shleft, $($queue,)*] @tail: | $($tail)*)
);
(@stack[Shright, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: | $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Shright, $($queue,)*] @tail: | $($tail)*)
);
(@stack[And, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: | $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[And, $($queue,)*] @tail: | $($tail)*)
);
(@stack[Xor, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: | $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Xor, $($queue,)*] @tail: | $($tail)*)
);
(@stack[Or, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: | $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Or, $($queue,)*] @tail: | $($tail)*)
);
(@stack[$($stack:ident,)*] @queue[$($queue:ident,)*] @tail: | $($tail:tt)*) => (
    __op_internal__!(@stack[Or, $($stack,)*] @queue[$($queue,)*] @tail: $($tail)*)
);
(@stack[Prod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: == $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Prod, $($queue,)*] @tail: == $($tail)*)
);
(@stack[Quot, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: == $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Quot, $($queue,)*] @tail: == $($tail)*)
);
(@stack[Mod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: == $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Mod, $($queue,)*] @tail: == $($tail)*)
);
(@stack[Sum, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: == $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Sum, $($queue,)*] @tail: == $($tail)*)
);
(@stack[Diff, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: == $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Diff, $($queue,)*] @tail: == $($tail)*)
);
(@stack[Shleft, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: == $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Shleft, $($queue,)*] @tail: == $($tail)*)
);
(@stack[Shright, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: == $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Shright, $($queue,)*] @tail: == $($tail)*)
);
(@stack[And, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: == $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[And, $($queue,)*] @tail: == $($tail)*)
);
(@stack[Xor, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: == $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Xor, $($queue,)*] @tail: == $($tail)*)
);
(@stack[Or, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: == $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Or, $($queue,)*] @tail: == $($tail)*)
);
(@stack[Eq, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: == $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Eq, $($queue,)*] @tail: == $($tail)*)
);
(@stack[NotEq, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: == $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[NotEq, $($queue,)*] @tail: == $($tail)*)
);
(@stack[LeEq, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: == $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[LeEq, $($queue,)*] @tail: == $($tail)*)
);
(@stack[GrEq, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: == $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[GrEq, $($queue,)*] @tail: == $($tail)*)
);
(@stack[Le, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: == $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Le, $($queue,)*] @tail: == $($tail)*)
);
(@stack[Gr, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: == $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Gr, $($queue,)*] @tail: == $($tail)*)
);
(@stack[$($stack:ident,)*] @queue[$($queue:ident,)*] @tail: == $($tail:tt)*) => (
    __op_internal__!(@stack[Eq, $($stack,)*] @queue[$($queue,)*] @tail: $($tail)*)
);
(@stack[Prod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: != $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Prod, $($queue,)*] @tail: != $($tail)*)
);
(@stack[Quot, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: != $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Quot, $($queue,)*] @tail: != $($tail)*)
);
(@stack[Mod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: != $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Mod, $($queue,)*] @tail: != $($tail)*)
);
(@stack[Sum, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: != $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Sum, $($queue,)*] @tail: != $($tail)*)
);
(@stack[Diff, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: != $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Diff, $($queue,)*] @tail: != $($tail)*)
);
(@stack[Shleft, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: != $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Shleft, $($queue,)*] @tail: != $($tail)*)
);
(@stack[Shright, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: != $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Shright, $($queue,)*] @tail: != $($tail)*)
);
(@stack[And, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: != $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[And, $($queue,)*] @tail: != $($tail)*)
);
(@stack[Xor, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: != $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Xor, $($queue,)*] @tail: != $($tail)*)
);
(@stack[Or, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: != $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Or, $($queue,)*] @tail: != $($tail)*)
);
(@stack[Eq, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: != $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Eq, $($queue,)*] @tail: != $($tail)*)
);
(@stack[NotEq, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: != $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[NotEq, $($queue,)*] @tail: != $($tail)*)
);
(@stack[LeEq, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: != $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[LeEq, $($queue,)*] @tail: != $($tail)*)
);
(@stack[GrEq, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: != $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[GrEq, $($queue,)*] @tail: != $($tail)*)
);
(@stack[Le, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: != $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Le, $($queue,)*] @tail: != $($tail)*)
);
(@stack[Gr, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: != $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Gr, $($queue,)*] @tail: != $($tail)*)
);
(@stack[$($stack:ident,)*] @queue[$($queue:ident,)*] @tail: != $($tail:tt)*) => (
    __op_internal__!(@stack[NotEq, $($stack,)*] @queue[$($queue,)*] @tail: $($tail)*)
);
(@stack[Prod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: <= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Prod, $($queue,)*] @tail: <= $($tail)*)
);
(@stack[Quot, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: <= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Quot, $($queue,)*] @tail: <= $($tail)*)
);
(@stack[Mod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: <= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Mod, $($queue,)*] @tail: <= $($tail)*)
);
(@stack[Sum, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: <= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Sum, $($queue,)*] @tail: <= $($tail)*)
);
(@stack[Diff, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: <= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Diff, $($queue,)*] @tail: <= $($tail)*)
);
(@stack[Shleft, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: <= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Shleft, $($queue,)*] @tail: <= $($tail)*)
);
(@stack[Shright, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: <= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Shright, $($queue,)*] @tail: <= $($tail)*)
);
(@stack[And, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: <= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[And, $($queue,)*] @tail: <= $($tail)*)
);
(@stack[Xor, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: <= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Xor, $($queue,)*] @tail: <= $($tail)*)
);
(@stack[Or, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: <= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Or, $($queue,)*] @tail: <= $($tail)*)
);
(@stack[Eq, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: <= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Eq, $($queue,)*] @tail: <= $($tail)*)
);
(@stack[NotEq, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: <= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[NotEq, $($queue,)*] @tail: <= $($tail)*)
);
(@stack[LeEq, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: <= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[LeEq, $($queue,)*] @tail: <= $($tail)*)
);
(@stack[GrEq, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: <= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[GrEq, $($queue,)*] @tail: <= $($tail)*)
);
(@stack[Le, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: <= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Le, $($queue,)*] @tail: <= $($tail)*)
);
(@stack[Gr, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: <= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Gr, $($queue,)*] @tail: <= $($tail)*)
);
(@stack[$($stack:ident,)*] @queue[$($queue:ident,)*] @tail: <= $($tail:tt)*) => (
    __op_internal__!(@stack[LeEq, $($stack,)*] @queue[$($queue,)*] @tail: $($tail)*)
);
(@stack[Prod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: >= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Prod, $($queue,)*] @tail: >= $($tail)*)
);
(@stack[Quot, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: >= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Quot, $($queue,)*] @tail: >= $($tail)*)
);
(@stack[Mod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: >= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Mod, $($queue,)*] @tail: >= $($tail)*)
);
(@stack[Sum, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: >= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Sum, $($queue,)*] @tail: >= $($tail)*)
);
(@stack[Diff, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: >= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Diff, $($queue,)*] @tail: >= $($tail)*)
);
(@stack[Shleft, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: >= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Shleft, $($queue,)*] @tail: >= $($tail)*)
);
(@stack[Shright, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: >= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Shright, $($queue,)*] @tail: >= $($tail)*)
);
(@stack[And, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: >= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[And, $($queue,)*] @tail: >= $($tail)*)
);
(@stack[Xor, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: >= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Xor, $($queue,)*] @tail: >= $($tail)*)
);
(@stack[Or, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: >= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Or, $($queue,)*] @tail: >= $($tail)*)
);
(@stack[Eq, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: >= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Eq, $($queue,)*] @tail: >= $($tail)*)
);
(@stack[NotEq, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: >= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[NotEq, $($queue,)*] @tail: >= $($tail)*)
);
(@stack[LeEq, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: >= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[LeEq, $($queue,)*] @tail: >= $($tail)*)
);
(@stack[GrEq, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: >= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[GrEq, $($queue,)*] @tail: >= $($tail)*)
);
(@stack[Le, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: >= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Le, $($queue,)*] @tail: >= $($tail)*)
);
(@stack[Gr, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: >= $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Gr, $($queue,)*] @tail: >= $($tail)*)
);
(@stack[$($stack:ident,)*] @queue[$($queue:ident,)*] @tail: >= $($tail:tt)*) => (
    __op_internal__!(@stack[GrEq, $($stack,)*] @queue[$($queue,)*] @tail: $($tail)*)
);
(@stack[Prod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: < $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Prod, $($queue,)*] @tail: < $($tail)*)
);
(@stack[Quot, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: < $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Quot, $($queue,)*] @tail: < $($tail)*)
);
(@stack[Mod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: < $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Mod, $($queue,)*] @tail: < $($tail)*)
);
(@stack[Sum, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: < $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Sum, $($queue,)*] @tail: < $($tail)*)
);
(@stack[Diff, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: < $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Diff, $($queue,)*] @tail: < $($tail)*)
);
(@stack[Shleft, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: < $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Shleft, $($queue,)*] @tail: < $($tail)*)
);
(@stack[Shright, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: < $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Shright, $($queue,)*] @tail: < $($tail)*)
);
(@stack[And, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: < $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[And, $($queue,)*] @tail: < $($tail)*)
);
(@stack[Xor, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: < $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Xor, $($queue,)*] @tail: < $($tail)*)
);
(@stack[Or, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: < $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Or, $($queue,)*] @tail: < $($tail)*)
);
(@stack[Eq, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: < $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Eq, $($queue,)*] @tail: < $($tail)*)
);
(@stack[NotEq, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: < $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[NotEq, $($queue,)*] @tail: < $($tail)*)
);
(@stack[LeEq, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: < $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[LeEq, $($queue,)*] @tail: < $($tail)*)
);
(@stack[GrEq, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: < $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[GrEq, $($queue,)*] @tail: < $($tail)*)
);
(@stack[Le, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: < $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Le, $($queue,)*] @tail: < $($tail)*)
);
(@stack[Gr, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: < $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Gr, $($queue,)*] @tail: < $($tail)*)
);
(@stack[$($stack:ident,)*] @queue[$($queue:ident,)*] @tail: < $($tail:tt)*) => (
    __op_internal__!(@stack[Le, $($stack,)*] @queue[$($queue,)*] @tail: $($tail)*)
);
(@stack[Prod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: > $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Prod, $($queue,)*] @tail: > $($tail)*)
);
(@stack[Quot, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: > $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Quot, $($queue,)*] @tail: > $($tail)*)
);
(@stack[Mod, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: > $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Mod, $($queue,)*] @tail: > $($tail)*)
);
(@stack[Sum, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: > $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Sum, $($queue,)*] @tail: > $($tail)*)
);
(@stack[Diff, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: > $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Diff, $($queue,)*] @tail: > $($tail)*)
);
(@stack[Shleft, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: > $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Shleft, $($queue,)*] @tail: > $($tail)*)
);
(@stack[Shright, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: > $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Shright, $($queue,)*] @tail: > $($tail)*)
);
(@stack[And, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: > $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[And, $($queue,)*] @tail: > $($tail)*)
);
(@stack[Xor, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: > $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Xor, $($queue,)*] @tail: > $($tail)*)
);
(@stack[Or, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: > $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Or, $($queue,)*] @tail: > $($tail)*)
);
(@stack[Eq, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: > $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Eq, $($queue,)*] @tail: > $($tail)*)
);
(@stack[NotEq, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: > $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[NotEq, $($queue,)*] @tail: > $($tail)*)
);
(@stack[LeEq, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: > $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[LeEq, $($queue,)*] @tail: > $($tail)*)
);
(@stack[GrEq, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: > $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[GrEq, $($queue,)*] @tail: > $($tail)*)
);
(@stack[Le, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: > $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Le, $($queue,)*] @tail: > $($tail)*)
);
(@stack[Gr, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: > $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Gr, $($queue,)*] @tail: > $($tail)*)
);
(@stack[$($stack:ident,)*] @queue[$($queue:ident,)*] @tail: > $($tail:tt)*) => (
    __op_internal__!(@stack[Gr, $($stack,)*] @queue[$($queue,)*] @tail: $($tail)*)
);
(@stack[$($stack:ident,)*] @queue[$($queue:ident,)*] @tail: ( $($stuff:tt)* ) $($tail:tt)* )
 => (
    __op_internal__!(@stack[LParen, $($stack,)*] @queue[$($queue,)*]
                     @tail: $($stuff)* RParen $($tail)*)
);
(@stack[LParen, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: RParen $($tail:tt)*) => (
    __op_internal__!(@rp3 @stack[$($stack,)*] @queue[$($queue,)*] @tail: $($tail)*)
);
(@stack[$stack_top:ident, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: RParen $($tail:tt)*)
 => (
    __op_internal__!(@stack[$($stack,)*] @queue[$stack_top, $($queue,)*] @tail: RParen $($tail)*)
);
(@rp3 @stack[Compare, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Compare, $($queue,)*] @tail: $($tail)*)
);
(@rp3 @stack[Square, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Square, $($queue,)*] @tail: $($tail)*)
);
(@rp3 @stack[Sqrt, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Sqrt, $($queue,)*] @tail: $($tail)*)
);
(@rp3 @stack[AbsVal, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[AbsVal, $($queue,)*] @tail: $($tail)*)
);
(@rp3 @stack[Cube, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Cube, $($queue,)*] @tail: $($tail)*)
);
(@rp3 @stack[Exp, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Exp, $($queue,)*] @tail: $($tail)*)
);
(@rp3 @stack[Minimum, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Minimum, $($queue,)*] @tail: $($tail)*)
);
(@rp3 @stack[Maximum, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Maximum, $($queue,)*] @tail: $($tail)*)
);
(@rp3 @stack[Log2, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Log2, $($queue,)*] @tail: $($tail)*)
);
(@rp3 @stack[Gcf, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail: $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[Gcf, $($queue,)*] @tail: $($tail)*)
);
(@rp3 @stack[$($stack:ident,)*] @queue[$($queue:ident,)*] @tail: $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[$($queue,)*] @tail: $($tail)*)
);
(@stack[$($stack:ident,)*] @queue[$($queue:ident,)*] @tail: $num:ident $($tail:tt)*) => (
    __op_internal__!(@stack[$($stack,)*] @queue[$num, $($queue,)*] @tail: $($tail)*)
);
(@stack[] @queue[$($queue:ident,)*] @tail: ) => (
    __op_internal__!(@reverse[] @input: $($queue,)*)
);
(@stack[$stack_top:ident, $($stack:ident,)*] @queue[$($queue:ident,)*] @tail:) => (
    __op_internal__!(@stack[$($stack,)*] @queue[$stack_top, $($queue,)*] @tail: )
);
(@reverse[$($revved:ident,)*] @input: $head:ident, $($tail:ident,)* ) => (
    __op_internal__!(@reverse[$head, $($revved,)*] @input: $($tail,)*)
);
(@reverse[$($revved:ident,)*] @input: ) => (
    __op_internal__!(@eval @stack[] @input[$($revved,)*])
);
(@eval @stack[$a:ty, $b:ty, $($stack:ty,)*] @input[Prod, $($tail:ident,)*]) => (
    __op_internal__!(@eval @stack[$crate::Prod<$b, $a>, $($stack,)*] @input[$($tail,)*])
);
(@eval @stack[$a:ty, $b:ty, $($stack:ty,)*] @input[Quot, $($tail:ident,)*]) => (
    __op_internal__!(@eval @stack[$crate::Quot<$b, $a>, $($stack,)*] @input[$($tail,)*])
);
(@eval @stack[$a:ty, $b:ty, $($stack:ty,)*] @input[Mod, $($tail:ident,)*]) => (
    __op_internal__!(@eval @stack[$crate::Mod<$b, $a>, $($stack,)*] @input[$($tail,)*])
);
(@eval @stack[$a:ty, $b:ty, $($stack:ty,)*] @input[Sum, $($tail:ident,)*]) => (
    __op_internal__!(@eval @stack[$crate::Sum<$b, $a>, $($stack,)*] @input[$($tail,)*])
);
(@eval @stack[$a:ty, $b:ty, $($stack:ty,)*] @input[Diff, $($tail:ident,)*]) => (
    __op_internal__!(@eval @stack[$crate::Diff<$b, $a>, $($stack,)*] @input[$($tail,)*])
);
(@eval @stack[$a:ty, $b:ty, $($stack:ty,)*] @input[Shleft, $($tail:ident,)*]) => (
    __op_internal__!(@eval @stack[$crate::Shleft<$b, $a>, $($stack,)*] @input[$($tail,)*])
);
(@eval @stack[$a:ty, $b:ty, $($stack:ty,)*] @input[Shright, $($tail:ident,)*]) => (
    __op_internal__!(@eval @stack[$crate::Shright<$b, $a>, $($stack,)*] @input[$($tail,)*])
);
(@eval @stack[$a:ty, $b:ty, $($stack:ty,)*] @input[And, $($tail:ident,)*]) => (
    __op_internal__!(@eval @stack[$crate::And<$b, $a>, $($stack,)*] @input[$($tail,)*])
);
(@eval @stack[$a:ty, $b:ty, $($stack:ty,)*] @input[Xor, $($tail:ident,)*]) => (
    __op_internal__!(@eval @stack[$crate::Xor<$b, $a>, $($stack,)*] @input[$($tail,)*])
);
(@eval @stack[$a:ty, $b:ty, $($stack:ty,)*] @input[Or, $($tail:ident,)*]) => (
    __op_internal__!(@eval @stack[$crate::Or<$b, $a>, $($stack,)*] @input[$($tail,)*])
);
(@eval @stack[$a:ty, $b:ty, $($stack:ty,)*] @input[Eq, $($tail:ident,)*]) => (
    __op_internal__!(@eval @stack[$crate::Eq<$b, $a>, $($stack,)*] @input[$($tail,)*])
);
(@eval @stack[$a:ty, $b:ty, $($stack:ty,)*] @input[NotEq, $($tail:ident,)*]) => (
    __op_internal__!(@eval @stack[$crate::NotEq<$b, $a>, $($stack,)*] @input[$($tail,)*])
);
(@eval @stack[$a:ty, $b:ty, $($stack:ty,)*] @input[LeEq, $($tail:ident,)*]) => (
    __op_internal__!(@eval @stack[$crate::LeEq<$b, $a>, $($stack,)*] @input[$($tail,)*])
);
(@eval @stack[$a:ty, $b:ty, $($stack:ty,)*] @input[GrEq, $($tail:ident,)*]) => (
    __op_internal__!(@eval @stack[$crate::GrEq<$b, $a>, $($stack,)*] @input[$($tail,)*])
);
(@eval @stack[$a:ty, $b:ty, $($stack:ty,)*] @input[Le, $($tail:ident,)*]) => (
    __op_internal__!(@eval @stack[$crate::Le<$b, $a>, $($stack,)*] @input[$($tail,)*])
);
(@eval @stack[$a:ty, $b:ty, $($stack:ty,)*] @input[Gr, $($tail:ident,)*]) => (
    __op_internal__!(@eval @stack[$crate::Gr<$b, $a>, $($stack,)*] @input[$($tail,)*])
);
(@eval @stack[$a:ty, $b:ty, $($stack:ty,)*] @input[Compare, $($tail:ident,)*]) => (
    __op_internal__!(@eval @stack[$crate::Compare<$b, $a>, $($stack,)*] @input[$($tail,)*])
);
(@eval @stack[$a:ty, $b:ty, $($stack:ty,)*] @input[Exp, $($tail:ident,)*]) => (
    __op_internal__!(@eval @stack[$crate::Exp<$b, $a>, $($stack,)*] @input[$($tail,)*])
);
(@eval @stack[$a:ty, $b:ty, $($stack:ty,)*] @input[Minimum, $($tail:ident,)*]) => (
    __op_internal__!(@eval @stack[$crate::Minimum<$b, $a>, $($stack,)*] @input[$($tail,)*])
);
(@eval @stack[$a:ty, $b:ty, $($stack:ty,)*] @input[Maximum, $($tail:ident,)*]) => (
    __op_internal__!(@eval @stack[$crate::Maximum<$b, $a>, $($stack,)*] @input[$($tail,)*])
);
(@eval @stack[$a:ty, $b:ty, $($stack:ty,)*] @input[Gcf, $($tail:ident,)*]) => (
    __op_internal__!(@eval @stack[$crate::Gcf<$b, $a>, $($stack,)*] @input[$($tail,)*])
);
(@eval @stack[$a:ty, $($stack:ty,)*] @input[Square, $($tail:ident,)*]) => (
    __op_internal__!(@eval @stack[$crate::Square<$a>, $($stack,)*] @input[$($tail,)*])
);
(@eval @stack[$a:ty, $($stack:ty,)*] @input[Sqrt, $($tail:ident,)*]) => (
    __op_internal__!(@eval @stack[$crate::Sqrt<$a>, $($stack,)*] @input[$($tail,)*])
);
(@eval @stack[$a:ty, $($stack:ty,)*] @input[AbsVal, $($tail:ident,)*]) => (
    __op_internal__!(@eval @stack[$crate::AbsVal<$a>, $($stack,)*] @input[$($tail,)*])
);
(@eval @stack[$a:ty, $($stack:ty,)*] @input[Cube, $($tail:ident,)*]) => (
    __op_internal__!(@eval @stack[$crate::Cube<$a>, $($stack,)*] @input[$($tail,)*])
);
(@eval @stack[$a:ty, $($stack:ty,)*] @input[Log2, $($tail:ident,)*]) => (
    __op_internal__!(@eval @stack[$crate::Log2<$a>, $($stack,)*] @input[$($tail,)*])
);
(@eval @stack[$($stack:ty,)*] @input[$head:ident, $($tail:ident,)*]) => (
    __op_internal__!(@eval @stack[$head, $($stack,)*] @input[$($tail,)*])
);
(@eval @stack[$stack:ty,] @input[]) => (
    $stack
);
($($tail:tt)* ) => (
    __op_internal__!(@stack[] @queue[] @tail: $($tail)*)
);
}
