# Attributes

You can add attributes to your types to customize Schemars's derived `JsonSchema` implementation.

[Serde](https://serde.rs/) allows setting `#[serde(...)]` attributes which change how types are serialized, and Schemars will generally respect these attributes to ensure that generated schemas will match how the type is serialized by serde_json. `#[serde(...)]` attributes can be overriden using `#[schemars(...)]` attributes, which behave identically (e.g. `#[schemars(rename_all = "camelCase")]`). You may find this useful if you want to change the generated schema without affecting Serde's behaviour, or if you're just not using Serde.

You can also "unset" serde attributes by including them with a `!` prefix in a schemars attribute, which will make schemars ignore the corresponding serde attribute item:

```rust
#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(from = "OtherType")]
// this makes schemars ignore the `from = "OtherType"` from the serde attribute:
#[schemars(!from)]
pub struct MyStruct {
    // ...
}
```

[Validator](https://github.com/Keats/validator) and [Garde](https://github.com/jprochazk/garde) allow setting `#[validate(...)]`/`#[garde(...)]` attributes to restrict valid values of particular fields, many of which will be used by Schemars to generate more accurate schemas. These can also be overridden by `#[schemars(...)]` attributes.

<details open>
<summary style="font-weight: bold">
TABLE OF CONTENTS
</summary>

1. [Supported Serde Attributes](#supported-serde-attributes)
   - [`rename`](#rename)
   - [`rename_all`](#rename_all)
   - [`rename_all_fields`](#rename_all_fields)
   - [`tag` / `content` / `untagged`](#tag)
   - [`default`](#default)
   - [`skip`](#skip)
   - [`skip_serializing`](#skip_serializing)
   - [`skip_deserializing`](#skip_deserializing)
   - [`flatten`](#flatten)
   - [`with`](#with)
   - [`bound`](#bound)
1. [Supported Validator/Garde Attributes](#supported-validatorgarde-attributes)
   - [`email` / `url` / `ip` / `ipv4` / `ipv6`](#formats)
   - [`length`](#length)
   - [`range`](#range)
   - [`regex` / `pattern`](#regex)
   - [`contains`](#contains)
   - [`required`](#required)
   - [`inner`](#inner)
1. [Other Attributes](#other-attributes)
   - [`schema_with`](#schema_with)
   - [`title` / `description`](#title-description)
   - [`example`](#example)
   - [`deprecated`](#deprecated)
   - [`inline`](#inline)
   - [`crate`](#crate)
   - [`extend`](#extend)
   - [`transform`](#transform)
   - [Doc Comments (`doc`)](#doc)

</details>

## Supported Serde Attributes

<div class="indented">

<h3 id="rename">

`#[serde(rename = "name")]` / `#[schemars(rename = "name")]`

</h3>

Set on a struct, enum, field or variant to use the given name in the generated schema instead of the Rust name. When used on a struct or enum, the given name will be used as the title for root schemas, and the key within the root's `$defs` property for subschemas.

If set on a struct or enum with generic type parameters, then the given name may contain them enclosed in curly braces (e.g. `{T}`) and they will be replaced with the concrete type names when the schema is generated.

Serde docs: [container](https://serde.rs/container-attrs.html#rename) / [variant](https://serde.rs/variant-attrs.html#rename) / [field](https://serde.rs/field-attrs.html#rename)

<h3 id="rename_all">

`#[serde(rename_all = "...")]` / `#[schemars(rename_all = "...")]`

</h3>

Set on a struct, enum or variant to rename all fields according to the given case convention (see the Serde docs for details).

Serde docs: [container](https://serde.rs/container-attrs.html#rename_all) / [variant](https://serde.rs/variant-attrs.html#rename_all)

<h3 id="rename_all_fields">

`#[serde(rename_all_fields = "...")]` / `#[schemars(rename_all_fields = "...")]`

</h3>

Set on an enum to rename all fields of all struct-style variants according to the given case convention (see the Serde docs for details).

Serde docs: [container](https://serde.rs/container-attrs.html#rename_all)

<h3 id="tag" style="line-height: 1.5">

`#[serde(tag = "type")]` / `#[schemars(tag = "type")]` <br />
`#[serde(tag = "t", content = "c")]` / `#[schemars(tag = "t", content = "c")]` <br />
`#[serde(untagged)]` / `#[schemars(untagged)]`

</h3>

Set on an enum to generate the schema for the [internally tagged](https://serde.rs/enum-representations.html#internally-tagged), [adjacently tagged](https://serde.rs/enum-representations.html#adjacently-tagged), or [untagged](https://serde.rs/enum-representations.html#untagged) representation of this enum.

`#[serde(untagged)]`/`#[schemars(untagged)]` can also be set on an individual variant of a tagged enum to treat just that variant as untagged.

Serde docs: [`tag`](https://serde.rs/container-attrs.html#tag) / [`tag`+`content`](https://serde.rs/container-attrs.html#tag--content) / [`untagged` (enum)](https://serde.rs/container-attrs.html#untagged) / [`untagged` (variant)](https://serde.rs/variant-attrs.html#untagged)

<h3 id="default">

`#[serde(default)]` / `#[schemars(default)]` / `#[serde(default = "path")]` / `#[schemars(default = "path")]`

</h3>

Set on a struct or field to give fields a default value, which excludes them from the schema's `required` properties. The default will also be set on the field's schema's `default` property, unless it is skipped by a [`skip_serializing_if`](https://serde.rs/field-attrs.html#skip_serializing_if) attribute on the field. Any [`serialize_with`](https://serde.rs/field-attrs.html#serialize_with) or [`with`](https://serde.rs/field-attrs.html#with) attribute set on the field will be used to serialize the default value.

Serde docs: [container](https://serde.rs/container-attrs.html#default) / [field](https://serde.rs/field-attrs.html#default)

<h3 id="skip">

`#[serde(skip)]` / `#[schemars(skip)]`

</h3>

Set on a variant or field to prevent it from appearing in any generated schema.

Serde docs: [variant](https://serde.rs/variant-attrs.html#skip) / [field](https://serde.rs/field-attrs.html#skip)

<h3 id="skip_serializing">

`#[serde(skip_serializing)]` / `#[schemars(skip_serializing)]`

</h3>

Set on a field of a (non-tuple) struct to set the `writeOnly` property on that field's schema. Serde also allows this attribute on variants or tuple struct fields, but this will have no effect on generated schemas.

Serde docs: [field](https://serde.rs/field-attrs.html#skip_deserializing)

<h3 id="skip_deserializing">

`#[serde(skip_deserializing)]` / `#[schemars(skip_deserializing)]`

</h3>

Set on a variant or field. When set on a field of a (non-tuple) struct, that field's schema will have the `readOnly` property set. When set on a variant or tuple struct field Schemars will treat this the same as a [`skip`](#skip) attribute.

Serde docs: [variant](https://serde.rs/variant-attrs.html#skip_deserializing) / [field](https://serde.rs/field-attrs.html#skip_deserializing)

<h3 id="flatten">

`#[serde(flatten)]` / `#[schemars(flatten)]`

</h3>

Set on a field to include that field's contents as though they belonged to the field's container.

Serde docs: [field](https://serde.rs/field-attrs.html#flatten)

<h3 id="with">

`#[serde(with = "Type")]` / `#[schemars(with = "Type")]`

</h3>

Set on a container, variant or field to generate its schema as the given type instead of its actual type. Serde allows the `with` attribute to refer to any module path, but Schemars requires this to be an actual type which implements `JsonSchema`.

Serde does not allow this attribute to be set on containers, but this is allowed in `#[schemars(...)]` attributes.

If the given type has any required generic type parameters, then they must all be explicitly specified in this attribute. Serde frequently allows you to omit them as it can make use of type inference, but unfortunately this is not possible with Schemars. For example, `with = "Vec::<i32>"` will work, but `with = "Vec"` and `with = "Vec::<_>"` will not.

Serde docs: [from](https://serde.rs/container-attrs.html#from) / [try_from](https://serde.rs/container-attrs.html#try_from)

<h3 id="from">

`#[serde(from = "Type")]` / `#[schemars(from = "Type")]`<br />
`#[serde(try_from = "Type")]` / `#[schemars(try_from = "Type")]`

</h3>

Set on a container to generate its [**deserialize** schema](https://graham.cool/schemars/generating/#serialize-vs-deserialize-contract) as the given type instead of its actual type. Schemars treats the `from`/`try_from` attributes identically.

Serde docs: [into](https://serde.rs/container-attrs.html#into)

<h3 id="into">

`#[serde(into = "Type")]` / `#[schemars(into = "Type")]`

</h3>

Set on a container to generate its [**serialize** schema](https://graham.cool/schemars/generating/#serialize-vs-deserialize-contract) as the given type instead of its actual type. Schemars treats the `from`/`try_from` attributes identically.

<h3 id="deny_unknown_fields">

`#[serde(deny_unknown_fields)]` / `#[schemars(deny_unknown_fields)]`

</h3>

Setting this on a container will set the `additionalProperties` keyword on generated schemas to `false` to show that any extra properties are explicitly disallowed.

Serde docs: [container](https://serde.rs/container-attrs.html#deny_unknown_fields)

<h3 id="transparent">

`#[serde(transparent)]` / `#[schemars(transparent)]`

</h3>

Set on a newtype struct or a braced struct with one field to make the struct's generated schema exactly the same as that of the single field's.

Serde docs: [container](https://serde.rs/container-attrs.html#transparent)

<h3 id="bound">

`#[schemars(bound = "...")]`

</h3>

Where-clause for the JsonSchema impl. This replaces any trait bounds inferred by schemars. Schemars does **not** use trait bounds from `#[serde(bound)]` attributes.

Serde docs: [container](https://serde.rs/container-attrs.html#bound)

</div>

## Supported Validator/Garde Attributes

<div class="indented">

<h3 id="formats">

`#[validate(email)]` / `#[garde(email)]` / `#[schemars(email)]`<br />
`#[validate(url)]` / `#[garde(url)]`/ `#[schemars(url)]`<br />
`#[garde(ip)]`/ `#[schemars(ip)]`<br />
`#[garde(ipv4)]`/ `#[schemars(ipv4)]`<br />
`#[garde(ipv6)]`/ `#[schemars(ip)v6]`<br />

</h3>

Sets the schema's `format` to `email`/`uri`/`ip`/`ipv4`/`ipv6`, as appropriate. Only one of these attributes may be present on a single field.

Validator docs: [email](https://github.com/Keats/validator#email) / [url](https://github.com/Keats/validator#url)

<h3 id="length">

`#[validate(length(min = 1, max = 10))]` / `#[garde(length(min = 1, max = 10))]` / `#[schemars(length(min = 1, max = 10))]`<br />
`#[validate(length(equal = 10))]` / `#[garde(length(equal = 10))]` / `#[schemars(length(equal = 10))]`

</h3>

Sets the `minLength`/`maxLength` properties for string schemas, or the `minItems`/`maxItems` properties for array schemas.

Validator docs: [length](https://github.com/Keats/validator#length)

<h3 id="range">

`#[validate(range(min = 1, max = 10))]` / `#[garde(range(min = 1, max = 10))]` / `#[schemars(range(min = 1, max = 10))]`

</h3>

Sets the `minimum`/`maximum` properties for number schemas.

Validator docs: [range](https://github.com/Keats/validator#range)

<h3 id="regex">

`#[validate(regex(path = *static_regex)]`<br />
`#[schemars(regex(pattern = r"^\d+$"))]` / `#[schemars(regex(pattern = *static_regex))]`<br />
`#[garde(pattern(r"^\d+$")]` / `#[schemars(pattern(r"^\d+$")]`/ `#[schemars(pattern(*static_regex)]`

</h3>

Sets the `pattern` property for string schemas. The `static_regex` will typically refer to a [`Regex`](https://docs.rs/regex/*/regex/struct.Regex.html) instance, but Schemars allows it to be any value with a `to_string()` method.

`regex(pattern = ...)` is a Schemars extension, and not currently supported by the Validator crate. When using this form (or the Garde-style `pattern` attribute), you may want to use a `r"raw string literal"` so that `\\` characters in the regex pattern are not interpreted as escape sequences in the string. Using the `path = ...` form is not allowed in a `#[schemars(...)]` attribute.

Validator docs: [regex](https://github.com/Keats/validator#regex)

<h3 id="contains">

`#[validate(contains(pattern = "string"))]` / `#[schemars(contains(pattern = "string"))]`<br />
`#[garde(contains("string"))]` / `#[schemars(contains("string"))]`

</h3>

For string schemas, sets the `pattern` property to the given value, with any regex special characters escaped.

Validator docs: [contains](https://github.com/Keats/validator#contains)

<h3 id="required">

`#[validate(required)]` / `#[garde(required)]` / `#[schemars(required)]`<br />

</h3>

When set on an `Option<T>` field, this will create a schemas as though the field were a `T`.

Validator docs: [required](https://github.com/Keats/validator#required)

</div>

<h3 id="inner">

`#[garde(inner(...))]` / `#[schemars(inner(...))]`

</h3>

Sets properties specified by [validation attributes](#supported-validatorgarde-attributes) on items of an array schema. For example:

```rust
struct Struct {
    #[schemars(inner(url, pattern("^https://")))]
    urls: Vec<String>,
}
```

Garde docs: [Inner type validation](https://github.com/jprochazk/garde?tab=readme-ov-file#inner-type-validation)

## Other Attributes

<h3 id="schema_with">

`#[schemars(schema_with = "some::function")]`

</h3>

Set on a variant or field to generate this field's schema using the given function. This function must be callable as `fn(&mut schemars::SchemaGenerator) -> schemars::schema::Schema`.

<h3 id="title-description">

`#[schemars(title = "Some title", description = "Some description")]`

</h3>

Set on a container, variant or field to set the generated schema's `title` and/or `description`. If present, these will be used instead of values from any [`doc` comments/attributes](#doc).

<h3 id="example">

`#[schemars(example = value)]`

</h3>

Set on a container, variant or field to include the given value in the generated schema's `examples`. The value can be any type that implements serde's `Serialize` trait - it does not need to be the same type as the attached struct/field. This attribute can be repeated to specify multiple examples.

In previous versions of schemars, the value had to be a string literal identifying a defined function that would be called to return the actual example value (similar to the [`default`](#default) attribute). To avoid the new attribute behaviour from silently breaking old consumers, string literals consisting of a single word (e.g. `#[schemars(example = "my_fn")]`) or a path (e.g. `#[schemars(example = "my_mod::my_fn")]`) are currently disallowed. This restriction may be relaxed in a future version of schemars, but for now if you want to include such a string as the literal example value, this can be done by borrowing the value, e.g. `#[schemars(example = &"my_fn")]`. If you instead want to call a function to get the example value (mirrorring the old behaviour), you must use an explicit function call expression, e.g. `#[schemars(example = my_fn())]`.

Alternatively, to directly set multiple examples without repeating `example = ...` attribute, you can instead use the [`extend`](#extend) attribute, e.g. `#[schemars(extend("examples" = [1, 2, 3]))]`.

<h3 id="deprecated">

`#[deprecated]`

</h3>

Set the Rust built-in [`deprecated`](https://doc.rust-lang.org/edition-guide/rust-2018/the-compiler/an-attribute-for-deprecation.html) attribute on a struct, enum, field or variant to set the generated schema's `deprecated` keyword to `true`.

<h3 id="inline">

`#[schemars(inline)]`

</h3>

Set the return value of [`inline_schema`](trait.JsonSchema.html#method.inline_schema) to `true` to include JSON schemas generated for this type directly in parent schemas, rather than being re-used where possible using the `$ref` keyword.

<h3 id="crate">

`#[schemars(crate = "other_crate::schemars")]`

</h3>

Set the path to the schemars crate instance the generated code should depend on. This is mostly useful for other crates that depend on schemars in their macros.

<h3 id="extend">

`#[schemars(extend("key" = value))]`

</h3>

Set on a container, variant or field to add properties (or replace existing properties) in a generated schema. This can contain multiple key/value pairs and/or be specified multiple times, as long as each key is unique.

The key must be a quoted string, and the value can be any expression that produces a type implementing `serde::Serialize`. The value can also be a JSON literal which can interpolate other values.

```plaintext
#[derive(JsonSchema)]
#[schemars(extend("simple" = "string value", "complex" = {"array": [1, 2, 3]}))]
struct Struct;
```

<h3 id="transform">

`#[schemars(transform = some::transform)]`

</h3>

Set on a container, variant or field to run a `schemars::transform::Transform` against the generated schema. This can be specified multiple times to run multiple transforms.

The `Transform` trait is implemented on functions with the signature `fn(&mut Schema) -> ()`, allowing you to do this:

```rust
fn my_transform(schema: &mut Schema) {
   todo!()
}

#[derive(JsonSchema)]
#[schemars(transform = my_transform)]
struct Struct;
```

<h3 id="doc">

Doc Comments (`#[doc = "..."]`)

</h3>

If a struct, variant or field has any [doc comments](https://doc.rust-lang.org/stable/rust-by-example/meta/doc.html#doc-comments) (or [`doc` attributes](https://doc.rust-lang.org/rustdoc/the-doc-attribute.html)), then these will be used as the generated schema's `description`. If the first line is an ATX-style markdown heading (i.e. it begins with a # character), then it will be used as the schema's `title`, and the remaining lines will be the `description`.
