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

/// Describes how an experimental enum variant appears on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExperimentalEnumVariantEncoding {
    /// A plain string-literal union arm, such as `"chatgptAuthTokens"`.
    StringLiteral,
    /// A tagged object arm, such as `{ "type": "chatgptAuthTokens", ... }`.
    TaggedObject { tag_name: &'static str },
    /// An externally tagged object arm, such as `{ "reject": { ... } }`.
    ExternallyTaggedObject,
}

/// Describes an experimental enum variant on a specific type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExperimentalEnumVariant {
    pub type_name: &'static str,
    pub serialized_name: &'static str,
    pub reason: &'static str,
    pub encoding: ExperimentalEnumVariantEncoding,
}

inventory::collect!(ExperimentalEnumVariant);

/// Returns all experimental enum variants registered across the protocol
/// types.
pub fn experimental_enum_variants() -> Vec<&'static ExperimentalEnumVariant> {
    inventory::iter::<ExperimentalEnumVariant>
        .into_iter()
        .collect()
}

/// Constructs a consistent error message for experimental gating.
pub fn experimental_required_message(reason: &str) -> String {
    format!("{reason} requires experimentalApi capability")
}

#[cfg(test)]
mod tests {
    use super::ExperimentalApi as ExperimentalApiTrait;
    use super::ExperimentalEnumVariantEncoding;
    use super::experimental_enum_variants;
    use codex_experimental_api_macros::ExperimentalApi;
    use pretty_assertions::assert_eq;
    use serde::Serialize;

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

    #[allow(dead_code)]
    #[derive(ExperimentalApi, Serialize)]
    #[serde(tag = "type", rename_all = "camelCase")]
    enum TaggedEnumVariantShapes {
        Stable,
        #[experimental("tagged/experimental")]
        ExperimentalVariant,
    }

    #[test]
    fn derive_registers_experimental_enum_variants_with_wire_shape_metadata() {
        let variant = experimental_enum_variants()
            .into_iter()
            .find(|variant| variant.reason == "tagged/experimental")
            .expect("tagged experimental variant should be registered");

        assert_eq!(
            *variant,
            crate::experimental_api::ExperimentalEnumVariant {
                type_name: "TaggedEnumVariantShapes",
                serialized_name: "experimentalVariant",
                reason: "tagged/experimental",
                encoding: ExperimentalEnumVariantEncoding::TaggedObject { tag_name: "type" },
            }
        );
    }
}
