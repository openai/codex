# De/Serialize Transformations Available

This page lists the transformations implemented in this crate and supported by `serde_as`.

1. [Base64 encode bytes](#base64-encode-bytes)
2. [Big Array support](#big-array-support)
3. [`bool` from integer](#bool-from-integer)
4. [Borrow from the input for `Cow` type](#borrow-from-the-input-for-cow-type)
5. [`Bytes` with more efficiency](#bytes-with-more-efficiency)
6. [Convert to an intermediate type using `Into`](#convert-to-an-intermediate-type-using-into)
7. [Convert to an intermediate type using `TryInto`](#convert-to-an-intermediate-type-using-tryinto)
8. [`Default` from `null`](#default-from-null)
9. [De/Serialize into `Vec`, ignoring errors](#deserialize-into-vec-ignoring-errors)
10. [De/Serialize into a map, ignoring errors](#deserialize-into-a-map-ignoring-errors)
11. [De/Serialize with `FromStr` and `Display`](#deserialize-with-fromstr-and-display)
12. [`Duration` as seconds](#duration-as-seconds)
13. [Hex encode bytes](#hex-encode-bytes)
14. [Ignore deserialization errors](#ignore-deserialization-errors)
15. [`Maps` to `Vec` of enums](#maps-to-vec-of-enums)
16. [`Maps` to `Vec` of tuples](#maps-to-vec-of-tuples)
17. [`NaiveDateTime` like UTC timestamp](#naivedatetime-like-utc-timestamp)
18. [`None` as empty `String`](#none-as-empty-string)
19. [One or many elements into `Vec`](#one-or-many-elements-into-vec)
20. [Overwrite existing set values](#overwrite-existing-set-values)
21. [Pick first successful deserialization](#pick-first-successful-deserialization)
22. [Prefer the first map key when duplicates exist](#prefer-the-first-map-key-when-duplicates-exist)
23. [Prevent duplicate map keys](#prevent-duplicate-map-keys)
24. [Prevent duplicate set values](#prevent-duplicate-set-values)
25. [Struct fields as map keys](#struct-fields-as-map-keys)
26. [Timestamps as seconds since UNIX epoch](#timestamps-as-seconds-since-unix-epoch)
27. [Value into JSON String](#value-into-json-string)
28. [`Vec` of tuples to `Maps`](#vec-of-tuples-to-maps)
29. [Well-known time formats for `OffsetDateTime`](#well-known-time-formats-for-offsetdatetime)
30. [De/Serialize depending on `De/Serializer::is_human_readable`](#deserialize-depending-on-deserializeris_human_readable)

## Base64 encode bytes

[`Base64`]

Requires the `base64` feature.
The character set and padding behavior can be configured.

```ignore
// Rust
#[serde_as(as = "serde_with::base64::Base64")]
value: Vec<u8>,
#[serde_as(as = "Base64<Bcrypt, Unpadded>")]
bcrypt_unpadded: Vec<u8>,

// JSON
"value": "SGVsbG8gV29ybGQ=",
"bcrypt_unpadded": "QETqZE6eT07wZEO",
```

## Big Array support

Support for arrays of arbitrary size.

```ignore
// Rust
#[serde_as(as = "[[_; 64]; 33]")]
value: [[u8; 64]; 33],

// JSON
"value": [[0,0,0,0,0,...], [0,0,0,...], ...],
```

## `bool` from integer

Deserialize an integer and convert it into a `bool`.
[`BoolFromInt<Strict>`] (default) deserializes 0 to `false` and `1` to `true`, other numbers are errors.
[`BoolFromInt<Flexible>`] deserializes any non-zero as `true`.
Serialization only emits 0/1.

```ignore
// Rust
#[serde_as(as = "BoolFromInt")] // BoolFromInt<Strict>
b: bool,

// JSON
"b": 1,
```

## Borrow from the input for `Cow` type

The types `Cow<'_, str>`, `Cow<'_, [u8]>`, or `Cow<'_, [u8; N]>` can borrow from the input, avoiding extra copies.

```ignore
// Rust
#[serde_as(as = "BorrowCow")]
value: Cow<'a, str>,

// JSON
"value": "foobar",
```

## `Bytes` with more efficiency

[`Bytes`]

More efficient serialization for byte slices and similar.

```ignore
// Rust
#[serde_as(as = "Bytes")]
value: Vec<u8>,

// JSON
"value": [0, 1, 2, 3, ...],
```

## Convert to an intermediate type using `Into`

[`FromInto`]

```ignore
// Rust
#[serde_as(as = "FromInto<(u8, u8, u8)>")]
value: Rgb,

impl From<(u8, u8, u8)> for Rgb { ... }
impl From<Rgb> for (u8, u8, u8) { ... }

// JSON
"value": [128, 64, 32],
```

## Convert to an intermediate type using `TryInto`

[`TryFromInto`]

```ignore
// Rust
#[serde_as(as = "TryFromInto<i8>")]
value: u8,

// JSON
"value": 127,
```

## `Default` from `null`

[`DefaultOnNull`]

```ignore
// Rust
#[serde_as(as = "DefaultOnNull")]
value: u32,
#[serde_as(as = "DefaultOnNull<DisplayFromStr>")]
value2: u32,

// JSON
"value": 123,
"value2": "999",

// Deserializes null into the Default value, i.e.,
null => 0
```

## De/Serialize into `Vec`, ignoring errors

[`VecSkipError`]

For formats with heterogeneously typed sequences, we can collect only the deserializable elements.
This is also useful for unknown enum variants.

```ignore
#[derive(serde::Deserialize)]
enum Color {
    Red,
    Green,
    Blue,
}

// JSON
"colors": ["Blue", "Yellow", "Green"],

// Rust
#[serde_as(as = "VecSkipError<_>")]
colors: Vec<Color>,

// => vec![Blue, Green]
```

## De/Serialize into a map, ignoring errors

[`MapSkipError`]

For formats with heterogeneously typed maps, we can collect only the elements where both key and value are deserializable.
This is also useful in conjunction to `#[serde(flatten)]` to ignore some entries when capturing additional fields.

```ignore
// JSON
"value": {"0": "v0", "5": "v5", "str": "str", "10": 2},

// Rust
#[serde_as(as = "MapSkipError<DisplayFromStr, _>")]
value: BTreeMap<u32, String>,

// Only deserializes entries with a numerical key and a string value, i.e.,
{0 => "v0", 5 => "v5"}
```

## De/Serialize with `FromStr` and `Display`

Useful if a type implements `FromStr` / `Display` but not `Deserialize` / `Serialize`.

[`DisplayFromStr`]

```ignore
// Rust
#[serde_as(as = "serde_with::DisplayFromStr")]
value: u128,
#[serde_as(as = "serde_with::DisplayFromStr")]
mime: mime::Mime,

// JSON
"value": "340282366920938463463374607431768211455",
"mime": "text/*",
```

## `Duration` as seconds

[`DurationSeconds`]

```ignore
// Rust
#[serde_as(as = "serde_with::DurationSeconds<u64>")]
value: Duration,

// JSON
"value": 86400,
```

[`DurationSecondsWithFrac`] supports sub-second precision:

```ignore
// Rust
#[serde_as(as = "serde_with::DurationSecondsWithFrac<f64>")]
value: Duration,

// JSON
"value": 1.234,
```

Different serialization formats are possible:

```ignore
// Rust
#[serde_as(as = "serde_with::DurationSecondsWithFrac<String>")]
value: Duration,

// JSON
"value": "1.234",
```

The same conversions are also implemented for [`chrono::Duration`] with the `chrono` feature.

The same conversions are also implemented for [`time::Duration`] with the `time_0_3` feature.

## Hex encode bytes

[`Hex`]

Requires the `hex` feature.
The hex string can use upper- and lowercase characters.

```ignore
// Rust
#[serde_as(as = "serde_with::hex::Hex")]
lowercase: Vec<u8>,
#[serde_as(as = "serde_with::hex::Hex<serde_with::formats::Uppercase>")]
uppercase: Vec<u8>,

// JSON
"lowercase": "deadbeef",
"uppercase": "DEADBEEF",
```

## Ignore deserialization errors

Check the documentation for [`DefaultOnError`].

## `Maps` to `Vec` of enums

[`EnumMap`]

Combine multiple enum values into a single map.
The key is the enum variant name, and the value is the variant value.
This only works with [*externally tagged*] enums, the default enum representation.
Other forms cannot be supported.

```ignore
enum EnumValue {
    Int(i32),
    String(String),
    Unit,
    Tuple(i32, String),
    Struct {
        a: i32,
        b: String,
    },
}

// Rust
struct VecEnumValues (
    #[serde_as(as = "EnumMap")]
    Vec<EnumValue>,
);

VecEnumValues(vec![
    EnumValue::Int(123),
    EnumValue::String("Foo".to_string()),
    EnumValue::Unit,
    EnumValue::Tuple(1, "Bar".to_string()),
    EnumValue::Struct {
        a: 666,
        b: "Baz".to_string(),
    },
])

// JSON
{
  "Int": 123,
  "String": "Foo",
  "Unit": null,
  "Tuple": [
    1,
    "Bar",
  ],
  "Struct": {
    "a": 666,
    "b": "Baz",
  }
}
```

[*externally tagged*]: https://serde.rs/enum-representations.html#externally-tagged

## `Maps` to `Vec` of tuples

```ignore
// Rust
#[serde_as(as = "Seq<(_, _)>")] // also works with Vec
value: HashMap<String, u32>, // also works with other maps like BTreeMap or IndexMap

// JSON
"value": [
    ["hello", 1],
    ["world", 2]
],
```

The [inverse operation](#vec-of-tuples-to-maps) is also available.

## `NaiveDateTime` like UTC timestamp

Requires the `chrono` feature.

```ignore
// Rust
#[serde_as(as = "chrono::DateTime<chrono::Utc>")]
value: chrono::NaiveDateTime,

// JSON
"value": "1994-11-05T08:15:30Z",
                             ^ Pretend DateTime is UTC
```

## `None` as empty `String`

[`NoneAsEmptyString`]

```ignore
// Rust
#[serde_as(as = "serde_with::NoneAsEmptyString")]
value: Option<String>,

// JSON
"value": "", // converts to None

"value": "Hello World!", // converts to Some
```

## One or many elements into `Vec`

[`OneOrMany`]

```ignore
// Rust
#[serde_as(as = "serde_with::OneOrMany<_>")]
value: Vec<String>,

// JSON
"value": "", // Deserializes single elements

"value": ["Hello", "World!"], // or lists of many
```

## Overwrite existing set values

[`SetLastValueWins`]

serdes default behavior for sets is to take the first value, when multiple "equal" values are inserted into a set.
This changes the logic to prefer the last value.

## Pick first successful deserialization

[`PickFirst`]

```ignore
// Rust
#[serde_as(as = "serde_with::PickFirst<(_, serde_with::DisplayFromStr)>")]
value: u32,

// JSON
// serialize into
"value": 666,
// deserialize from either
"value": 666,
"value": "666",
```

## Prefer the first map key when duplicates exist

[`MapFirstKeyWins`]

Serde's default behavior is to take the last key-value combination, if multiple "equal" keys exist.
This changes the logic to instead prefer the first found key-value combination.

## Prevent duplicate map keys

[`MapPreventDuplicates`]

Error during deserialization, when duplicate map keys are detected.

## Prevent duplicate set values

[`SetPreventDuplicates`]

Error during deserialization, when duplicate set values are detected.

## Struct fields as map keys

[`KeyValueMap`]

This conversion is possible for structs and maps, using the `$key$` field.
Tuples, tuple structs, and sequences are supported by turning the first value into the map key.

Each of the `SimpleStruct`s

```ignore
// Somewhere there is a collection:
// #[serde_as(as = "KeyValueMap<_>")]
// Vec<SimpleStruct>,

#[derive(Serialize, Deserialize)]
struct SimpleStruct {
    b: bool,
    // The field named `$key$` will become the map key
    #[serde(rename = "$key$")]
    id: String,
    i: i32,
}
```

will turn into a JSON snippet like this.

```json
"id-0000": {
  "b": false,
  "i": 123
},
```

## Timestamps as seconds since UNIX epoch

[`TimestampSeconds`]

```ignore
// Rust
#[serde_as(as = "serde_with::TimestampSeconds<i64>")]
value: SystemTime,

// JSON
"value": 86400,
```

[`TimestampSecondsWithFrac`] supports sub-second precision:

```ignore
// Rust
#[serde_as(as = "serde_with::TimestampSecondsWithFrac<f64>")]
value: SystemTime,

// JSON
"value": 1.234,
```

Different serialization formats are possible:

```ignore
// Rust
#[serde_as(as = "serde_with::TimestampSecondsWithFrac<String>")]
value: SystemTime,

// JSON
"value": "1.234",
```

The same conversions are also implemented for [`chrono::DateTime<Utc>`], [`chrono::DateTime<Local>`], and [`chrono::NaiveDateTime`] with the `chrono` feature.

The conversions are available for [`time::OffsetDateTime`] and [`time::PrimitiveDateTime`] with the `time_0_3` feature enabled.

## Value into JSON String

Some JSON APIs are weird and return a JSON encoded string in a JSON response

[`JsonString`]

Requires the `json` feature.

```ignore
// Rust
#[derive(Deserialize, Serialize)]
struct OtherStruct {
    value: usize,
}

#[serde_as(as = "serde_with::json::JsonString")]
value: OtherStruct,

// JSON
"value": "{\"value\":5}",
```

```ignore
#[serde_as(as = "JsonString<Vec<(JsonString, _)>>")]
value: BTreeMap<[u8; 2], u32>,

// JSON
{"value":"[[\"[1,2]\",3],[\"[4,5]\",6]]"}
```

## `Vec` of tuples to `Maps`

```ignore
// Rust
#[serde_as(as = "Map<_, _>")] // also works with BTreeMap and HashMap
value: Vec<(String, u32)>,

// JSON
"value": {
    "hello": 1,
    "world": 2
},
```

This operation is also available for other sequence types.
This includes `BinaryHeap<(K, V)>`, `BTreeSet<(K, V)>`, `HashSet<(K, V)>`, `LinkedList<(K, V)>`, `VecDeque<(K, V)>`, `Option<(K, V)>` and `[(K, V); N]` for all sizes of N.

The [inverse operation](#maps-to-vec-of-tuples) is also available.

## Well-known time formats for `OffsetDateTime`

[`time::OffsetDateTime`] can be serialized in string format in different well-known formats.
Three formats are supported, [`time::format_description::well_known::Rfc2822`], [`time::format_description::well_known::Rfc3339`], and [`time::format_description::well_known::Iso8601`].

```ignore
// Rust
#[serde_as(as = "time::format_description::well_known::Rfc2822")]
rfc_2822: OffsetDateTime,
#[serde_as(as = "time::format_description::well_known::Rfc3339")]
rfc_3339: OffsetDateTime,
#[serde_as(as = "time::format_description::well_known::Iso8601<Config>")]
iso_8601: OffsetDateTime,

// JSON
"rfc_2822": "Fri, 21 Nov 1997 09:55:06 -0600",
"rfc_3339": "1997-11-21T09:55:06-06:00",
"iso_8061": "1997-11-21T09:55:06-06:00",
```

These conversions are available with the `time_0_3` feature flag.

## De/Serialize depending on `De/Serializer::is_human_readable`

Used to specify different transformations for text-based and binary formats.

[`IfIsHumanReadable`]

```ignore
// Rust
#[serde_as(as = "serde_with::IfIsHumanReadable<serde_with::DisplayFromStr>")]
value: u128,

// JSON
"value": "340282366920938463463374607431768211455",
```

[`Base64`]: crate::base64::Base64
[`BoolFromInt<Flexible>`]: crate::BoolFromInt
[`BoolFromInt<Strict>`]: crate::BoolFromInt
[`Bytes`]: crate::Bytes
[`chrono::DateTime<Local>`]: chrono_0_4::DateTime
[`chrono::DateTime<Utc>`]: chrono_0_4::DateTime
[`chrono::Duration`]: chrono_0_4::Duration
[`chrono::NaiveDateTime`]: chrono_0_4::NaiveDateTime
[`DefaultOnError`]: crate::DefaultOnError
[`DefaultOnNull`]: crate::DefaultOnNull
[`DisplayFromStr`]: crate::DisplayFromStr
[`DurationSeconds`]: crate::DurationSeconds
[`DurationSecondsWithFrac`]: crate::DurationSecondsWithFrac
[`EnumMap`]: crate::EnumMap
[`FromInto`]: crate::FromInto
[`Hex`]: crate::hex::Hex
[`IfIsHumanReadable`]: crate::IfIsHumanReadable
[`JsonString`]: crate::json::JsonString
[`KeyValueMap`]: crate::KeyValueMap
[`MapFirstKeyWins`]: crate::MapFirstKeyWins
[`MapPreventDuplicates`]: crate::MapPreventDuplicates
[`NoneAsEmptyString`]: crate::NoneAsEmptyString
[`OneOrMany`]: crate::OneOrMany
[`PickFirst`]: crate::PickFirst
[`SetLastValueWins`]: crate::SetLastValueWins
[`SetPreventDuplicates`]: crate::SetPreventDuplicates
[`time::Duration`]: time_0_3::Duration
[`time::format_description::well_known::Iso8601`]: time_0_3::format_description::well_known::Iso8601
[`time::format_description::well_known::Rfc2822`]: time_0_3::format_description::well_known::Rfc2822
[`time::format_description::well_known::Rfc3339`]: time_0_3::format_description::well_known::Rfc3339
[`time::OffsetDateTime`]: time_0_3::OffsetDateTime
[`time::PrimitiveDateTime`]: time_0_3::PrimitiveDateTime
[`TimestampSeconds`]: crate::TimestampSeconds
[`TimestampSecondsWithFrac`]: crate::TimestampSecondsWithFrac
[`TryFromInto`]: crate::TryFromInto
[`VecSkipError`]: crate::VecSkipError
[`MapSkipError`]: crate::MapSkipError
