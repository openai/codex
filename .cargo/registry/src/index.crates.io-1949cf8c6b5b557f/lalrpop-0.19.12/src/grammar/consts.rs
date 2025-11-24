/// Recognized associated type for the token location
pub const LOCATION: &str = "Location";

/// Recognized associated type for custom errors
pub const ERROR: &str = "Error";

/// The lifetime parameter injected when we do not have an external token enum
pub const INPUT_LIFETIME: &str = "'input";

/// The parameter injected when we do not have an external token enum
pub const INPUT_PARAMETER: &str = "input";

/// The annotation to request inlining.
pub const INLINE: &str = "inline";

/// The annotation to request conditional compilation.
pub const CFG: &str = "cfg";

/// Annotation to request LALR.
pub const LALR: &str = "LALR";

/// Annotation to request recursive-ascent-style code generation.
pub const TABLE_DRIVEN: &str = "table_driven";

/// Annotation to request recursive-ascent-style code generation.
pub const RECURSIVE_ASCENT: &str = "recursive_ascent";

/// Annotation to request test-all-style code generation.
pub const TEST_ALL: &str = "test_all";
