# protocol-ts Package Summary

## Purpose
TypeScript type generation utility for protocol definitions. Generates TypeScript type definitions from Rust protocol types, ensuring type safety across language boundaries.

## Key Components

### Type Generator
- **ts-rs Integration**: Use ts-rs for generation
- **Type Extraction**: Extract Rust types
- **TypeScript Emission**: Generate .ts files
- **Schema Generation**: Create JSON schemas

### CLI Tool
- **Command Interface**: CLI for generation
- **Batch Processing**: Generate multiple types
- **Output Management**: Control output location
- **Validation**: Verify generated types

### Type Mapping
- **Rust to TypeScript**: Type conversions
- **Generic Support**: Handle generic types
- **Enum Mapping**: Convert Rust enums
- **Struct Mapping**: Convert Rust structs

## Main Functionality
1. **Type Generation**: Convert Rust types to TypeScript
2. **File Management**: Organize generated files
3. **Schema Creation**: Generate JSON schemas
4. **Validation**: Ensure type correctness
5. **Integration**: Build process integration

## Dependencies
- `ts-rs`: TypeScript generation library
- `protocol`: Source protocol types
- `serde`: Serialization support
- CLI parsing libraries

## Integration Points
- Generates types from `protocol` package
- Used by TypeScript/JavaScript frontends
- Part of build process
- Enables cross-language type safety

## Generation Process

### Input Processing
1. Parse Rust types
2. Extract type information
3. Resolve dependencies
4. Handle imports
5. Process attributes

### Type Conversion
- Primitive type mapping
- Collection types
- Option/Result types
- Custom type handling
- Generic instantiation

### Output Generation
- TypeScript interfaces
- Type aliases
- Enum definitions
- Constant values
- Import statements

## Supported Types

### Rust Types
- Structs
- Enums
- Type aliases
- Generic types
- Trait objects

### TypeScript Output
- Interfaces
- Type aliases
- Enums
- Union types
- Intersection types

## Use Cases
- **Frontend Development**: Type-safe frontend
- **API Contracts**: Shared type definitions
- **Documentation**: Type documentation
- **Validation**: Runtime type checking
- **Code Generation**: Template generation

## Configuration

### Generation Options
- Output directory
- File naming
- Import paths
- Module format
- Formatting options

### Type Options
- Optional handling
- Date/time formats
- BigInt support
- Custom mappings
- Validation rules

## Build Integration
- Cargo build scripts
- CI/CD pipelines
- Watch mode
- Incremental generation
- Version synchronization

## Quality Assurance
- Type checking
- Linting
- Format validation
- Compatibility testing
- Regression testing

## Benefits
- **Type Safety**: Cross-language types
- **Maintenance**: Single source of truth
- **Documentation**: Auto-generated docs
- **Refactoring**: Safe refactoring
- **Developer Experience**: IDE support