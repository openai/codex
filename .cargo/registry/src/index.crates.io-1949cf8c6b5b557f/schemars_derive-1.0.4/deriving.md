# Deriving JsonSchema

The most important trait in Schemars is `JsonSchema`, and the most important function of that trait is `json_schema(...)` which returns a JSON schema describing the type. Implementing this manually on many types would be slow and error-prone, so Schemars includes a derive macro which can implement that trait for you. Any derived implementation of `JsonSchema` should create a schema that describes the JSON representation of the type if it were to be serialized by serde_json.

Usually, all you need to do to use it is to add a `#[derive(JsonSchema)]` attribute to your type:

```rust
use schemars::{JsonSchema, schema_for};

#[derive(JsonSchema, Debug)]
struct Point {
    x: i32,
    y: i32,
}

fn main() {
    let schema = schema_for!(Point);

    let serialized = serde_json::to_string(&schema).unwrap();
    println!("{}", serialized);
}
```

<!-- TODO:
show example output
requirements - when can/can't it be derived
generic params behaviour
-->
