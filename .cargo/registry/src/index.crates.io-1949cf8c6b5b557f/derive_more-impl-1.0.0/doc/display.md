# What `#[derive(Display)]` generates

Deriving `Display` will generate a `Display` implementation, with a `fmt`
method that matches `self` and each of its variants. In the case of a struct or union,
only a single variant is available, and it is thus equivalent to a simple `let` statement.
In the case of an enum, each of its variants is matched.

For each matched variant, a `write!` expression will be generated with
the supplied format, or an automatically inferred one.

You specify the format on each variant by writing e.g. `#[display("my val: {}", some_val * 2)]`.
For enums, you can either specify it on each variant, or on the enum as a whole.

For variants that don't have a format specified, it will simply defer to the format of the
inner variable. If there is no such variable, or there is more than 1, an error is generated.




## The format of the format

You supply a format by attaching an attribute of the syntax: `#[display("...", args...)]`.
The format supplied is passed verbatim to `write!`.

The variables available in the arguments is `self` and each member of the
struct or enum variant, with members of tuple structs being named with a
leading underscore and their index, i.e. `_0`, `_1`, `_2`, etc. Due to
ownership/lifetime limitations the member variables are all references to the
fields, except when used directly in the format string. For most purposes this
detail doesn't matter, but it is quite important when using `Pointer`
formatting. If you don't use the `{field:p}` syntax, you have to dereference
once to get the address of the field itself, instead of the address of the
reference to the field:

```rust
# use derive_more::Display;
#
#[derive(Display)]
#[display("{field:p} {:p}", *field)]
struct RefInt<'a> {
    field: &'a i32,
}

let a = &123;
assert_eq!(format!("{}", RefInt{field: &a}), format!("{a:p} {:p}", a));
```

For enums you can also specify a shared format on the enum itself instead of
the variant. This format is used for each of the variants, and can be
customized per variant by including the special `{_variant}` placeholder in
this shared format, which is then replaced by the format string that's provided
on the variant.


### Other formatting traits

The syntax does not change, but the name of the attribute is the snake case version of the trait.
E.g. `Octal` -> `octal`, `Pointer` -> `pointer`, `UpperHex` -> `upper_hex`.

Note, that `Debug` has a slightly different API and semantics, described in its docs, and so,
requires a separate `debug` feature.


### Generic data types

When deriving `Display` (or other formatting trait) for a generic struct/enum, all generic type
arguments used during formatting are bound by respective formatting trait.
Bounds can only be inferred this way if a field is used directly in the interpolation.

E.g., for a structure `Foo` defined like this:
```rust
# use derive_more::Display;
#
# trait Trait { type Type; }
#
#[derive(Display)]
#[display("{a} {b} {c:?} {d:p}")]
struct Foo<'a, T1, T2: Trait, T3> {
    a: T1,
    b: <T2 as Trait>::Type,
    c: Vec<T3>,
    d: &'a T1,
}
```

The following where clauses would be generated:
* `T1: Display`
* `<T2 as Trait>::Type: Display`
* `Vec<T3>: Debug`
* `&'a T1: Pointer`


### Custom trait bounds

Sometimes you may want to specify additional trait bounds on your generic type parameters, so that they
could be used during formatting. This can be done with a `#[display(bound(...))]` attribute.

`#[display(bound(...))]` accepts code tokens in a format similar to the format
used in angle bracket list (or `where` clause predicates): `T: MyTrait, U: Trait1 + Trait2`.

`#[display("fmt", ...)]` arguments are parsed as an arbitrary Rust expression and passed to generated
`write!` as-is, it's impossible to meaningfully infer any kind of trait bounds for generic type parameters
used this way. That means that you'll **have to** explicitly specify all the required trait bounds of the
expression. Either in the struct/enum definition, or via `#[display(bound(...))]` attribute.

Explicitly specified bounds are added to the inferred ones. Note how no `V: Display` bound is necessary,
because it's inferred already.

```rust
# use derive_more::Display;
#
# trait MyTrait { fn my_function(&self) -> i32; }
#
#[derive(Display)]
#[display(bound(T: MyTrait, U: Display))]
#[display("{} {} {}", a.my_function(), b.to_string().len(), c)]
struct MyStruct<T, U, V> {
    a: T,
    b: U,
    c: V,
}
```


### Transparency

If the `#[display("...", args...)]` attribute is omitted, the implementation transparently delegates to the format
of the inner type, so all the additional [formatting parameters][1] do work as expected:
```rust
# use derive_more::Display;
#
#[derive(Display)]
struct MyInt(i32);

assert_eq!(format!("{:03}", MyInt(7)), "007");
```

If the `#[display("...", args...)]` attribute is specified and can be trivially substituted with a transparent
delegation call to the inner type, then additional [formatting parameters][1] will work too:
```rust
# use derive_more::Display;
#
#[derive(Display)]
#[display("{_0:o}")] // the same as calling `Octal::fmt()`
struct MyOctalInt(i32);

// so, additional formatting parameters do work transparently
assert_eq!(format!("{:03}", MyOctalInt(9)), "011");

#[derive(Display)]
#[display("{_0:02b}")]   // cannot be trivially substituted with `Binary::fmt()`,
struct MyBinaryInt(i32); // because of specified formatting parameters

// so, additional formatting parameters have no effect
assert_eq!(format!("{:07}", MyBinaryInt(2)), "10");
```

If, for some reason, transparency in trivial cases is not desired, it may be suppressed explicitly
either with the [`format_args!()`] macro usage:
```rust
# use derive_more::Display;
#
#[derive(Display)]
#[display("{}", format_args!("{_0:o}"))] // `format_args!()` obscures the inner type
struct MyOctalInt(i32);

// so, additional formatting parameters have no effect
assert_eq!(format!("{:07}", MyOctalInt(9)), "11");
```
Or by adding [formatting parameters][1] which cause no visual effects:
```rust
# use derive_more::Display;
#
#[derive(Display)]
#[display("{_0:^o}")] // `^` is centering, but in absence of additional width has no effect
struct MyOctalInt(i32);

// and so, additional formatting parameters have no effect
assert_eq!(format!("{:07}", MyOctalInt(9)), "11");
```




## Example usage

```rust
# use std::path::PathBuf;
#
# use derive_more::{Display, Octal, UpperHex};
#
#[derive(Display)]
struct MyInt(i32);

#[derive(Display)]
#[display("({x}, {y})")]
struct Point2D {
    x: i32,
    y: i32,
}

#[derive(Display)]
#[display("Enum E: {_variant}")]
enum E {
    Uint(u32),
    #[display("I am B {:b}", i)]
    Binary {
        i: i8,
    },
    #[display("I am C {}", _0.display())]
    Path(PathBuf),
}

#[derive(Display)]
#[display("Enum E2: {_0:?}")]
enum E2 {
    Uint(u32),
    String(&'static str, &'static str),
}

#[derive(Display)]
#[display("Hello there!")]
union U {
    i: u32,
}

#[derive(Octal)]
#[octal("7")]
struct S;

#[derive(UpperHex)]
#[upper_hex("UpperHex")]
struct UH;

#[derive(Display)]
struct Unit;

#[derive(Display)]
struct UnitStruct {}

#[derive(Display)]
#[display("{}", self.sign())]
struct PositiveOrNegative {
    x: i32,
}

impl PositiveOrNegative {
    fn sign(&self) -> &str {
        if self.x >= 0 {
            "Positive"
        } else {
            "Negative"
        }
    }
}

assert_eq!(MyInt(-2).to_string(), "-2");
assert_eq!(Point2D { x: 3, y: 4 }.to_string(), "(3, 4)");
assert_eq!(E::Uint(2).to_string(), "Enum E: 2");
assert_eq!(E::Binary { i: -2 }.to_string(), "Enum E: I am B 11111110");
assert_eq!(E::Path("abc".into()).to_string(), "Enum E: I am C abc");
assert_eq!(E2::Uint(2).to_string(), "Enum E2: 2");
assert_eq!(E2::String("shown", "ignored").to_string(), "Enum E2: \"shown\"");
assert_eq!(U { i: 2 }.to_string(), "Hello there!");
assert_eq!(format!("{:o}", S), "7");
assert_eq!(format!("{:X}", UH), "UpperHex");
assert_eq!(Unit.to_string(), "Unit");
assert_eq!(UnitStruct {}.to_string(), "UnitStruct");
assert_eq!(PositiveOrNegative { x: 1 }.to_string(), "Positive");
assert_eq!(PositiveOrNegative { x: -1 }.to_string(), "Negative");
```




[`format_args!()`]: https://doc.rust-lang.org/stable/std/macro.format_args.html

[1]: https://doc.rust-lang.org/stable/std/fmt/index.html#formatting-parameters
