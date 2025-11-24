/*!
Constants associated with JSON Schema generation.
*/

/// Known values of the `$schema` property.
pub mod meta_schemas {
    /// The mata-schema for [JSON Schema Draft 7](https://json-schema.org/specification-links#draft-7)
    /// (`http://json-schema.org/draft-07/schema#`).
    pub const DRAFT07: &str = "http://json-schema.org/draft-07/schema#";

    /// The mata-schema for [JSON Schema 2019-09](https://json-schema.org/specification-links#draft-2019-09-(formerly-known-as-draft-8))
    /// (`https://json-schema.org/draft/2019-09/schema`).
    pub const DRAFT2019_09: &str = "https://json-schema.org/draft/2019-09/schema";

    /// The mata-schema for [JSON Schema 2020-12](https://json-schema.org/specification-links#2020-12)
    /// (`https://json-schema.org/draft/2020-12/schema`).
    pub const DRAFT2020_12: &str = "https://json-schema.org/draft/2020-12/schema";

    /// The mata-schema for [OpenAPI 3.0 schemas](https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.0.4.md#schema)
    /// (`https://spec.openapis.org/oas/3.0/schema/2024-10-18#/definitions/Schema`).
    ///
    /// This should rarely be encountered in practice, as OpenAPI schemas are typically only
    /// embedded within OpenAPI documents, so do not have a `$schema` property set.
    pub const OPENAPI3: &str =
        "https://spec.openapis.org/oas/3.0/schema/2024-10-18#/definitions/Schema";
}
