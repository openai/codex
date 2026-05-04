use std::collections::BTreeMap;
use std::collections::HashMap;

/// Marker trait for protocol types that can signal experimental usage.
pub trait ExperimentalApi {
    /// Returns a short reason identifier when an experimental method or field is
    /// used, or `None` when the value is entirely stable.
    fn experimental_reason(&self) -> Option<&'static str>;
}

/// Describes an experimental field on a specific type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExperimentalField {
    pub type_name: &'static str,
    pub field_name: &'static str,
    /// Stable identifier returned when this field is used.
    /// Convention: `<method>` for method-level gates or `<method>.<field>` for
    /// field-level gates.
    pub reason: &'static str,
}

inventory::collect!(ExperimentalField);

/// Returns all experimental fields registered across the protocol types.
pub fn experimental_fields() -> Vec<&'static ExperimentalField> {
    inventory::iter::<ExperimentalField>.into_iter().collect()
}

/// Constructs a consistent error message for experimental gating.
pub fn experimental_required_message(reason: &str) -> String {
    format!("{reason} requires experimentalApi capability")
}

impl<T: ExperimentalApi> ExperimentalApi for Option<T> {
    fn experimental_reason(&self) -> Option<&'static str> {
        self.as_ref().and_then(ExperimentalApi::experimental_reason)
    }
}

impl<T: ExperimentalApi> ExperimentalApi for Vec<T> {
    fn experimental_reason(&self) -> Option<&'static str> {
        self.iter().find_map(ExperimentalApi::experimental_reason)
    }
}

impl<K, V: ExperimentalApi, S> ExperimentalApi for HashMap<K, V, S> {
    fn experimental_reason(&self) -> Option<&'static str> {
        self.values().find_map(ExperimentalApi::experimental_reason)
    }
}

impl<K: Ord, V: ExperimentalApi> ExperimentalApi for BTreeMap<K, V> {
    fn experimental_reason(&self) -> Option<&'static str> {
        self.values().find_map(ExperimentalApi::experimental_reason)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::ExperimentalApi as ExperimentalApiTrait;
    use codex_experimental_api_macros::ExperimentalApi;
    use pretty_assertions::assert_eq;

    #[allow(dead_code)]
    #[derive(ExperimentalApi)]
    enum EnumVariantShapes {
        #[experimental("enum/unit")]
        Unit,
        #[experimental("enum/tuple")]
        Tuple(u8),
        #[experimental("enum/named")]
        Named {
            value: u8,
        },
        StableTuple(u8),
    }

    #[allow(dead_code)]
    #[derive(ExperimentalApi)]
    struct NestedFieldShape {
        #[experimental(nested)]
        inner: Option<EnumVariantShapes>,
    }

    #[allow(dead_code)]
    #[derive(ExperimentalApi)]
    struct NestedCollectionShape {
        #[experimental(nested)]
        inners: Vec<EnumVariantShapes>,
    }

    #[allow(dead_code)]
    #[derive(ExperimentalApi)]
    struct NestedMapShape {
        #[experimental(nested)]
        inners: HashMap<String, EnumVariantShapes>,
    }

    #[allow(dead_code)]
    #[derive(ExperimentalApi)]
    struct ExperimentalFieldShape {
        #[experimental("field/optionalCollection")]
        optional_collection: Option<Vec<EnumVariantShapes>>,
    }

    #[test]
    fn derive_supports_all_enum_variant_shapes() {
        assert_eq!(
            ExperimentalApiTrait::experimental_reason(&EnumVariantShapes::Unit),
            Some("enum/unit")
        );
        assert_eq!(
            ExperimentalApiTrait::experimental_reason(&EnumVariantShapes::Tuple(1)),
            Some("enum/tuple")
        );
        assert_eq!(
            ExperimentalApiTrait::experimental_reason(&EnumVariantShapes::Named { value: 1 }),
            Some("enum/named")
        );
        assert_eq!(
            ExperimentalApiTrait::experimental_reason(&EnumVariantShapes::StableTuple(1)),
            None
        );
    }

    #[test]
    fn derive_supports_nested_experimental_fields() {
        assert_eq!(
            ExperimentalApiTrait::experimental_reason(&NestedFieldShape {
                inner: Some(EnumVariantShapes::Named { value: 1 }),
            }),
            Some("enum/named")
        );
        assert_eq!(
            ExperimentalApiTrait::experimental_reason(&NestedFieldShape { inner: None }),
            None
        );
    }

    #[test]
    fn derive_supports_nested_collections() {
        assert_eq!(
            ExperimentalApiTrait::experimental_reason(&NestedCollectionShape {
                inners: vec![
                    EnumVariantShapes::StableTuple(1),
                    EnumVariantShapes::Tuple(2)
                ],
            }),
            Some("enum/tuple")
        );
        assert_eq!(
            ExperimentalApiTrait::experimental_reason(&NestedCollectionShape {
                inners: Vec::new()
            }),
            None
        );
    }

    #[test]
    fn derive_supports_nested_maps() {
        assert_eq!(
            ExperimentalApiTrait::experimental_reason(&NestedMapShape {
                inners: HashMap::from([(
                    "default".to_string(),
                    EnumVariantShapes::Named { value: 1 },
                )]),
            }),
            Some("enum/named")
        );
        assert_eq!(
            ExperimentalApiTrait::experimental_reason(&NestedMapShape {
                inners: HashMap::new(),
            }),
            None
        );
    }

    #[test]
    fn derive_marks_optional_experimental_fields_when_some() {
        assert_eq!(
            ExperimentalApiTrait::experimental_reason(&ExperimentalFieldShape {
                optional_collection: Some(Vec::new()),
            }),
            Some("field/optionalCollection")
        );
        assert_eq!(
            ExperimentalApiTrait::experimental_reason(&ExperimentalFieldShape {
                optional_collection: None,
            }),
            None
        );
    }
}
