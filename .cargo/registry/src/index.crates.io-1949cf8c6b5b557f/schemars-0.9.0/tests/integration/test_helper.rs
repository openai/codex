use jsonschema::Validator;
use pretty_assertions::assert_eq;
use schemars::{
    generate::{Contract, SchemaSettings},
    JsonSchema, Schema,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use snapbox::{data::DataFormat, IntoJson};
use std::{
    any::type_name, borrow::Borrow, cell::OnceCell, f64, marker::PhantomData, path::Path,
    sync::OnceLock,
};

pub struct TestHelper<T: JsonSchema> {
    settings: SchemaSettings,
    name: String,
    phantom: PhantomData<T>,
    de_schema: Schema,
    ser_schema: Schema,
    de_schema_validator: OnceCell<Validator>,
    ser_schema_validator: OnceCell<Validator>,
    validator: fn(&T) -> bool,
}

impl<T: JsonSchema> TestHelper<T> {
    /// Should be used via the `test!(SomeType)` macro
    pub fn new(name: String, settings: SchemaSettings) -> Self {
        let de_schema = schema_for::<T>(&settings, Contract::Deserialize);
        let ser_schema = schema_for::<T>(&settings, Contract::Serialize);
        Self {
            settings,
            name,
            phantom: PhantomData,
            de_schema,
            ser_schema,
            de_schema_validator: OnceCell::new(),
            ser_schema_validator: OnceCell::new(),
            validator: |_| true,
        }
    }

    /// Should be used via the `test!(value: SomeType)` macro
    pub fn new_for_value(name: String, settings: SchemaSettings, value: T) -> Self
    where
        T: Serialize,
    {
        let de_schema = schema_for_value(&settings, Contract::Deserialize, &value);
        let ser_schema = schema_for_value(&settings, Contract::Serialize, &value);
        Self {
            settings,
            name,
            phantom: PhantomData,
            de_schema,
            ser_schema,
            de_schema_validator: OnceCell::new(),
            ser_schema_validator: OnceCell::new(),
            validator: |_| true,
        }
    }

    pub fn with_validator(&mut self, validator: fn(&T) -> bool) -> &mut Self {
        self.validator = validator;
        self
    }

    /// Checks the generated schema against the saved schema in the snapshots directory, for manual verification of changes.
    ///
    /// Run tests with the SNAPSHOTS env var set to "overwrite" to overwrite any changed snapshots.
    pub fn assert_snapshot(&self) -> &Self {
        let common_path = format!("tests/integration/snapshots/{}.json", self.name);
        let de_path = format!("tests/integration/snapshots/{}.de.json", self.name);
        let ser_path = format!("tests/integration/snapshots/{}.ser.json", self.name);

        if self.de_schema == self.ser_schema {
            snapbox::assert_data_eq!(
                (&self.de_schema).into_json(),
                snapbox::Data::read_from(Path::new(&common_path), Some(DataFormat::Json)).raw()
            );
            _ = std::fs::remove_file(de_path);
            _ = std::fs::remove_file(ser_path);
        } else {
            snapbox::assert_data_eq!(
                (&self.de_schema).into_json(),
                snapbox::Data::read_from(Path::new(&de_path), Some(DataFormat::Json)).raw()
            );
            snapbox::assert_data_eq!(
                (&self.ser_schema).into_json(),
                snapbox::Data::read_from(Path::new(&ser_path), Some(DataFormat::Json)).raw()
            );
            _ = std::fs::remove_file(common_path);
        }

        self
    }

    /// Checks that the schema generated for this type is identical to that of another type.
    pub fn assert_identical<T2: JsonSchema>(&self) -> &Self {
        snapbox::assert_data_eq!(
            (&self.de_schema).into_json(),
            schema_for::<T2>(&self.settings, Contract::Deserialize)
                .into_json()
                .raw()
        );
        snapbox::assert_data_eq!(
            (&self.ser_schema).into_json(),
            schema_for::<T2>(&self.settings, Contract::Serialize)
                .into_json()
                .raw()
        );

        let t = type_name::<T>();
        let t2 = type_name::<T2>();
        assert_eq!(
            T::schema_name(),
            T2::schema_name(),
            "`{t}` and `{t2}` have identical schemas, so should have the same schema_name"
        );

        assert_eq!(
            T::schema_id(),
            T2::schema_id(),
            "`{t}` and `{t2}` have identical schemas, so should have the same schema_id"
        );

        assert_eq!(
            T::inline_schema(),
            T2::inline_schema(),
            "`{t}` and `{t2}` have identical schemas, so should have the same inline_schema"
        );

        self
    }

    pub fn custom(&self, assertion: impl Fn(&Schema, Contract)) {
        assertion(&self.de_schema, Contract::Deserialize);
        assertion(&self.ser_schema, Contract::Serialize);
    }

    fn de_schema_validate(&self, instance: &Value) -> bool {
        self.de_schema_validator
            .get_or_init(|| build_validator(&self.de_schema))
            .is_valid(instance)
    }

    fn ser_schema_validate(&self, instance: &Value) -> bool {
        self.ser_schema_validator
            .get_or_init(|| build_validator(&self.ser_schema))
            .is_valid(instance)
    }
}

fn build_validator(schema: &Schema) -> Validator {
    jsonschema::options()
        .should_validate_formats(true)
        .build(schema.as_value())
        .expect("valid schema")
}

impl<T: JsonSchema + Serialize> TestHelper<T> {
    /// Checks that the "serialize" schema allows the given sample values when serialized to JSON.
    /// If `T implements `DeserializeOwned`, prefer using `assert_allows_ser_roundtrip()`
    pub fn assert_allows_ser_only(&self, samples: impl IntoIterator<Item = T>) -> &Self {
        for sample in samples {
            let json = serde_json::to_value(&sample).unwrap();

            assert!(
                (self.validator)(&sample),
                "invalid test case - attempt to serialize value failing validation: {json}"
            );

            assert!(
                self.ser_schema_validate(&json),
                "serialize schema should allow serialized value: {json}"
            );
        }

        self
    }
}

impl<T: JsonSchema + Serialize + DeserializeOwned> TestHelper<T> {
    /// Checks that the "serialize" schema allows the given sample values when serialized to JSON
    /// and, if the value can then be deserialized, that the "deserialize" schema also allows it.
    pub fn assert_allows_ser_roundtrip(&self, samples: impl IntoIterator<Item = T>) -> &Self {
        for sample in samples {
            let json = serde_json::to_value(&sample).unwrap();

            assert!(
                (self.validator)(&sample),
                "invalid test case - attempt to serialize value failing validation: {json}"
            );

            assert!(
                self.ser_schema_validate(&json),
                "serialize schema should allow serialized value: {json}"
            );

            if T::deserialize(&json).is_ok() {
                assert!(
                    (self.validator)(&sample),
                    "invalid test case - roundtripped value fails validation: {json}"
                );
                assert!(
                    self.de_schema_validate(&json),
                    "deserialize schema should allow value accepted by deserialization: {json}"
                );
            } else {
                assert!(
                    !self.de_schema_validate(&json),
                    "deserialize schema should reject undeserializable value: {json}"
                );
            }
        }

        self
    }

    /// Checks that the "deserialize" schema allow the given sample values, and the "serialize"
    /// schema allows the value obtained from deserializing then re-serializing the sample values
    /// (only for values that can successfully be serialized).
    ///
    /// This is intended for types that have different serialize/deserialize schemas, or when you
    /// want to test specific values that are valid for deserialization but not for serialization.
    pub fn assert_allows_de_roundtrip(
        &self,
        samples: impl IntoIterator<Item = impl Borrow<Value>>,
    ) -> &Self {
        for sample in samples {
            let sample = sample.borrow();
            let Ok(deserialized) = T::deserialize(sample) else {
                panic!(
                    "expected deserialize to succeed for {}: {sample}",
                    type_name::<T>()
                )
            };

            assert!(
                (self.validator)(&deserialized),
                "invalid test case - deserialized value fails validation: {sample}"
            );

            assert!(
                self.de_schema_validate(sample),
                "deserialize schema should allow value accepted by deserialization: {sample}"
            );

            if let Ok(serialized) = serde_json::to_value(&deserialized) {
                assert!(
                    self.ser_schema_validate(&serialized),
                    "serialize schema should allow serialized value: {serialized}"
                );
            }
        }

        self
    }

    /// Checks that the "deserialize" schema allows only the given sample values that successfully
    /// deserialize and pass validation.
    ///
    /// This is intended to be given a range of values (see `arbitrary_values`), allowing limited
    /// fuzzing.
    pub fn assert_matches_de_roundtrip(
        &self,
        samples: impl IntoIterator<Item = impl Borrow<Value>>,
    ) -> &Self {
        for value in samples {
            let value = value.borrow();

            match T::deserialize(value) {
                Ok(deserialized) if (self.validator)(&deserialized) => {
                    assert!(
                        self.de_schema_validate(value),
                        "deserialize schema should allow value accepted by deserialization: {value}"
                    );

                    if let Ok(serialized) = serde_json::to_value(&deserialized) {
                        assert!(
                            self.ser_schema_validate(&serialized),
                            "serialize schema should allow serialized value: {serialized}"
                        );
                    }
                }
                _ => {
                    assert!(
                        !self.de_schema_validate(value),
                        "deserialize schema should reject invalid value: {value}"
                    );

                    // This assertion isn't necessarily valid in the general case but it would be
                    // odd (though not necessarily wrong) for it to fail. If this does ever fail
                    // a case that should be legitimate, then this assert can be removed/weakened.
                    assert!(
                        !self.ser_schema_validate(value),
                        "serialize schema should reject invalid value: {value}"
                    );
                }
            }
        }

        self
    }

    /// Checks that the "deserialize" schema does not allow any of the given sample values.
    ///
    /// While `assert_matches_de_roundtrip()` would also work in this case, `assert_rejects_de()`
    /// has the advantage that it also verifies that the test case itself is actually covering the
    /// failure case as intended.
    pub fn assert_rejects_de(&self, values: impl IntoIterator<Item = impl Borrow<Value>>) -> &Self {
        for value in values {
            let value = value.borrow();

            assert!(
                T::deserialize(value).is_err(),
                "invalid test case - expected deserialize to fail for {}: {value}",
                type_name::<T>()
            );

            assert!(
                !self.de_schema_validate(value),
                "deserialize schema should reject invalid value: {value}"
            );
        }

        self
    }

    /// Checks that neither "serialize" nor "deserialize" schemas allow any of the given sample
    /// values when serialized to JSON due to the values failing validation.
    pub fn assert_rejects_invalid(&self, samples: impl IntoIterator<Item = T>) -> &Self {
        for sample in samples {
            let json = serde_json::to_value(&sample).unwrap();

            assert!(
                !(self.validator)(&sample),
                "invalid test case - serialized value passes validation: {json}"
            );

            assert!(
                !self.de_schema_validate(&json),
                "deserialize schema should reject invalid value: {json}"
            );
            assert!(
                !self.ser_schema_validate(&json),
                "serialize schema should reject invalid value: {json}"
            );
        }

        self
    }

    /// Checks that both the "serialize" and "deserialize" schema allow the type's default value
    /// when serialized to JSON.
    pub fn assert_allows_ser_roundtrip_default(&self) -> &Self
    where
        T: Default,
    {
        self.assert_allows_ser_roundtrip([T::default()])
    }
}

fn schema_for<T: JsonSchema>(base_settings: &SchemaSettings, contract: Contract) -> Schema {
    base_settings
        .clone()
        .with(|s| s.contract = contract)
        .into_generator()
        .into_root_schema_for::<T>()
}

fn schema_for_value(
    base_settings: &SchemaSettings,
    contract: Contract,
    value: impl Serialize,
) -> Schema {
    base_settings
        .clone()
        .with(|s| s.contract = contract)
        .into_generator()
        .into_root_schema_for_value(&value)
        .unwrap()
}

/// Returns an iterator over an selection of arbitrary JSON values.
///
/// This is intended to be used as `test!(...).assert_matches_de_roundtrip(arbitrary_values())`
pub fn arbitrary_values() -> impl Iterator<Item = &'static Value> {
    fn primitives() -> impl Iterator<Item = Value> {
        [
            Value::Null,
            false.into(),
            true.into(),
            0.into(),
            255.into(),
            (-1).into(),
            u64::MAX.into(),
            f64::consts::PI.into(),
            "".into(),
            "0".into(),
            "3E8".into(),
            "\tPâté costs “£1”\0".into(),
            Value::Array(Default::default()),
            Value::Object(Default::default()),
        ]
        .into_iter()
    }

    // TODO once MSRV has reached 1.80, replace this with LazyLock
    static VALUES: OnceLock<Vec<Value>> = OnceLock::new();

    VALUES
        .get_or_init(|| {
            Vec::from_iter(
                primitives()
                    .chain(primitives().map(|p| json!([p])))
                    .chain(primitives().map(|p| json!({"key": p}))),
            )
        })
        .iter()
}

/// Returns an iterator over an selection of arbitrary JSON values, except for value that match
/// the given filter predicate.
///
/// This is to handle known cases of schemas not matching the actual deserialize behaviour.
pub fn arbitrary_values_except(
    filter: impl Fn(&Value) -> bool,
    _reason: &str,
) -> impl Iterator<Item = &'static Value> {
    arbitrary_values().filter(move |v| !filter(v))
}
