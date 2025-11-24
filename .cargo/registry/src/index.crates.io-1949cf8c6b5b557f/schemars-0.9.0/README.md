# Schemars

> [!NOTE]
> This branch is for the current v1 alpha version of Schemars which is still under development.
>
> The current stable release (v0.9.0) is identical to 1.0.0-alpha.18, but was made available for those who are unable to unwilling to use a pre-release version.
>
> For the previous stable release of Schemars (v0.8.x), see the [v0 branch](https://github.com/GREsau/schemars/tree/v0).
>
> For information on migrating from 0.8 to 0.9/1.0, see [the migration guide](https://graham.cool/schemars/migrating/).

[![CI Build](https://img.shields.io/github/actions/workflow/status/GREsau/schemars/ci.yml?branch=master&logo=GitHub)](https://github.com/GREsau/schemars/actions)
[![Crates.io](https://img.shields.io/crates/v/schemars)](https://crates.io/crates/schemars)
[![Docs](https://img.shields.io/docsrs/schemars/1.0.0--latest?label=docs)](https://docs.rs/schemars/1.0.0--latest)
[![MSRV 1.74+](https://img.shields.io/badge/msrv-1.74-blue)](https://blog.rust-lang.org/2023/11/16/Rust-1.74.0/)

Generate JSON Schema documents from Rust code

## Basic Usage

If you don't really care about the specifics, the easiest way to generate a JSON schema for your types is to `#[derive(JsonSchema)]` and use the `schema_for!` macro. All fields of the type must also implement `JsonSchema` - Schemars implements this for many standard library types.

```rust
use schemars::{schema_for, JsonSchema};

#[derive(JsonSchema)]
pub struct MyStruct {
    pub my_int: i32,
    pub my_bool: bool,
    pub my_nullable_enum: Option<MyEnum>,
}

#[derive(JsonSchema)]
pub enum MyEnum {
    StringNewType(String),
    StructVariant { floats: Vec<f32> },
}

let schema = schema_for!(MyStruct);
println!("{}", serde_json::to_string_pretty(&schema).unwrap());
```

<details>
<summary>Click to see the output JSON schema...</summary>

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "MyStruct",
  "type": "object",
  "properties": {
    "my_bool": {
      "type": "boolean"
    },
    "my_int": {
      "type": "integer",
      "format": "int32"
    },
    "my_nullable_enum": {
      "anyOf": [
        {
          "$ref": "#/$defs/MyEnum"
        },
        {
          "type": "null"
        }
      ]
    }
  },
  "required": ["my_int", "my_bool"],
  "$defs": {
    "MyEnum": {
      "oneOf": [
        {
          "type": "object",
          "properties": {
            "StringNewType": {
              "type": "string"
            }
          },
          "additionalProperties": false,
          "required": ["StringNewType"]
        },
        {
          "type": "object",
          "properties": {
            "StructVariant": {
              "type": "object",
              "properties": {
                "floats": {
                  "type": "array",
                  "items": {
                    "type": "number",
                    "format": "float"
                  }
                }
              },
              "required": ["floats"]
            }
          },
          "additionalProperties": false,
          "required": ["StructVariant"]
        }
      ]
    }
  }
}
```

</details>

### Serde Compatibility

One of the main aims of this library is compatibility with [Serde](https://github.com/serde-rs/serde). Any generated schema _should_ match how [serde_json](https://github.com/serde-rs/json) would serialize/deserialize to/from JSON. To support this, Schemars will check for any `#[serde(...)]` attributes on types that derive `JsonSchema`, and adjust the generated schema accordingly.

```rust
use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MyStruct {
    #[serde(rename = "myNumber")]
    pub my_int: i32,
    pub my_bool: bool,
    #[serde(default)]
    pub my_nullable_enum: Option<MyEnum>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum MyEnum {
    StringNewType(String),
    StructVariant { floats: Vec<f32> },
}

let schema = schema_for!(MyStruct);
println!("{}", serde_json::to_string_pretty(&schema).unwrap());
```

<details>
<summary>Click to see the output JSON schema...</summary>

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "MyStruct",
  "type": "object",
  "properties": {
    "myBool": {
      "type": "boolean"
    },
    "myNullableEnum": {
      "anyOf": [
        {
          "$ref": "#/$defs/MyEnum"
        },
        {
          "type": "null"
        }
      ],
      "default": null
    },
    "myNumber": {
      "type": "integer",
      "format": "int32"
    }
  },
  "additionalProperties": false,
  "required": ["myNumber", "myBool"],
  "$defs": {
    "MyEnum": {
      "anyOf": [
        {
          "type": "string"
        },
        {
          "type": "object",
          "properties": {
            "floats": {
              "type": "array",
              "items": {
                "type": "number",
                "format": "float"
              }
            }
          },
          "required": ["floats"]
        }
      ]
    }
  }
}
```

</details>

`#[serde(...)]` attributes can be overriden using `#[schemars(...)]` attributes, which behave identically (e.g. `#[schemars(rename_all = "camelCase")]`). You may find this useful if you want to change the generated schema without affecting Serde's behaviour, or if you're just not using Serde.

### Schema from Example Value

If you want a schema for a type that can't/doesn't implement `JsonSchema`, but does implement `serde::Serialize`, then you can generate a JSON schema from a value of that type. However, this schema will generally be less precise than if the type implemented `JsonSchema` - particularly when it involves enums, since schemars will not make any assumptions about the structure of an enum based on a single variant.

```rust
use schemars::schema_for_value;
use serde::Serialize;

#[derive(Serialize)]
pub struct MyStruct {
    pub my_int: i32,
    pub my_bool: bool,
    pub my_nullable_enum: Option<MyEnum>,
}

#[derive(Serialize)]
pub enum MyEnum {
    StringNewType(String),
    StructVariant { floats: Vec<f32> },
}

let schema = schema_for_value!(MyStruct {
    my_int: 123,
    my_bool: true,
    my_nullable_enum: Some(MyEnum::StringNewType("foo".to_string()))
});
println!("{}", serde_json::to_string_pretty(&schema).unwrap());
```

<details>
<summary>Click to see the output JSON schema...</summary>

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "MyStruct",
  "examples": [
    {
      "my_bool": true,
      "my_int": 123,
      "my_nullable_enum": {
        "StringNewType": "foo"
      }
    }
  ],
  "type": "object",
  "properties": {
    "my_bool": {
      "type": "boolean"
    },
    "my_int": {
      "type": "integer"
    },
    "my_nullable_enum": true
  }
}
```

</details>

## Feature Flags

- `std` (enabled by default) - implements `JsonSchema` for types in the rust standard library (`JsonSchema` is still implemented on types in `core` and `alloc`, even when this feature is disabled). Disable this feature to use schemars in `no_std` environments.
- `derive` (enabled by default) - provides `#[derive(JsonSchema)]` macro
- `preserve_order` - keep the order of struct fields in `Schema` properties
- `raw_value` - implements `JsonSchema` for `serde_json::value::RawValue` (enables the serde_json `raw_value` feature)

Schemars can implement `JsonSchema` on types from several popular crates, enabled via feature flags (dependency versions are shown in brackets):

- `arrayvec07` - [arrayvec](https://crates.io/crates/arrayvec) (^0.7)
- `bigdecimal04` - [bigdecimal](https://crates.io/crates/bigdecimal) (^0.4)
- `bytes1` - [bytes](https://crates.io/crates/bytes) (^1.0)
- `chrono04` - [chrono](https://crates.io/crates/chrono) (^0.4)
- `either1` - [either](https://crates.io/crates/either) (^1.3)
- `indexmap2` - [indexmap](https://crates.io/crates/indexmap) (^2.0)
- `jiff02` - [jiff](https://crates.io/crates/jiff) (^0.2)
- `rust_decimal1` - [rust_decimal](https://crates.io/crates/rust_decimal) (^1.0)
- `semver1` - [semver](https://crates.io/crates/semver) (^1.0.9)
- `smallvec1` - [smallvec](https://crates.io/crates/smallvec) (^1.0)
- `smol_str02` - [smol_str](https://crates.io/crates/smol_str) (^0.2.1)
- `url2` - [url](https://crates.io/crates/url) (^2.0)
- `uuid1` - [uuid](https://crates.io/crates/uuid) (^1.0)

For example, to implement `JsonSchema` on types from `chrono`, enable it as a feature in the `schemars` dependency in your `Cargo.toml` like so:

```toml
[dependencies]
schemars = { version = "0.9.0", features = ["chrono04"] }
```
