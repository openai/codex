# Rust Coding Conventions (cocode-rs)

General Rust coding conventions for cocode-rs development.

## Code Style

### Format and Lint

- When using `format!` and you can inline variables into `{}`, always do that
  ```rust
  // Correct
  format!("{name} is {age}")
  // Avoid
  format!("{} is {}", name, age)
  ```
- Always collapse if statements per [collapsible_if](https://rust-lang.github.io/rust-clippy/master/index.html#collapsible_if)
- Use method references over closures when possible per [redundant_closure_for_method_calls](https://rust-lang.github.io/rust-clippy/master/index.html#redundant_closure_for_method_calls)

### Integer Types

- Use `i32`/`i64` instead of `u32`/`u64` for most cases
- This avoids subtle overflow bugs and matches common API conventions

### Error Handling

- Never use `.unwrap()` in non-test code
- Use `?` for propagation or `.expect("reason")` with clear context
- See `CLAUDE.md` for cocode-error patterns

### Serde Conventions

- Add `#[serde(default)]` for optional config fields
- Add `#[derive(Default)]` for structs used with `..Default::default()`
- Use `#[serde(rename_all = "snake_case")]` for enums

### Comments

- Keep concise - describe purpose, not implementation details
- Field docs: 1-2 lines max, no example configs/commands
- Code comments: state intent only when non-obvious

## Testing

### Test Assertions

- Use `pretty_assertions::assert_eq` for clearer diffs
- Prefer comparing entire objects over individual fields
  ```rust
  // Correct
  assert_eq!(actual, expected);
  // Avoid
  assert_eq!(actual.name, expected.name);
  assert_eq!(actual.value, expected.value);
  ```
- Avoid mutating process environment in tests; prefer passing flags or dependencies

### Test Organization

- Unit tests in same file with `#[cfg(test)]` module
- Integration tests in `tests/` directory
- Use descriptive test names: `test_<function>_<scenario>_<expected>`

## Async Conventions

### Tokio Runtime

- Use `tokio::task::spawn_blocking` for blocking operations
- Prefer `tokio::sync` primitives over `std::sync` in async contexts
- Add `Send + Sync` bounds to traits used with `Arc<dyn Trait>`

### Error Propagation

- Use `?` operator consistently
- Avoid mixing `Result` and `Option` without clear conversion

## Dependencies

### Adding Dependencies

- Prefer well-maintained crates with active development
- Check for security advisories before adding
- Use workspace dependencies when possible

### Common Dependencies

| Purpose | Crate |
|---------|-------|
| Async runtime | `tokio` |
| HTTP client | `reqwest` |
| JSON | `serde_json` |
| Error handling | `anyhow`, `snafu` |
| Logging | `tracing` |
| Testing | `pretty_assertions` |

## Documentation

### Doc Comments

- Use `///` for public items
- Use `//!` for module-level docs
- Include examples for complex APIs
- Keep docs in sync with code changes

### API Changes

When making changes that add or modify an API:
1. Update relevant doc comments
2. Update `docs/` folder if applicable
3. No need to consider backwards compatibility
