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

#[cfg(test)]
mod tests {
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
}
