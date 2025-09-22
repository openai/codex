# common Package Summary

## Purpose
Shared utilities and common functionality used across multiple CLI components. Provides consistent behavior and reduces code duplication throughout the Codex ecosystem.

## Key Components

### Configuration Utilities
- **Override Handling**: Manage configuration overrides
- **Default Values**: Shared default configurations
- **Environment Integration**: Environment variable processing

### CLI Argument Helpers
- **Approval Mode Args**: Shared approval flow arguments
- **Sandbox Mode Args**: Common sandbox configuration
- **Common Flags**: Reusable CLI flags and options

### Performance Utilities
- **Timing Tools**: Measure execution time (`elapsed` feature)
- **Performance Metrics**: Track operation performance
- **Profiling Support**: Built-in profiling helpers

### Security Helpers
- **Sandbox Summaries**: Generate policy summaries
- **Policy Utilities**: Common policy operations
- **Validation Helpers**: Input validation utilities

### Matching & Search
- **Fuzzy Matching**: Fuzzy search algorithms
- **Pattern Matching**: Common pattern utilities
- **String Utilities**: Text processing helpers

### Presets & Templates
- **Model Presets**: Pre-configured model settings
- **Approval Presets**: Standard approval configurations
- **Template Helpers**: Common template operations

## Main Functionality
1. **Code Reuse**: Eliminate duplication across packages
2. **Consistency**: Ensure uniform behavior
3. **Convenience**: Provide helpful utilities
4. **Performance**: Optimize common operations
5. **Validation**: Shared validation logic

## Dependencies
Minimal external dependencies to avoid circular dependencies:
- Core Rust libraries
- Basic utility crates
- Conditional feature dependencies

## Integration Points
- Used by `cli` for argument parsing
- Used by `exec` for common operations
- Used by `tui` for utilities
- Shared across all binary packages

## Utility Categories

### String Operations
- Text formatting
- Path manipulation
- Pattern matching
- Encoding/decoding

### File Operations
- Path utilities
- File helpers
- Directory operations
- Permission checks

### Time & Date
- Timestamp formatting
- Duration calculations
- Elapsed time tracking
- Scheduling utilities

### Validation
- Input validation
- Format checking
- Range validation
- Type conversion

## Design Principles
- **Zero-cost Abstractions**: No runtime overhead
- **Type Safety**: Compile-time guarantees
- **Minimal Dependencies**: Reduce dependency tree
- **Backward Compatible**: Stable interfaces
- **Well-tested**: Comprehensive test coverage

## Feature Flags
- `elapsed`: Performance timing utilities
- Additional features for specific functionality
- Optional dependencies based on features