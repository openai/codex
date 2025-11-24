# Schemars

> [!NOTE]
> Schemars 1.0 is in development on [the v1 branch](https://github.com/GREsau/schemars/tree/v1), see [draft PR 290](https://github.com/GREsau/schemars/pull/290) for updates

[![CI Build](https://img.shields.io/github/actions/workflow/status/GREsau/schemars/ci.yml?branch=master&logo=GitHub)](https://github.com/GREsau/schemars/actions)
[![Crates.io](https://img.shields.io/crates/v/schemars)](https://crates.io/crates/schemars)
[![Docs](https://docs.rs/schemars/badge.svg)](https://docs.rs/schemars)
[![MSRV 1.60+](https://img.shields.io/badge/schemars-rustc_1.60+-lightgray.svg)](https://blog.rust-lang.org/2022/04/07/Rust-1.60.0.html)

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
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "MyStruct",
  "type": "object",
  "required": ["my_bool", "my_int"],
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
          "$ref": "#/definitions/MyEnum"
        },
        {
          "type": "null"
        }
      ]
    }
  },
  "definitions": {
    "MyEnum": {
      "anyOf": [
        {
          "type": "object",
          "required": ["StringNewType"],
          "properties": {
            "StringNewType": {
              "type": "string"
            }
          },
          "additionalProperties": false
        },
        {
          "type": "object",
          "required": ["StructVariant"],
          "properties": {
            "StructVariant": {
              "type": "object",
              "required": ["floats"],
              "properties": {
                "floats": {
                  "type": "array",
                  "items": {
                    "type": "number",
                    "format": "float"
                  }
                }
              }
            }
          },
          "additionalProperties": false
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
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "MyStruct",
  "type": "object",
  "required": ["myBool", "myNumber"],
  "properties": {
    "myBool": {
      "type": "boolean"
    },
    "myNullableEnum": {
      "default": null,
      "anyOf": [
        {
          "$ref": "#/definitions/MyEnum"
        },
        {
          "type": "null"
        }
      ]
    },
    "myNumber": {
      "type": "integer",
      "format": "int32"
    }
  },
  "additionalProperties": false,
  "definitions": {
    "MyEnum": {
      "anyOf": [
        {
          "type": "string"
        },
        {
          "type": "object",
          "required": ["floats"],
          "properties": {
            "floats": {
              "type": "array",
              "items": {
                "type": "number",
                "format": "float"
              }
            }
          }
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

- `derive` (enabled by default) - provides `#[derive(JsonSchema)]` macro
- `impl_json_schema` - implements `JsonSchema` for Schemars types themselves
- `preserve_order` - keep the order of struct fields in `Schema` and `SchemaObject`
- `raw_value` - implements `JsonSchema` for `serde_json::value::RawValue` (enables the serde_json `raw_value` feature)

Schemars can implement `JsonSchema` on types from several popular crates, enabled via feature flags (dependency versions are shown in brackets):

- `chrono` - [chrono](https://crates.io/crates/chrono) (^0.4)
- `indexmap1` - [indexmap](https://crates.io/crates/indexmap) (^1.2)
- `indexmap2` - [indexmap](https://crates.io/crates/indexmap) (^2.0)
- `either` - [either](https://crates.io/crates/either) (^1.3)
- `uuid08` - [uuid](https://crates.io/crates/uuid) (^0.8)
- `uuid1` - [uuid](https://crates.io/crates/uuid) (^1.0)
- `smallvec` - [smallvec](https://crates.io/crates/smallvec) (^1.0)
- `arrayvec05` - [arrayvec](https://crates.io/crates/arrayvec) (^0.5)
- `arrayvec07` - [arrayvec](https://crates.io/crates/arrayvec) (^0.7)
- `url` - [url](https://crates.io/crates/url) (^2.0)
- `bytes` - [bytes](https://crates.io/crates/bytes) (^1.0)
- `enumset` - [enumset](https://crates.io/crates/enumset) (^1.0)
- `rust_decimal` - [rust_decimal](https://crates.io/crates/rust_decimal) (^1.0)
- `bigdecimal03` - [bigdecimal](https://crates.io/crates/bigdecimal) (^0.3)
- `bigdecimal04` - [bigdecimal](https://crates.io/crates/bigdecimal) (^0.4)
- `smol_str` - [smol_str](https://crates.io/crates/smol_str) (^0.1.17)
- `semver` - [semver](https://crates.io/crates/semver) (^1.0.9)

For example, to implement `JsonSchema` on types from `chrono`, enable it as a feature in the `schemars` dependency in your `Cargo.toml` like so:

```toml
[dependencies]
schemars = { version = "0.8", features = ["chrono"] }
```
