[![Crates.io](https://img.shields.io/crates/v/assert-json-diff.svg)](https://crates.io/crates/assert-json-diff)
[![Docs](https://docs.rs/assert-json-diff/badge.svg)](https://docs.rs/assert-json-diff)
[![dependency status](https://deps.rs/repo/github/davidpdrsn/assert-json-diff/status.svg)](https://deps.rs/repo/github/davidpdrsn/assert-json-diff)
[![Build status](https://github.com/davidpdrsn/assert-json-diff/workflows/CI/badge.svg)](https://github.com/davidpdrsn/assert-json-diff/actions)
![maintenance-status](https://img.shields.io/badge/maintenance-passively--maintained-yellowgreen.svg)

# assert-json-diff

This crate includes macros for comparing two serializable values by diffing their JSON
representations. It is designed to give much more helpful error messages than the standard
[`assert_eq!`]. It basically does a diff of the two objects and tells you the exact
differences. This is useful when asserting that two large JSON objects are the same.

It uses the [serde] and [serde_json] to perform the serialization.

[serde]: https://crates.io/crates/serde
[serde_json]: https://crates.io/crates/serde_json
[`assert_eq!`]: https://doc.rust-lang.org/std/macro.assert_eq.html

### Partial matching

If you want to assert that one JSON value is "included" in another use
[`assert_json_include`](macro.assert_json_include.html):

```rust
use assert_json_diff::assert_json_include;
use serde_json::json;

let a = json!({
    "data": {
        "users": [
            {
                "id": 1,
                "country": {
                    "name": "Denmark"
                }
            },
            {
                "id": 24,
                "country": {
                    "name": "Denmark"
                }
            }
        ]
    }
});

let b = json!({
    "data": {
        "users": [
            {
                "id": 1,
                "country": {
                    "name": "Sweden"
                }
            },
            {
                "id": 2,
                "country": {
                    "name": "Denmark"
                }
            }
        ]
    }
});

assert_json_include!(actual: a, expected: b)
```

This will panic with the error message:

```
json atoms at path ".data.users[0].country.name" are not equal:
    expected:
        "Sweden"
    actual:
        "Denmark"

json atoms at path ".data.users[1].id" are not equal:
    expected:
        2
    actual:
        24
```

[`assert_json_include`](macro.assert_json_include.html) allows extra data in `actual` but not in `expected`. That is so you can verify just a part
of the JSON without having to specify the whole thing. For example this test passes:

```rust
use assert_json_diff::assert_json_include;
use serde_json::json;

assert_json_include!(
    actual: json!({
        "a": { "b": 1 },
    }),
    expected: json!({
        "a": {},
    })
)
```

However `expected` cannot contain additional data so this test fails:

```rust
use assert_json_diff::assert_json_include;
use serde_json::json;

assert_json_include!(
    actual: json!({
        "a": {},
    }),
    expected: json!({
        "a": { "b": 1 },
    })
)
```

That will print

```
json atom at path ".a.b" is missing from actual
```

### Exact matching

If you want to ensure two JSON values are *exactly* the same, use [`assert_json_eq`](macro.assert_json_eq.html).

```rust
use assert_json_diff::assert_json_eq;
use serde_json::json;

assert_json_eq!(
    json!({ "a": { "b": 1 } }),
    json!({ "a": {} })
)
```

This will panic with the error message:

```
json atom at path ".a.b" is missing from lhs
```

### Further customization

You can use [`assert_json_matches`] to further customize the comparison.

License: MIT
