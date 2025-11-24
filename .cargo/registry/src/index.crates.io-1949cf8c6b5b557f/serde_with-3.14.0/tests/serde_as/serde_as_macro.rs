use super::*;

/// Test that the [`serde_as`] macro can replace the `_` type and the resulting code compiles.
#[test]
fn test_serde_as_macro_replace_infer_type() {
    #[serde_as]
    #[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
    struct Data {
        #[serde_as(as = "_")]
        a: u32,
        #[serde_as(as = "std::vec::Vec<_>")]
        b: Vec<u32>,
        #[serde_as(as = "Vec<(_, _)>")]
        c: Vec<(u32, String)>,
        #[serde_as(as = "[_; 2]")]
        d: [u32; 2],
        #[serde_as(as = "Box<[_]>")]
        e: Box<[u32]>,
    }

    is_equal(
        Data {
            a: 10,
            b: vec![20, 33],
            c: vec![(40, "Hello".into()), (55, "World".into()), (60, "!".into())],
            d: [70, 88],
            e: vec![99, 100, 110].into_boxed_slice(),
        },
        expect![[r#"
        {
          "a": 10,
          "b": [
            20,
            33
          ],
          "c": [
            [
              40,
              "Hello"
            ],
            [
              55,
              "World"
            ],
            [
              60,
              "!"
            ]
          ],
          "d": [
            70,
            88
          ],
          "e": [
            99,
            100,
            110
          ]
        }"#]],
    );
}

/// Test that the [`serde_as`] macro supports `deserialize_as`
#[test]
fn test_serde_as_macro_deserialize() {
    #[serde_as]
    #[derive(Debug, Eq, PartialEq, Deserialize)]
    struct Data {
        #[serde_as(deserialize_as = "DisplayFromStr")]
        a: u32,
        #[serde_as(deserialize_as = "Vec<DisplayFromStr>")]
        b: Vec<u32>,
        #[serde_as(deserialize_as = "(DisplayFromStr, _)")]
        c: (u32, u32),
    }

    check_deserialization(
        Data {
            a: 10,
            b: vec![20, 33],
            c: (40, 55),
        },
        r#"{
  "a": "10",
  "b": [
    "20",
    "33"
  ],
  "c": [
    "40",
    55
  ]
}"#,
    );
}

/// Test that the [`serde_as`] macro supports `serialize_as`
#[test]
fn test_serde_as_macro_serialize() {
    #[serde_as]
    #[derive(Debug, Eq, PartialEq, Serialize)]
    struct Data {
        #[serde_as(serialize_as = "DisplayFromStr")]
        a: u32,
        #[serde_as(serialize_as = "Vec<DisplayFromStr>")]
        b: Vec<u32>,
        #[serde_as(serialize_as = "(DisplayFromStr, _)")]
        c: (u32, u32),
    }

    check_serialization(
        Data {
            a: 10,
            b: vec![20, 33],
            c: (40, 55),
        },
        expect![[r#"
        {
          "a": "10",
          "b": [
            "20",
            "33"
          ],
          "c": [
            "40",
            55
          ]
        }"#]],
    );
}

/// Test that the [`serde_as`] macro supports `serialize_as` and `deserialize_as`
#[test]
fn test_serde_as_macro_serialize_deserialize() {
    #[serde_as]
    #[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
    struct Data {
        #[serde_as(serialize_as = "DisplayFromStr", deserialize_as = "DisplayFromStr")]
        a: u32,
        #[serde_as(
            serialize_as = "Vec<DisplayFromStr>",
            deserialize_as = "Vec<DisplayFromStr>"
        )]
        b: Vec<u32>,
        #[serde_as(
            serialize_as = "(DisplayFromStr, _)",
            deserialize_as = "(DisplayFromStr, _)"
        )]
        c: (u32, u32),
    }

    is_equal(
        Data {
            a: 10,
            b: vec![20, 33],
            c: (40, 55),
        },
        expect![[r#"
        {
          "a": "10",
          "b": [
            "20",
            "33"
          ],
          "c": [
            "40",
            55
          ]
        }"#]],
    );
}

/// Test that the [`serde_as`] macro works correctly if applied multiple times to a field
#[test]
fn test_serde_as_macro_multiple_field_attributes() {
    #[serde_as]
    #[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
    struct Data {
        #[serde_as(serialize_as = "DisplayFromStr")]
        #[serde_as(deserialize_as = "DisplayFromStr")]
        a: u32,
    }

    is_equal(
        Data { a: 10 },
        expect![[r#"
        {
          "a": "10"
        }"#]],
    );
}

/// Ensure that `serde_as` applies `default` if both the field and the conversion are option.
#[test]
fn test_default_on_option() {
    #[serde_as]
    #[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
    struct Data {
        #[serde_as(as = "Option<DisplayFromStr>")]
        a: Option<u32>,
    }

    is_equal(
        Data { a: None },
        expect![[r#"
          {
            "a": null
          }"#]],
    );
    is_equal(
        Data { a: Some(123) },
        expect![[r#"
          {
            "a": "123"
          }"#]],
    );
    check_deserialization(Data { a: None }, "{}");

    #[serde_as]
    #[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
    struct DataNoDefault {
        #[serde_as(as = "Option<DisplayFromStr>", no_default)]
        a: Option<u32>,
    }

    is_equal(
        DataNoDefault { a: None },
        expect![[r#"
          {
            "a": null
          }"#]],
    );
    is_equal(
        DataNoDefault { a: Some(123) },
        expect![[r#"
          {
            "a": "123"
          }"#]],
    );
    check_error_deserialization::<DataNoDefault>(
        "{}",
        expect!["missing field `a` at line 1 column 2"],
    );

    fn default_555() -> Option<u32> {
        Some(555)
    }

    #[serde_as]
    #[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
    struct DataExplicitDefault {
        #[serde_as(as = "Option<DisplayFromStr>")]
        #[serde(default = "default_555")]
        a: Option<u32>,
    }

    is_equal(
        DataExplicitDefault { a: None },
        expect![[r#"
          {
            "a": null
          }"#]],
    );
    is_equal(
        DataExplicitDefault { a: Some(123) },
        expect![[r#"
          {
            "a": "123"
          }"#]],
    );
    check_deserialization(DataExplicitDefault { a: Some(555) }, "{}");

    #[serde_as]
    #[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
    struct DataString {
        #[serde_as(as = "NoneAsEmptyString")]
        a: Option<String>,
    }

    is_equal(
        DataString { a: None },
        expect![[r#"
            {
              "a": ""
            }"#]],
    );
    is_equal(
        DataString {
            a: Some("123".to_string()),
        },
        expect![[r#"
          {
            "a": "123"
          }"#]],
    );
    check_deserialization(DataString { a: None }, r#"{"a": ""}"#);
    check_deserialization(
        DataString {
            a: Some("555".to_string()),
        },
        r#"{"a": "555"}"#,
    );
    check_error_deserialization::<DataString>(
        "{}",
        expect!["missing field `a` at line 1 column 2"],
    );
}
