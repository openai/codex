# Bitflags

`bitflags` generates flags enums with well-defined semantics and ergonomic end-user APIs.

You can use `bitflags` to:

- provide more user-friendly bindings to C APIs where flags may or may not be fully known in advance.
- generate efficient options types with string parsing and formatting support.

You can't use `bitflags` to:

- guarantee only bits corresponding to defined flags will ever be set. `bitflags` allows access to the underlying bits type so arbitrary bits may be set.
- define bitfields. `bitflags` only generates types where set bits denote the presence of some combination of flags.

## Definitions

This section formally defines the terminology and semantics of `bitflags`. It's organized so more fundamental concepts are introduced before those that build on them. It may be helpful to start from the bottom of the section and refer back up to concepts defined earlier.

Examples use `bitflags` syntax with `u8` as the bits type.

### Bits type

A type that defines a fixed number of bits at specific locations.

----

Bits types are typically fixed-width unsigned integers. For example, `u8` is a bits type that defines 8 bits; bit-0 through bit-7.

### Bits value

An instance of a bits type where each bit may be set (`1`) or unset (`0`).

----

Some examples of bits values for the bits type `u8` are:

```rust
0b0000_0000
0b1111_1111
0b1010_0101
```

#### Equality

Two bits values are equal if their bits are in the same configuration; set bits in one are set in the other, and unset bits in one are unset in the other.

#### Operations

Bits values define the bitwise operators and (`&`), or (`|`), exclusive-or (`^`), and negation (`!`) that apply to each of their bits.

### Flag

A set of bits in a bits type that may have a unique name.

----

Bits are not required to be exclusive to a flag. Bits are not required to be contiguous.

The following is a flag for `u8` with the name `A` that includes bit-0:

```rust
const A = 0b0000_0001;
```

The following is a flag for `u8` with the name `B` that includes bit-0, and bit-5:

```rust
const B = 0b0010_0001;
```

#### Named flag

A flag with a name.

----

The following is a named flag, where the name is `A`:

```rust
const A = 0b0000_0001;
```

#### Unnamed flag

A flag without a name.

----

The following is an unnamed flag:

```rust
const _ = 0b0000_0001;
```

#### Zero-bit flag

A flag with a set of zero bits.

----

The following is a zero-bit flag:

```rust
const ZERO = 0b0000_0000;
```

#### Single-bit flag

A flag with a set of one bit.

----

The following are single-bit flags:

```rust
const A = 0b0000_0001;
const B = 0b0000_0010;
```

#### Multi-bit flag

A flag with a set of more than one bit.

----

The following are multi-bit flags:

```rust
const A = 0b0000_0011;
const B = 0b1111_1111;
```

### Flags type

A set of defined flags over a specific bits type.

#### Known bit

A bit in any defined flag.

----

In the following flags type:

```rust
struct Flags {
    const A = 0b0000_0001;
    const B = 0b0000_0010;
    const C = 0b0000_0100;
}
```

the known bits are:

```rust
0b0000_0111
```

#### Unknown bit

A bit not in any defined flag.

----

In the following flags type:

```rust
struct Flags {
    const A = 0b0000_0001;
    const B = 0b0000_0010;
    const C = 0b0000_0100;
}
```

the unknown bits are:

```rust
0b1111_1000
```

### Flags value

An instance of a flags type using its specific bits value for storage.

The flags value of a flag is one where each of its bits is set, and all others are unset.

#### Contains

Whether all set bits in a source flags value are also set in a target flags value.

----

Given the flags value:

```rust
0b0000_0011
```

the following flags values are contained:

```rust
0b0000_0000
0b0000_0010
0b0000_0001
0b0000_0011
```

but the following flags values are not contained:

```rust
0b0000_1000
0b0000_0110
```

#### Intersects

Whether any set bits in a source flags value are also set in a target flags value.

----

Given the flags value:

```rust
0b0000_0011
```

the following flags intersect:

```rust
0b0000_0010
0b0000_0001
0b1111_1111
```

but the following flags values do not intersect:

```rust
0b0000_0000
0b1111_0000
```

#### Empty

Whether all bits in a flags value are unset.

----

The following flags value is empty:

```rust
0b0000_0000
```

The following flags values are not empty:

```rust
0b0000_0001
0b0110_0000
```

#### All

Whether all defined flags are contained in a flags value.

----

Given a flags type:

```rust
struct Flags {
    const A   = 0b0000_0001;
    const B   = 0b0000_0010;
}
```

the following flags values all satisfy all:

```rust
0b0000_0011
0b1000_0011
0b1111_1111
```

### Operations

Examples in this section all use the given flags type:

```rust
struct Flags {
    const A = 0b0000_0001;
    const B = 0b0000_0010;
    const C = 0b0000_1100;
}
```

#### Truncate

Unset all unknown bits in a flags value.

----

Given the flags value:

```rust
0b1111_1111
```

the result of truncation will be:

```rust
0b0000_1111
```

----

Truncating doesn't guarantee that a non-empty result will contain any defined flags. Given the following flags type:

```rust
struct Flags {
    const A = 0b0000_0101;
}
```

and the following flags value:

```rust
0b0000_1110;
```

The result of truncation will be:

```rust
0b0000_0100;
```

which intersects the flag `A`, but doesn't contain it.

This behavior is possible even when only operating with flags values containing defined flags. Given the following flags type:

```rust
struct Flags {
    const A = 0b0000_0101;
    const B = 0b0000_0001;
}
```

The result of `A ^ B` is `0b0000_0100`, which also doesn't contain any defined flag.

----

If all known bits are in the set of at least one defined single-bit flag, then all operations that produce non-empty results will always contain defined flags.

#### Union

The bitwise or (`|`) of the bits in two flags values.

----

The following are examples of the result of unioning flags values:

```rust
0b0000_0001 | 0b0000_0010 = 0b0000_0011
0b0000_0000 | 0b1111_1111 = 0b1111_1111
```

#### Intersection

The bitwise and (`&`) of the bits in two flags values.

----

The following are examples of the result of intersecting flags values:

```rust
0b0000_0001 & 0b0000_0010 = 0b0000_0000
0b1111_1100 & 0b1111_0111 = 0b1111_0100
0b1111_1111 & 0b1111_1111 = 0b1111_1111
```

#### Symmetric difference

The bitwise exclusive-or (`^`) of the bits in two flags values.

----

The following are examples of the symmetric difference between two flags values:

```rust
0b0000_0001 ^ 0b0000_0010 = 0b0000_0011
0b0000_1111 ^ 0b0000_0011 = 0b0000_1100
0b1100_0000 ^ 0b0011_0000 = 0b1111_0000
```

#### Complement

The bitwise negation (`!`) of the bits in a flags value, truncating the result.

----

The complement is the only operation that explicitly truncates its result, because it doesn't accept a second flags value as input and so is likely to set unknown bits.

----

The following are examples of the complement of a flags value:

```rust
!0b0000_0000 = 0b0000_1111
!0b0000_1111 = 0b0000_0000
!0b1111_1000 = 0b0000_0111
```

#### Difference

The bitwise intersection (`&`) of the bits in one flags value and the bitwise negation (`!`) of the bits in another.

----

This operation is not equivalent to the intersection of one flags value with the complement of another (`&!`).
The former will truncate the result in the complement, where difference will not.

----

The following are examples of the difference between two flags values:

```rust
0b0000_0001 & !0b0000_0010 = 0b0000_0001
0b0000_1101 & !0b0000_0011 = 0b0000_1100
0b1111_1111 & !0b0000_0001 = 0b1111_1110
```

### Iteration

Yield the bits of a source flags value in a set of contained flags values.

----

To be most useful, each yielded flags value should set exactly the bits of a defined flag contained in the source. Any known bits that aren't in the set of any contained flag should be yielded together as a final flags value.

----

Given the following flags type:

```rust
struct Flags {
    const A  = 0b0000_0001;
    const B  = 0b0000_0010;
    const AB = 0b0000_0011;
}
```

and the following flags value:

```rust
0b0000_1111
```

When iterated it may yield a flags value for `A` and `B`, then a final flag with the unknown bits:

```rust
0b0000_0001
0b0000_0010
0b0000_1100
```

It may also yield a flags value for `AB`, then a final flag with the unknown bits:

```rust
0b0000_0011
0b0000_1100
```

----

Given the following flags type:

```rust
struct Flags {
    const A = 0b0000_0011;
}
```

and the following flags value:

```rust
0b0000_0001
```

When iterated it will still yield a flags value for the known bit `0b0000_0001` even though it doesn't contain a flag.

### Formatting

Format and parse a flags value as text using the following grammar:

- _Flags:_ (_Whitespace_ _Flag_ _Whitespace_)`|`*
- _Flag:_ _Name_ | _Hex Number_
- _Name:_ The name of any defined flag
- _Hex Number_: `0x`([0-9a-fA-F])*
- _Whitespace_: (\s)*

Flags values can be formatted as _Flags_ by iterating over them, formatting each yielded flags value as a _Flag_. Any yielded flags value that sets exactly the bits of a defined flag with a name should be formatted as a _Name_. Otherwise it must be formatted as a _Hex Number_.

Formatting and parsing supports three modes:

- **Retain**: Formatting and parsing roundtrips exactly the bits of the source flags value. This is the default behavior.
- **Truncate**: Flags values are truncated before formatting, and truncated after parsing.
- **Strict**: A _Flag_ may only be formatted and parsed as a _Name_. _Hex numbers_ are not allowed. A consequence of this is that unknown bits and any bits that aren't in a contained named flag will be ignored. This is recommended for flags values serialized across API boundaries, like web services.

Text that is empty or whitespace is an empty flags value.

----

Given the following flags type:

```rust
struct Flags {
    const A  = 0b0000_0001;
    const B  = 0b0000_0010;
    const AB = 0b0000_0011;
    const C  = 0b0000_1100;
}
```

The following are examples of how flags values can be formatted using any mode:

```rust
0b0000_0000 = ""
0b0000_0001 = "A"
0b0000_0010 = "B"
0b0000_0011 = "A | B"
0b0000_0011 = "AB"
0b0000_1111 = "A | B | C"
```

Truncate mode will unset any unknown bits:

```rust
0b1000_0000 = ""
0b1111_1111 = "A | B | C"
0b0000_1000 = "0x8"
```

Retain mode will include any unknown bits as a final _Flag_:

```rust
0b1000_0000 = "0x80"
0b1111_1111 = "A | B | C | 0xf0"
0b0000_1000 = "0x8"
```

Strict mode will unset any unknown bits, as well as bits not contained in any defined named flags:

```rust
0b1000_0000 = ""
0b1111_1111 = "A | B | C"
0b0000_1000 = ""
```
