//! Type-safe schema definitions for MCP elicitation requests.
//!
//! This module provides strongly-typed schema definitions for elicitation requests
//! that comply with the MCP 2025-06-18 specification. Elicitation schemas must be
//! objects with primitive-typed properties.
//!
//! # Example
//!
//! ```rust
//! use rmcp::model::*;
//!
//! let schema = ElicitationSchema::builder()
//!     .required_email("email")
//!     .required_integer("age", 0, 150)
//!     .optional_bool("newsletter", false)
//!     .build();
//! ```

use std::{borrow::Cow, collections::BTreeMap, marker::PhantomData};

use serde::{Deserialize, Serialize};

use crate::{const_string, model::ConstString};

// =============================================================================
// CONST TYPES FOR JSON SCHEMA TYPE FIELD
// =============================================================================

const_string!(ObjectTypeConst = "object");
const_string!(StringTypeConst = "string");
const_string!(NumberTypeConst = "number");
const_string!(IntegerTypeConst = "integer");
const_string!(BooleanTypeConst = "boolean");
const_string!(EnumTypeConst = "string");
const_string!(ArrayTypeConst = "array");

// =============================================================================
// PRIMITIVE SCHEMA DEFINITIONS
// =============================================================================

/// Primitive schema definition for elicitation properties.
///
/// According to MCP 2025-06-18 specification, elicitation schemas must have
/// properties of primitive types only (string, number, integer, boolean, enum).
///
/// Note: Put Enum as the first variant to avoid ambiguity during deserialization.
/// This is due to the fact that EnumSchema can contain StringSchema internally and serde
/// uses first match wins strategy when deserializing untagged enums.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum PrimitiveSchema {
    /// Enum property (explicit enum schema)
    Enum(EnumSchema),
    /// String property (with optional enum constraint)
    String(StringSchema),
    /// Number property (with optional enum constraint)
    Number(NumberSchema),
    /// Integer property (with optional enum constraint)
    Integer(IntegerSchema),
    /// Boolean property
    Boolean(BooleanSchema),
}

// =============================================================================
// STRING SCHEMA
// =============================================================================

/// String format types allowed by the MCP specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "kebab-case")]
pub enum StringFormat {
    /// Email address format
    Email,
    /// URI format
    Uri,
    /// Date format (YYYY-MM-DD)
    Date,
    /// Date-time format (ISO 8601)
    DateTime,
}

/// Schema definition for string properties.
///
/// Compliant with MCP 2025-06-18 specification for elicitation schemas.
/// Supports only the fields allowed by the MCP spec:
/// - format limited to: "email", "uri", "date", "date-time"
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct StringSchema {
    /// Type discriminator
    #[serde(rename = "type")]
    pub type_: StringTypeConst,

    /// Optional title for the schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Cow<'static, str>>,

    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Cow<'static, str>>,

    /// Minimum string length
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_length: Option<u32>,

    /// Maximum string length
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_length: Option<u32>,

    /// String format - limited to: "email", "uri", "date", "date-time"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<StringFormat>,
}

impl Default for StringSchema {
    fn default() -> Self {
        Self {
            type_: StringTypeConst,
            title: None,
            description: None,
            min_length: None,
            max_length: None,
            format: None,
        }
    }
}

impl StringSchema {
    /// Create a new string schema
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an email string schema
    pub fn email() -> Self {
        Self {
            format: Some(StringFormat::Email),
            ..Default::default()
        }
    }

    /// Create a URI string schema
    pub fn uri() -> Self {
        Self {
            format: Some(StringFormat::Uri),
            ..Default::default()
        }
    }

    /// Create a date string schema
    pub fn date() -> Self {
        Self {
            format: Some(StringFormat::Date),
            ..Default::default()
        }
    }

    /// Create a date-time string schema
    pub fn date_time() -> Self {
        Self {
            format: Some(StringFormat::DateTime),
            ..Default::default()
        }
    }

    /// Set title
    pub fn title(mut self, title: impl Into<Cow<'static, str>>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set description
    pub fn description(mut self, description: impl Into<Cow<'static, str>>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set minimum and maximum length
    pub fn with_length(mut self, min: u32, max: u32) -> Result<Self, &'static str> {
        if min > max {
            return Err("min_length must be <= max_length");
        }
        self.min_length = Some(min);
        self.max_length = Some(max);
        Ok(self)
    }

    /// Set minimum and maximum length (panics on invalid input)
    pub fn length(mut self, min: u32, max: u32) -> Self {
        assert!(min <= max, "min_length must be <= max_length");
        self.min_length = Some(min);
        self.max_length = Some(max);
        self
    }

    /// Set minimum length
    pub fn min_length(mut self, min: u32) -> Self {
        self.min_length = Some(min);
        self
    }

    /// Set maximum length
    pub fn max_length(mut self, max: u32) -> Self {
        self.max_length = Some(max);
        self
    }

    /// Set format (limited to: "email", "uri", "date", "date-time")
    pub fn format(mut self, format: StringFormat) -> Self {
        self.format = Some(format);
        self
    }
}

// =============================================================================
// NUMBER SCHEMA
// =============================================================================

/// Schema definition for number properties (floating-point).
///
/// Compliant with MCP 2025-06-18 specification for elicitation schemas.
/// Supports only the fields allowed by the MCP spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct NumberSchema {
    /// Type discriminator
    #[serde(rename = "type")]
    pub type_: NumberTypeConst,

    /// Optional title for the schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Cow<'static, str>>,

    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Cow<'static, str>>,

    /// Minimum value (inclusive)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum: Option<f64>,

    /// Maximum value (inclusive)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum: Option<f64>,
}

impl Default for NumberSchema {
    fn default() -> Self {
        Self {
            type_: NumberTypeConst,
            title: None,
            description: None,
            minimum: None,
            maximum: None,
        }
    }
}

impl NumberSchema {
    /// Create a new number schema
    pub fn new() -> Self {
        Self::default()
    }

    /// Set minimum and maximum (inclusive)
    pub fn with_range(mut self, min: f64, max: f64) -> Result<Self, &'static str> {
        if min > max {
            return Err("minimum must be <= maximum");
        }
        self.minimum = Some(min);
        self.maximum = Some(max);
        Ok(self)
    }

    /// Set minimum and maximum (panics on invalid input)
    pub fn range(mut self, min: f64, max: f64) -> Self {
        assert!(min <= max, "minimum must be <= maximum");
        self.minimum = Some(min);
        self.maximum = Some(max);
        self
    }

    /// Set minimum (inclusive)
    pub fn minimum(mut self, min: f64) -> Self {
        self.minimum = Some(min);
        self
    }

    /// Set maximum (inclusive)
    pub fn maximum(mut self, max: f64) -> Self {
        self.maximum = Some(max);
        self
    }

    /// Set title
    pub fn title(mut self, title: impl Into<Cow<'static, str>>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set description
    pub fn description(mut self, description: impl Into<Cow<'static, str>>) -> Self {
        self.description = Some(description.into());
        self
    }
}

// =============================================================================
// INTEGER SCHEMA
// =============================================================================

/// Schema definition for integer properties.
///
/// Compliant with MCP 2025-06-18 specification for elicitation schemas.
/// Supports only the fields allowed by the MCP spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct IntegerSchema {
    /// Type discriminator
    #[serde(rename = "type")]
    pub type_: IntegerTypeConst,

    /// Optional title for the schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Cow<'static, str>>,

    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Cow<'static, str>>,

    /// Minimum value (inclusive)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum: Option<i64>,

    /// Maximum value (inclusive)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum: Option<i64>,
}

impl Default for IntegerSchema {
    fn default() -> Self {
        Self {
            type_: IntegerTypeConst,
            title: None,
            description: None,
            minimum: None,
            maximum: None,
        }
    }
}

impl IntegerSchema {
    /// Create a new integer schema
    pub fn new() -> Self {
        Self::default()
    }

    /// Set minimum and maximum (inclusive)
    pub fn with_range(mut self, min: i64, max: i64) -> Result<Self, &'static str> {
        if min > max {
            return Err("minimum must be <= maximum");
        }
        self.minimum = Some(min);
        self.maximum = Some(max);
        Ok(self)
    }

    /// Set minimum and maximum (panics on invalid input)
    pub fn range(mut self, min: i64, max: i64) -> Self {
        assert!(min <= max, "minimum must be <= maximum");
        self.minimum = Some(min);
        self.maximum = Some(max);
        self
    }

    /// Set minimum (inclusive)
    pub fn minimum(mut self, min: i64) -> Self {
        self.minimum = Some(min);
        self
    }

    /// Set maximum (inclusive)
    pub fn maximum(mut self, max: i64) -> Self {
        self.maximum = Some(max);
        self
    }

    /// Set title
    pub fn title(mut self, title: impl Into<Cow<'static, str>>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set description
    pub fn description(mut self, description: impl Into<Cow<'static, str>>) -> Self {
        self.description = Some(description.into());
        self
    }
}

// =============================================================================
// BOOLEAN SCHEMA
// =============================================================================

/// Schema definition for boolean properties.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct BooleanSchema {
    /// Type discriminator
    #[serde(rename = "type")]
    pub type_: BooleanTypeConst,

    /// Optional title for the schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Cow<'static, str>>,

    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Cow<'static, str>>,

    /// Default value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<bool>,
}

impl Default for BooleanSchema {
    fn default() -> Self {
        Self {
            type_: BooleanTypeConst,
            title: None,
            description: None,
            default: None,
        }
    }
}

impl BooleanSchema {
    /// Create a new boolean schema
    pub fn new() -> Self {
        Self::default()
    }

    /// Set title
    pub fn title(mut self, title: impl Into<Cow<'static, str>>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set description
    pub fn description(mut self, description: impl Into<Cow<'static, str>>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set default value
    pub fn with_default(mut self, default: bool) -> Self {
        self.default = Some(default);
        self
    }
}

// =============================================================================
// ENUM SCHEMA
// =============================================================================

/// Schema definition for enum properties.
///
/// Represent single entry for titled item
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ConstTitle {
    #[serde(rename = "const")]
    pub const_: String,
    pub title: String,
}

/// Legacy enum schema, keep for backward compatibility
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct LegacyEnumSchema {
    #[serde(rename = "type")]
    pub type_: StringTypeConst,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Cow<'static, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Cow<'static, str>>,
    #[serde(rename = "enum")]
    pub enum_: Vec<String>,
    pub enum_names: Option<Vec<String>>,
}

/// Untitled single-select
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct UntitledSingleSelectEnumSchema {
    #[serde(rename = "type")]
    pub type_: StringTypeConst,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Cow<'static, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Cow<'static, str>>,
    #[serde(rename = "enum")]
    pub enum_: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

/// Titled single-select
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct TitledSingleSelectEnumSchema {
    #[serde(rename = "type")]
    pub type_: StringTypeConst,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Cow<'static, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Cow<'static, str>>,
    #[serde(rename = "oneOf")]
    pub one_of: Vec<ConstTitle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

/// Combined single-select
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum SingleSelectEnumSchema {
    Untitled(UntitledSingleSelectEnumSchema),
    Titled(TitledSingleSelectEnumSchema),
}

/// Items for untitled multi-select options
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct UntitledItems {
    #[serde(rename = "type")]
    pub type_: StringTypeConst,
    #[serde(rename = "enum")]
    pub enum_: Vec<String>,
}

/// Items for titled multi-select options
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct TitledItems {
    // MCP spec requires "anyOf" for multi-select enums (allows any combination)
    // Alias "oneOf" for compatibility with schemars
    #[serde(rename = "anyOf", alias = "oneOf")]
    pub any_of: Vec<ConstTitle>,
}

/// Multi-select untitled options
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct UntitledMultiSelectEnumSchema {
    #[serde(rename = "type")]
    pub type_: ArrayTypeConst,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Cow<'static, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Cow<'static, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_items: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_items: Option<u64>,
    pub items: UntitledItems,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<Vec<String>>,
}

/// Multi-select titled options
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct TitledMultiSelectEnumSchema {
    #[serde(rename = "type")]
    pub type_: ArrayTypeConst,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Cow<'static, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Cow<'static, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_items: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_items: Option<u64>,
    pub items: TitledItems,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<Vec<String>>,
}

/// Multi-select enum options
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum MultiSelectEnumSchema {
    Untitled(UntitledMultiSelectEnumSchema),
    Titled(TitledMultiSelectEnumSchema),
}

/// Compliant with MCP 2025-06-18 specification for elicitation schemas.
/// Enums must have string type for values and can optionally include human-readable names.
///
/// # Example
///
/// ```rust
/// use rmcp::model::*;
///
/// let enum_schema = EnumSchema::builder(vec!["US".to_string(), "UK".to_string()])
///    .multiselect()
///    .min_items(1u64).expect("Min items should be correct value")
///    .max_items(4u64).expect("Max items should be correct value")
///    .description("Country code")
///    .build();
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum EnumSchema {
    Single(SingleSelectEnumSchema),
    Multi(MultiSelectEnumSchema),
    Legacy(LegacyEnumSchema),
}

/// Marker type for single-select enum builder
#[derive(Debug)]
pub struct SingleSelect;
/// Marker type for multi-select enum builder
#[derive(Debug)]
pub struct MultiSelect;
/// Builder for EnumSchema
/// Allows to create various enum schema types (single/multi select, titled/untitled)
/// with validation of provided parameters
///
/// # Example
/// ```rust
/// use rmcp::model::*;
/// let enum_schema = EnumSchema::builder(vec!["Red".to_string(), "Green".to_string(), "Blue".to_string()])
///  .multiselect()
///  .enum_titles(vec!["Red Color".to_string(), "Green Color".to_string(), "Blue Color".to_string()])
///  .expect("Number of titles should match number of values")
///  .min_items(1u64).expect("Min items should be correct value")
///  .max_items(3u64).expect("Max items should be correct value")
///  .description("Select your favorite colors")
///  .build();
/// ```
#[derive(Debug)]
pub struct EnumSchemaBuilder<T> {
    /// Enum values
    enum_values: Vec<String>,
    /// If true generate Titled EnumSchema, UnTitled otherwise
    titled: bool,
    /// Title of EnumSchema
    title: Option<Cow<'static, str>>,
    /// Description of EnumSchema
    description: Option<Cow<'static, str>>,
    /// Titles of given enum values
    enum_titles: Vec<String>,
    /// Minimum number of items to choose for MultiSelect
    min_items: Option<u64>,
    /// Maximum number of items to choose for MultiSelect
    max_items: Option<u64>,
    /// Default values for enum
    default: Vec<String>,
    select_type: PhantomData<T>,
}

/// Default implementation for single-select enum builder
impl Default for EnumSchemaBuilder<SingleSelect> {
    fn default() -> Self {
        Self {
            title: None,
            description: None,
            titled: false,
            enum_titles: Vec::new(),
            enum_values: Vec::new(),
            min_items: None,
            max_items: None,
            default: Vec::new(),
            select_type: PhantomData,
        }
    }
}

/// Common enum schema builder methods
impl<T> EnumSchemaBuilder<T> {
    /// Set title of enum schema
    pub fn title(mut self, value: impl Into<Cow<'static, str>>) -> Self {
        self.title = Some(value.into());
        self
    }

    /// Set description of enum schema
    pub fn description(mut self, value: impl Into<Cow<'static, str>>) -> Self {
        self.description = Some(value.into());
        self
    }

    /// Set enum as untitled
    /// Clears any previously set titles
    pub fn untitled(mut self) -> Self {
        self.enum_titles = Vec::new();
        self.titled = false;
        self
    }

    /// Set titles to enum values. Also, implicitly set this enum schema as titled
    pub fn enum_titles(mut self, titles: Vec<String>) -> Result<EnumSchemaBuilder<T>, String> {
        if titles.len() != self.enum_values.len() {
            return Err(format!(
                "Provided number of titles do not match number of values: expected {}, but got {}",
                self.enum_values.len(),
                titles.len()
            ));
        }
        self.titled = true;
        self.enum_titles = titles;
        Ok(self)
    }
}

/// Enum selection builder for single-select enums
impl EnumSchemaBuilder<SingleSelect> {
    pub fn new(values: Vec<String>) -> EnumSchemaBuilder<SingleSelect> {
        EnumSchemaBuilder {
            enum_values: values,
            ..Default::default()
        }
    }

    /// Transition to multi-select enum builder.
    ///
    /// Clears any previously set default values and resets min/max items.
    /// After this transition, you can use `min_items()`, `max_items()`, and
    /// `with_default()` for multi-select semantics.
    pub fn multiselect(self) -> EnumSchemaBuilder<MultiSelect> {
        EnumSchemaBuilder {
            enum_values: self.enum_values,
            titled: self.titled,
            title: self.title,
            description: self.description,
            enum_titles: self.enum_titles,
            min_items: None,
            max_items: None,
            default: Vec::new(), // Clear default for multi-select
            select_type: PhantomData,
        }
    }

    /// Set default value
    pub fn with_default(
        mut self,
        default_value: impl Into<String>,
    ) -> Result<EnumSchemaBuilder<SingleSelect>, String> {
        let value: String = default_value.into();
        if !self.enum_values.contains(&value) {
            return Err("Provided default value is not in enum values".to_string());
        }
        self.default = vec![value];
        Ok(self)
    }

    /// Build enum schema
    pub fn build(mut self) -> EnumSchema {
        match self.titled {
            false => EnumSchema::Single(SingleSelectEnumSchema::Untitled(
                UntitledSingleSelectEnumSchema {
                    type_: StringTypeConst,
                    title: self.title,
                    description: self.description,
                    enum_: self.enum_values,
                    default: self.default.pop(),
                },
            )),
            true => EnumSchema::Single(SingleSelectEnumSchema::Titled(
                TitledSingleSelectEnumSchema {
                    type_: StringTypeConst,
                    title: self.title,
                    description: self.description,
                    one_of: self
                        .enum_titles
                        .into_iter()
                        .zip(self.enum_values)
                        .map(|(title, const_)| ConstTitle { const_, title })
                        .collect(),
                    default: self.default.pop(),
                },
            )),
        }
    }
}

/// Enum selection builder for multi-select enums
impl EnumSchemaBuilder<MultiSelect> {
    /// Set enum as single-select
    /// If it was multi-select, clear default values
    pub fn single_select(self) -> EnumSchemaBuilder<SingleSelect> {
        EnumSchemaBuilder {
            enum_values: self.enum_values,
            titled: self.titled,
            title: self.title,
            description: self.description,
            enum_titles: self.enum_titles,
            min_items: None,
            max_items: None,
            default: Vec::new(), // Clear default for single-select
            select_type: PhantomData,
        }
    }

    /// Set default values
    pub fn with_default(
        mut self,
        default_values: Vec<String>,
    ) -> Result<EnumSchemaBuilder<MultiSelect>, String> {
        for value in &default_values {
            if !self.enum_values.contains(value) {
                return Err("One of the provided default values is not in enum values".to_string());
            }
        }
        if let Some(min) = self.min_items {
            if (default_values.len() as u64) < min {
                return Err("Number of provided default values is less than min_items".to_string());
            }
        }
        if let Some(max) = self.max_items {
            if (default_values.len() as u64) > max {
                return Err(
                    "Number of provided default values is greater than max_items".to_string(),
                );
            }
        }
        self.default = default_values;
        Ok(self)
    }

    /// Set minimal number of items for multi-select enum options
    pub fn min_items(mut self, value: u64) -> Result<EnumSchemaBuilder<MultiSelect>, String> {
        if let Some(max) = self.max_items
            && value > max
        {
            return Err("Provided value is greater than max_items".to_string());
        }
        self.min_items = Some(value);
        Ok(self)
    }

    /// Set maximal number of items for multi-select enum options
    pub fn max_items(mut self, value: u64) -> Result<EnumSchemaBuilder<MultiSelect>, String> {
        if let Some(min) = self.min_items
            && value < min
        {
            return Err("Provided value is less than min_items".to_string());
        }
        self.max_items = Some(value);
        Ok(self)
    }

    /// Build enum schema
    pub fn build(self) -> EnumSchema {
        match self.titled {
            false => EnumSchema::Multi(MultiSelectEnumSchema::Untitled(
                UntitledMultiSelectEnumSchema {
                    type_: ArrayTypeConst,
                    title: self.title,
                    description: self.description,
                    min_items: self.min_items,
                    max_items: self.max_items,
                    items: UntitledItems {
                        type_: StringTypeConst,
                        enum_: self.enum_values,
                    },
                    default: if self.default.is_empty() {
                        None
                    } else {
                        Some(self.default)
                    },
                },
            )),
            true => EnumSchema::Multi(MultiSelectEnumSchema::Titled(TitledMultiSelectEnumSchema {
                type_: ArrayTypeConst,
                title: self.title,
                description: self.description,
                min_items: self.min_items,
                max_items: self.max_items,
                items: TitledItems {
                    any_of: self
                        .enum_titles
                        .into_iter()
                        .zip(self.enum_values)
                        .map(|(title, const_)| ConstTitle { const_, title })
                        .collect(),
                },
                default: if self.default.is_empty() {
                    None
                } else {
                    Some(self.default)
                },
            })),
        }
    }
}

impl EnumSchema {
    /// Creates a new `EnumSchemaBuilder` with the given enum values.
    ///
    /// This convenience method allows you to construct an enum schema by specifying
    /// the possible string values for the enum. Use the returned builder to further
    /// configure the schema before building it.
    ///
    /// # Arguments
    ///
    /// * `values` - A vector of strings representing the allowed enum values.
    ///
    /// # Example
    ///
    /// ```
    /// use rmcp::model::*;
    ///
    /// let enum_schema = EnumSchema::builder(vec!["A".to_string(), "B".to_string()]).
    ///     with_default("A").
    ///     expect("Default value should be valid").
    ///     enum_titles(vec!["Option A".to_string(), "Option B".to_string()]).
    ///     expect("Number of titles should match number of values").
    ///     build();
    /// ```
    pub fn builder(values: Vec<String>) -> EnumSchemaBuilder<SingleSelect> {
        EnumSchemaBuilder::new(values)
    }
}

// =============================================================================
// ELICITATION SCHEMA
// =============================================================================

/// Type-safe elicitation schema for requesting structured user input.
///
/// This enforces the MCP 2025-06-18 specification that elicitation schemas
/// must be objects with primitive-typed properties.
///
/// # Example
///
/// ```rust
/// use rmcp::model::*;
///
/// let schema = ElicitationSchema::builder()
///     .required_email("email")
///     .required_integer("age", 0, 150)
///     .optional_bool("newsletter", false)
///     .build();
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ElicitationSchema {
    /// Always "object" for elicitation schemas
    #[serde(rename = "type")]
    pub type_: ObjectTypeConst,

    /// Optional title for the schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Cow<'static, str>>,

    /// Property definitions (must be primitive types)
    pub properties: BTreeMap<String, PrimitiveSchema>,

    /// List of required property names
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,

    /// Optional description of what this schema represents
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Cow<'static, str>>,
}

impl ElicitationSchema {
    /// Create a new elicitation schema with the given properties
    pub fn new(properties: BTreeMap<String, PrimitiveSchema>) -> Self {
        Self {
            type_: ObjectTypeConst,
            title: None,
            properties,
            required: None,
            description: None,
        }
    }

    /// Convert from a JSON Schema object (typically generated by schemars)
    ///
    /// This allows converting from JsonObject to ElicitationSchema, which is useful
    /// when working with automatically generated schemas from types.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use rmcp::model::*;
    ///
    /// let json_schema = schema_for_type::<MyType>();
    /// let elicitation_schema = ElicitationSchema::from_json_schema(json_schema)?;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a [`serde_json::Error`] if the JSON object cannot be deserialized
    /// into a valid ElicitationSchema.
    pub fn from_json_schema(schema: crate::model::JsonObject) -> Result<Self, serde_json::Error> {
        serde_json::from_value(serde_json::Value::Object(schema))
    }

    /// Generate an ElicitationSchema from a Rust type that implements JsonSchema
    ///
    /// This is a convenience method that combines schema generation and conversion.
    /// It uses the same schema generation settings as the rest of the MCP SDK.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use rmcp::model::*;
    /// use schemars::JsonSchema;
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(JsonSchema, Serialize, Deserialize)]
    /// struct UserInput {
    ///     name: String,
    ///     age: u32,
    /// }
    ///
    /// let schema = ElicitationSchema::from_type::<UserInput>()?;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a [`serde_json::Error`] if the generated schema cannot be converted
    /// to a valid ElicitationSchema.
    #[cfg(feature = "schemars")]
    pub fn from_type<T>() -> Result<Self, serde_json::Error>
    where
        T: schemars::JsonSchema,
    {
        use crate::schemars::generate::SchemaSettings;

        let mut settings = SchemaSettings::draft07();
        settings.transforms = vec![Box::new(schemars::transform::AddNullable::default())];
        let generator = settings.into_generator();
        let schema = generator.into_root_schema_for::<T>();
        let object = serde_json::to_value(schema).expect("failed to serialize schema");
        match object {
            serde_json::Value::Object(object) => Self::from_json_schema(object),
            _ => panic!(
                "Schema serialization produced non-object value: expected JSON object but got {:?}",
                object
            ),
        }
    }

    /// Set the required fields
    pub fn with_required(mut self, required: Vec<String>) -> Self {
        self.required = Some(required);
        self
    }

    /// Set the title
    pub fn with_title(mut self, title: impl Into<Cow<'static, str>>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the description
    pub fn with_description(mut self, description: impl Into<Cow<'static, str>>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Create a builder for constructing elicitation schemas fluently
    pub fn builder() -> ElicitationSchemaBuilder {
        ElicitationSchemaBuilder::new()
    }
}

// =============================================================================
// BUILDER
// =============================================================================

/// Fluent builder for constructing elicitation schemas.
///
/// # Example
///
/// ```rust
/// use rmcp::model::*;
///
/// let schema = ElicitationSchema::builder()
///     .required_email("email")
///     .required_integer("age", 0, 150)
///     .optional_bool("newsletter", false)
///     .description("User registration")
///     .build();
/// ```
#[derive(Debug, Default)]
pub struct ElicitationSchemaBuilder {
    pub properties: BTreeMap<String, PrimitiveSchema>,
    pub required: Vec<String>,
    pub title: Option<Cow<'static, str>>,
    pub description: Option<Cow<'static, str>>,
}

impl ElicitationSchemaBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a property to the schema
    pub fn property(mut self, name: impl Into<String>, schema: PrimitiveSchema) -> Self {
        self.properties.insert(name.into(), schema);
        self
    }

    /// Add a required property to the schema
    pub fn required_property(mut self, name: impl Into<String>, schema: PrimitiveSchema) -> Self {
        let name_str = name.into();
        self.required.push(name_str.clone());
        self.properties.insert(name_str, schema);
        self
    }

    // ===========================================================================
    // TYPED PROPERTY METHODS - Cleaner API without PrimitiveSchema wrapper
    // ===========================================================================

    /// Add a string property with custom builder (required)
    pub fn string_property(
        mut self,
        name: impl Into<String>,
        f: impl FnOnce(StringSchema) -> StringSchema,
    ) -> Self {
        self.properties
            .insert(name.into(), PrimitiveSchema::String(f(StringSchema::new())));
        self
    }

    /// Add a required string property with custom builder
    pub fn required_string_property(
        mut self,
        name: impl Into<String>,
        f: impl FnOnce(StringSchema) -> StringSchema,
    ) -> Self {
        let name_str = name.into();
        self.required.push(name_str.clone());
        self.properties
            .insert(name_str, PrimitiveSchema::String(f(StringSchema::new())));
        self
    }

    /// Add a number property with custom builder
    pub fn number_property(
        mut self,
        name: impl Into<String>,
        f: impl FnOnce(NumberSchema) -> NumberSchema,
    ) -> Self {
        self.properties
            .insert(name.into(), PrimitiveSchema::Number(f(NumberSchema::new())));
        self
    }

    /// Add a required number property with custom builder
    pub fn required_number_property(
        mut self,
        name: impl Into<String>,
        f: impl FnOnce(NumberSchema) -> NumberSchema,
    ) -> Self {
        let name_str = name.into();
        self.required.push(name_str.clone());
        self.properties
            .insert(name_str, PrimitiveSchema::Number(f(NumberSchema::new())));
        self
    }

    /// Add an integer property with custom builder
    pub fn integer_property(
        mut self,
        name: impl Into<String>,
        f: impl FnOnce(IntegerSchema) -> IntegerSchema,
    ) -> Self {
        self.properties.insert(
            name.into(),
            PrimitiveSchema::Integer(f(IntegerSchema::new())),
        );
        self
    }

    /// Add a required integer property with custom builder
    pub fn required_integer_property(
        mut self,
        name: impl Into<String>,
        f: impl FnOnce(IntegerSchema) -> IntegerSchema,
    ) -> Self {
        let name_str = name.into();
        self.required.push(name_str.clone());
        self.properties
            .insert(name_str, PrimitiveSchema::Integer(f(IntegerSchema::new())));
        self
    }

    /// Add a boolean property with custom builder
    pub fn bool_property(
        mut self,
        name: impl Into<String>,
        f: impl FnOnce(BooleanSchema) -> BooleanSchema,
    ) -> Self {
        self.properties.insert(
            name.into(),
            PrimitiveSchema::Boolean(f(BooleanSchema::new())),
        );
        self
    }

    /// Add a required boolean property with custom builder
    pub fn required_bool_property(
        mut self,
        name: impl Into<String>,
        f: impl FnOnce(BooleanSchema) -> BooleanSchema,
    ) -> Self {
        let name_str = name.into();
        self.required.push(name_str.clone());
        self.properties
            .insert(name_str, PrimitiveSchema::Boolean(f(BooleanSchema::new())));
        self
    }

    // ===========================================================================
    // CONVENIENCE METHODS - Simple common cases
    // ===========================================================================

    /// Add a required string property
    pub fn required_string(self, name: impl Into<String>) -> Self {
        self.required_property(name, PrimitiveSchema::String(StringSchema::new()))
    }

    /// Add an optional string property
    pub fn optional_string(self, name: impl Into<String>) -> Self {
        self.property(name, PrimitiveSchema::String(StringSchema::new()))
    }

    /// Add a required email property
    pub fn required_email(self, name: impl Into<String>) -> Self {
        self.required_property(name, PrimitiveSchema::String(StringSchema::email()))
    }

    /// Add an optional email property
    pub fn optional_email(self, name: impl Into<String>) -> Self {
        self.property(name, PrimitiveSchema::String(StringSchema::email()))
    }

    /// Add a required string property with custom builder
    pub fn required_string_with(
        self,
        name: impl Into<String>,
        f: impl FnOnce(StringSchema) -> StringSchema,
    ) -> Self {
        self.required_property(name, PrimitiveSchema::String(f(StringSchema::new())))
    }

    /// Add an optional string property with custom builder
    pub fn optional_string_with(
        self,
        name: impl Into<String>,
        f: impl FnOnce(StringSchema) -> StringSchema,
    ) -> Self {
        self.property(name, PrimitiveSchema::String(f(StringSchema::new())))
    }

    // Convenience methods for numbers

    /// Add a required number property with range
    pub fn required_number(self, name: impl Into<String>, min: f64, max: f64) -> Self {
        self.required_property(
            name,
            PrimitiveSchema::Number(NumberSchema::new().range(min, max)),
        )
    }

    /// Add an optional number property with range
    pub fn optional_number(self, name: impl Into<String>, min: f64, max: f64) -> Self {
        self.property(
            name,
            PrimitiveSchema::Number(NumberSchema::new().range(min, max)),
        )
    }

    /// Add a required number property with custom builder
    pub fn required_number_with(
        self,
        name: impl Into<String>,
        f: impl FnOnce(NumberSchema) -> NumberSchema,
    ) -> Self {
        self.required_property(name, PrimitiveSchema::Number(f(NumberSchema::new())))
    }

    /// Add an optional number property with custom builder
    pub fn optional_number_with(
        self,
        name: impl Into<String>,
        f: impl FnOnce(NumberSchema) -> NumberSchema,
    ) -> Self {
        self.property(name, PrimitiveSchema::Number(f(NumberSchema::new())))
    }

    // Convenience methods for integers

    /// Add a required integer property with range
    pub fn required_integer(self, name: impl Into<String>, min: i64, max: i64) -> Self {
        self.required_property(
            name,
            PrimitiveSchema::Integer(IntegerSchema::new().range(min, max)),
        )
    }

    /// Add an optional integer property with range
    pub fn optional_integer(self, name: impl Into<String>, min: i64, max: i64) -> Self {
        self.property(
            name,
            PrimitiveSchema::Integer(IntegerSchema::new().range(min, max)),
        )
    }

    /// Add a required integer property with custom builder
    pub fn required_integer_with(
        self,
        name: impl Into<String>,
        f: impl FnOnce(IntegerSchema) -> IntegerSchema,
    ) -> Self {
        self.required_property(name, PrimitiveSchema::Integer(f(IntegerSchema::new())))
    }

    /// Add an optional integer property with custom builder
    pub fn optional_integer_with(
        self,
        name: impl Into<String>,
        f: impl FnOnce(IntegerSchema) -> IntegerSchema,
    ) -> Self {
        self.property(name, PrimitiveSchema::Integer(f(IntegerSchema::new())))
    }

    // Convenience methods for booleans

    /// Add a required boolean property
    pub fn required_bool(self, name: impl Into<String>) -> Self {
        self.required_property(name, PrimitiveSchema::Boolean(BooleanSchema::new()))
    }

    /// Add an optional boolean property with default value
    pub fn optional_bool(self, name: impl Into<String>, default: bool) -> Self {
        self.property(
            name,
            PrimitiveSchema::Boolean(BooleanSchema::new().with_default(default)),
        )
    }

    /// Add a required boolean property with custom builder
    pub fn required_bool_with(
        self,
        name: impl Into<String>,
        f: impl FnOnce(BooleanSchema) -> BooleanSchema,
    ) -> Self {
        self.required_property(name, PrimitiveSchema::Boolean(f(BooleanSchema::new())))
    }

    /// Add an optional boolean property with custom builder
    pub fn optional_bool_with(
        self,
        name: impl Into<String>,
        f: impl FnOnce(BooleanSchema) -> BooleanSchema,
    ) -> Self {
        self.property(name, PrimitiveSchema::Boolean(f(BooleanSchema::new())))
    }

    // Enum convenience methods

    /// Add a required enum property using EnumSchema
    pub fn required_enum_schema(self, name: impl Into<String>, enum_schema: EnumSchema) -> Self {
        self.required_property(name, PrimitiveSchema::Enum(enum_schema))
    }

    /// Add an optional enum property using EnumSchema
    pub fn optional_enum_schema(self, name: impl Into<String>, enum_schema: EnumSchema) -> Self {
        self.property(name, PrimitiveSchema::Enum(enum_schema))
    }

    /// Add a required enum property using values. Creates an untitled single-select enum.
    #[deprecated(
        since = "0.13.0",
        note = "Use ElicitationSchemaBuilder::required_enum_schema with EnumSchema::builder instead"
    )]
    pub fn required_enum(self, name: impl Into<String>, values: Vec<String>) -> Self {
        self.required_property(
            name,
            PrimitiveSchema::Enum(EnumSchema::Legacy(LegacyEnumSchema {
                type_: StringTypeConst,
                title: None,
                description: None,
                enum_: values,
                enum_names: None,
            })),
        )
    }

    /// Add an optional enum property using values. Creates an untitled single-select enum.
    #[deprecated(
        since = "0.13.0",
        note = "Use ElicitationSchemaBuilder::optional_enum_schema with EnumSchema::builder instead"
    )]
    pub fn optional_enum(self, name: impl Into<String>, values: Vec<String>) -> Self {
        self.property(
            name,
            PrimitiveSchema::Enum(EnumSchema::Legacy(LegacyEnumSchema {
                type_: StringTypeConst,
                title: None,
                description: None,
                enum_: values,
                enum_names: None,
            })),
        )
    }

    /// Mark an existing property as required
    pub fn mark_required(mut self, name: impl Into<String>) -> Self {
        self.required.push(name.into());
        self
    }

    /// Set the schema title
    pub fn title(mut self, title: impl Into<Cow<'static, str>>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the schema description
    pub fn description(mut self, description: impl Into<Cow<'static, str>>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Build the elicitation schema with validation
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Required fields reference non-existent properties
    /// - No properties are defined (empty schema)
    pub fn build(self) -> Result<ElicitationSchema, &'static str> {
        // Validate that all required fields exist in properties
        if !self.required.is_empty() {
            for field_name in &self.required {
                if !self.properties.contains_key(field_name) {
                    return Err("Required field does not exist in properties");
                }
            }
        }

        Ok(ElicitationSchema {
            type_: ObjectTypeConst,
            title: self.title,
            properties: self.properties,
            required: if self.required.is_empty() {
                None
            } else {
                Some(self.required)
            },
            description: self.description,
        })
    }

    /// Build the elicitation schema without validation (panics on invalid schema)
    ///
    /// # Panics
    ///
    /// Panics if required fields reference non-existent properties
    pub fn build_unchecked(self) -> ElicitationSchema {
        self.build().expect("Invalid elicitation schema")
    }
}

#[cfg(test)]
mod tests {
    use anyhow::anyhow;
    use serde_json::json;

    use super::*;

    #[test]
    fn test_string_schema_serialization() {
        let schema = StringSchema::email().description("Email address");
        let json = serde_json::to_value(&schema).unwrap();

        assert_eq!(json["type"], "string");
        assert_eq!(json["format"], "email");
        assert_eq!(json["description"], "Email address");
    }

    #[test]
    fn test_number_schema_serialization() {
        let schema = NumberSchema::new()
            .range(0.0, 100.0)
            .description("Percentage");
        let json = serde_json::to_value(&schema).unwrap();

        assert_eq!(json["type"], "number");
        assert_eq!(json["minimum"], 0.0);
        assert_eq!(json["maximum"], 100.0);
    }

    #[test]
    fn test_integer_schema_serialization() {
        let schema = IntegerSchema::new().range(0, 150);
        let json = serde_json::to_value(&schema).unwrap();

        assert_eq!(json["type"], "integer");
        assert_eq!(json["minimum"], 0);
        assert_eq!(json["maximum"], 150);
    }

    #[test]
    fn test_boolean_schema_serialization() {
        let schema = BooleanSchema::new().with_default(true);
        let json = serde_json::to_value(&schema).unwrap();

        assert_eq!(json["type"], "boolean");
        assert_eq!(json["default"], true);
    }

    #[test]
    fn test_enum_schema_untitled_single_select_serialization() {
        let schema = EnumSchema::builder(vec!["US".to_string(), "UK".to_string()])
            .description("Country code")
            .build();
        let json = serde_json::to_value(&schema).unwrap();

        assert_eq!(json["type"], "string");
        assert_eq!(json["enum"], json!(["US", "UK"]));
        assert_eq!(json["description"], "Country code");
    }

    #[test]
    fn test_enum_schema_untitled_multi_select_serialization() -> anyhow::Result<()> {
        let schema = EnumSchema::builder(vec!["US".to_string(), "UK".to_string()])
            .multiselect()
            .min_items(1u64)
            .map_err(|e| anyhow!("{e}"))?
            .max_items(4u64)
            .map_err(|e| anyhow!("{e}"))?
            .description("Country code")
            .build();
        let json = serde_json::to_value(&schema)?;

        assert_eq!(json["type"], "array");
        assert_eq!(json["minItems"], 1u64);
        assert_eq!(json["maxItems"], 4u64);
        assert_eq!(json["items"], json!({"type":"string", "enum":["US", "UK"]}));
        assert_eq!(json["description"], "Country code");
        Ok(())
    }

    #[test]
    fn test_enum_schema_titled_single_select_serialization() -> anyhow::Result<()> {
        let schema = EnumSchema::builder(vec!["US".to_string(), "UK".to_string()])
            .enum_titles(vec![
                "United States".to_string(),
                "United Kingdom".to_string(),
            ])
            .map_err(|e| anyhow!("{e}"))?
            .description("Country code")
            .build();
        let json = serde_json::to_value(&schema)?;

        assert_eq!(json["type"], "string");
        assert_eq!(
            json["oneOf"],
            json!([
                {"const": "US", "title":"United States"},
                {"const": "UK", "title":"United Kingdom"}
            ])
        );
        assert_eq!(json["description"], "Country code");
        Ok(())
    }

    #[test]
    fn test_enum_schema_legacy_serialization() -> anyhow::Result<()> {
        let schema = EnumSchema::Legacy(LegacyEnumSchema {
            type_: StringTypeConst,
            title: Some("Legacy Enum".into()),
            description: Some("A legacy enum schema".into()),
            enum_: vec!["A".to_string(), "B".to_string()],
            enum_names: Some(vec!["Option A".to_string(), "Option B".to_string()]),
        });
        let json = serde_json::to_value(&schema)?;

        assert_eq!(json["type"], "string");
        assert_eq!(json["title"], "Legacy Enum");
        assert_eq!(json["description"], "A legacy enum schema");
        assert_eq!(json["enum"], json!(["A", "B"]));
        assert_eq!(json["enumNames"], json!(["Option A", "Option B"]));
        Ok(())
    }

    #[test]
    fn test_enum_schema_titled_multi_select_serialization() -> anyhow::Result<()> {
        let schema = EnumSchema::builder(vec!["US".to_string(), "UK".to_string()])
            .enum_titles(vec![
                "United States".to_string(),
                "United Kingdom".to_string(),
            ])
            .map_err(|e| anyhow!("{e}"))?
            .multiselect()
            .min_items(1u64)
            .map_err(|e| anyhow!("{e}"))?
            .max_items(4u64)
            .map_err(|e| anyhow!("{e}"))?
            .description("Country code")
            .build();
        let json = serde_json::to_value(&schema)?;

        assert_eq!(json["type"], "array");
        assert_eq!(json["minItems"], 1u64);
        assert_eq!(json["maxItems"], 4u64);
        assert_eq!(
            json["items"],
            json!({"anyOf":[
                {"const":"US","title":"United States"},
                {"const":"UK","title":"United Kingdom"}
            ]})
        );
        assert_eq!(json["description"], "Country code");
        Ok(())
    }

    #[test]
    fn test_enum_schema_single_select_with_default() -> anyhow::Result<()> {
        let schema = EnumSchema::builder(vec![
            "red".to_string(),
            "green".to_string(),
            "blue".to_string(),
        ])
        .with_default("green")
        .map_err(|e| anyhow!("{e}"))?
        .description("Favorite color")
        .build();

        let json = serde_json::to_value(&schema)?;

        assert_eq!(json["type"], "string");
        assert_eq!(json["enum"], json!(["red", "green", "blue"]));
        assert_eq!(json["default"], "green");
        assert_eq!(json["description"], "Favorite color");
        Ok(())
    }

    #[test]
    fn test_enum_schema_multi_select_with_default() -> anyhow::Result<()> {
        let schema = EnumSchema::builder(vec![
            "red".to_string(),
            "green".to_string(),
            "blue".to_string(),
        ])
        .multiselect()
        .with_default(vec!["red".to_string(), "blue".to_string()])
        .map_err(|e| anyhow!("{e}"))?
        .min_items(1)
        .map_err(|e| anyhow!("{e}"))?
        .max_items(3)
        .map_err(|e| anyhow!("{e}"))?
        .build();

        let json = serde_json::to_value(&schema)?;

        assert_eq!(json["type"], "array");
        assert_eq!(json["items"]["enum"], json!(["red", "green", "blue"]));
        assert_eq!(json["default"], json!(["red", "blue"]));
        assert_eq!(json["minItems"], 1);
        assert_eq!(json["maxItems"], 3);
        Ok(())
    }

    #[test]
    fn test_enum_schema_transition_clears_defaults() -> anyhow::Result<()> {
        // Start with single-select with default
        let builder = EnumSchema::builder(vec!["A".to_string(), "B".to_string()])
            .with_default("A")
            .map_err(|e| anyhow!("{e}"))?;

        // Transition to multi-select should clear the default
        let schema = builder.multiselect().build();
        let json = serde_json::to_value(&schema)?;

        assert_eq!(json["type"], "array");
        assert!(json["default"].is_null());
        Ok(())
    }

    #[test]
    fn test_enum_schema_multi_to_single_transition() -> anyhow::Result<()> {
        // Start with multi-select with defaults
        let builder = EnumSchema::builder(vec!["A".to_string(), "B".to_string(), "C".to_string()])
            .multiselect()
            .with_default(vec!["A".to_string(), "B".to_string()])
            .map_err(|e| anyhow!("{e}"))?
            .min_items(1)
            .map_err(|e| anyhow!("{e}"))?;

        // Transition back to single-select should clear defaults and min/max items
        let schema = builder.single_select().build();
        let json = serde_json::to_value(&schema)?;

        assert_eq!(json["type"], "string");
        assert!(json["default"].is_null());
        assert!(json["minItems"].is_null());
        assert!(json["maxItems"].is_null());
        Ok(())
    }

    #[test]
    fn test_enum_schema_invalid_single_default() {
        let result = EnumSchema::builder(vec!["A".to_string(), "B".to_string()]).with_default("C");

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Provided default value is not in enum values"
        );
    }

    #[test]
    fn test_enum_schema_invalid_multi_default() {
        let result = EnumSchema::builder(vec!["A".to_string(), "B".to_string()])
            .multiselect()
            .with_default(vec!["A".to_string(), "C".to_string()]);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "One of the provided default values is not in enum values"
        );
    }

    #[test]
    fn test_enum_schema_titled_with_default() -> anyhow::Result<()> {
        let schema = EnumSchema::builder(vec!["US".to_string(), "UK".to_string()])
            .enum_titles(vec![
                "United States".to_string(),
                "United Kingdom".to_string(),
            ])
            .map_err(|e| anyhow!("{e}"))?
            .with_default("UK")
            .map_err(|e| anyhow!("{e}"))?
            .build();

        let json = serde_json::to_value(&schema)?;

        assert_eq!(json["type"], "string");
        assert_eq!(json["default"], "UK");
        assert_eq!(
            json["oneOf"],
            json!([
                {"const": "US", "title": "United States"},
                {"const": "UK", "title": "United Kingdom"}
            ])
        );
        Ok(())
    }

    #[test]
    fn test_enum_schema_untitled_after_titled() -> anyhow::Result<()> {
        let schema = EnumSchema::builder(vec!["A".to_string(), "B".to_string()])
            .enum_titles(vec!["Option A".to_string(), "Option B".to_string()])
            .map_err(|e| anyhow!("{e}"))?
            .untitled()
            .build();

        let json = serde_json::to_value(&schema)?;

        assert_eq!(json["type"], "string");
        assert_eq!(json["enum"], json!(["A", "B"]));
        assert!(json["oneOf"].is_null());
        Ok(())
    }

    #[test]
    fn test_primitive_schema_enum_deserialization() {
        // Test that enum schemas deserialize as Enum variant, not String
        let json = json!({
            "type": "string",
            "enum": ["a", "b"]
        });
        let schema: PrimitiveSchema = serde_json::from_value(json).unwrap();
        assert!(matches!(schema, PrimitiveSchema::Enum(_)));
        // Test that string schemas deserialize as String variant
        let json = json!({
            "type": "string"
        });
        let schema: PrimitiveSchema = serde_json::from_value(json).unwrap();
        assert!(matches!(schema, PrimitiveSchema::String(_)));
    }

    #[test]
    fn test_elicitation_schema_builder_simple() {
        let schema = ElicitationSchema::builder()
            .required_email("email")
            .optional_bool("newsletter", false)
            .build()
            .unwrap();

        assert_eq!(schema.properties.len(), 2);
        assert!(schema.properties.contains_key("email"));
        assert!(schema.properties.contains_key("newsletter"));
        assert_eq!(schema.required, Some(vec!["email".to_string()]));
    }

    #[test]
    fn test_elicitation_schema_builder_complex() {
        let enum_schema =
            EnumSchema::builder(vec!["US".to_string(), "UK".to_string(), "CA".to_string()]).build();
        let schema = ElicitationSchema::builder()
            .required_string_with("name", |s| s.length(1, 100))
            .required_integer("age", 0, 150)
            .optional_bool("newsletter", false)
            .required_enum_schema("country", enum_schema)
            .description("User registration")
            .build()
            .unwrap();

        assert_eq!(schema.properties.len(), 4);
        assert_eq!(
            schema.required,
            Some(vec![
                "name".to_string(),
                "age".to_string(),
                "country".to_string()
            ])
        );
        assert_eq!(schema.description.as_deref(), Some("User registration"));
    }

    #[test]
    fn test_elicitation_schema_serialization() {
        let schema = ElicitationSchema::builder()
            .required_string_with("name", |s| s.length(1, 100))
            .build()
            .unwrap();

        let json = serde_json::to_value(&schema).unwrap();

        assert_eq!(json["type"], "object");
        assert!(json["properties"]["name"].is_object());
        assert_eq!(json["required"], json!(["name"]));
    }

    #[test]
    #[should_panic(expected = "minimum must be <= maximum")]
    fn test_integer_range_validation() {
        IntegerSchema::new().range(10, 5); // Should panic
    }

    #[test]
    #[should_panic(expected = "min_length must be <= max_length")]
    fn test_string_length_validation() {
        StringSchema::new().length(10, 5); // Should panic
    }

    #[test]
    fn test_integer_range_validation_with_result() {
        let result = IntegerSchema::new().with_range(10, 5);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "minimum must be <= maximum");
    }

    #[cfg(feature = "schemars")]
    mod schemars_tests {
        use anyhow::Result;
        use schemars::JsonSchema;
        use serde::{Deserialize, Serialize};
        use serde_json::json;

        use crate::model::ElicitationSchema;

        #[derive(Debug, Serialize, Deserialize, JsonSchema, Default)]
        #[schemars(inline)]
        #[schemars(extend("type" = "string"))]
        enum TitledEnum {
            #[schemars(title = "Title for the first value")]
            #[default]
            FirstValue,
            #[schemars(title = "Title for the second value")]
            SecondValue,
        }

        #[derive(Debug, Serialize, Deserialize, JsonSchema)]
        #[schemars(inline)]
        enum UntitledEnum {
            First,
            Second,
            Third,
        }

        fn default_untitled_multi_select() -> Vec<UntitledEnum> {
            vec![UntitledEnum::Second, UntitledEnum::Third]
        }

        #[derive(Debug, Serialize, Deserialize, JsonSchema)]
        #[schemars(description = "User information")]
        struct UserInfo {
            #[schemars(description = "User's name")]
            pub name: String,
            pub single_select_untitled: UntitledEnum,
            #[schemars(
                title = "Single Select Titled",
                description = "Description for single select enum",
                default
            )]
            pub single_select_titled: TitledEnum,
            #[serde(default = "default_untitled_multi_select")]
            pub multi_select_untitled: Vec<UntitledEnum>,
            #[schemars(
                title = "Multi Select Titled",
                description = "Multi Select Description"
            )]
            pub multi_select_titled: Vec<TitledEnum>,
        }

        #[test]
        fn test_schema_inference_for_enum_fields() -> Result<()> {
            let schema = ElicitationSchema::from_type::<UserInfo>()?;

            let json = serde_json::to_value(&schema)?;
            assert_eq!(json["type"], "object");
            assert_eq!(json["description"], "User information");
            assert_eq!(
                json["required"],
                json!(["name", "single_select_untitled", "multi_select_titled"])
            );
            let properties = match json.get("properties") {
                Some(serde_json::Value::Object(map)) => map,
                _ => panic!("Schema does not have 'properties' field or it is not object type"),
            };

            assert_eq!(properties.len(), 5);
            assert_eq!(
                properties["name"],
                json!({"description":"User's name", "type":"string"})
            );

            assert_eq!(
                properties["single_select_untitled"],
                serde_json::json!({
                    "type":"string",
                    "enum":["First", "Second", "Third"]
                })
            );

            assert_eq!(
                properties["single_select_titled"],
                json!({
                    "type":"string",
                    "title":"Single Select Titled",
                    "description":"Description for single select enum",
                    "oneOf":[
                        {"const":"FirstValue", "title":"Title for the first value"},
                        {"const":"SecondValue", "title":"Title for the second value"}
                    ],
                    "default":"FirstValue"
                })
            );
            assert_eq!(
                properties["multi_select_untitled"],
                serde_json::json!({
                    "type":"array",
                    "items" : {
                        "type":"string",
                        "enum":["First", "Second", "Third"]
                    },
                    "default":["Second", "Third"]
                })
            );
            assert_eq!(
                properties["multi_select_titled"],
                serde_json::json!({
                    "type":"array",
                    "title":"Multi Select Titled",
                    "description":"Multi Select Description",
                    "items":{
                        "anyOf":[
                            {"const":"FirstValue", "title":"Title for the first value"},
                            {"const":"SecondValue", "title":"Title for the second value"}
                        ]
                    }
                })
            );
            Ok(())
        }
    }
}
