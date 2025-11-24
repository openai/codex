# @generated
# To regenerate, run:
# ```
# STARLARK_RUST_REGENERATE_GOLDEN_TESTS=1 cargo test -p starlark --lib
# ```

error: Invalid format: Unmatched '{' in format string `foo {bar`
 --> assert.bzl:1:1
  |
1 | f'foo {bar'
  | ^^^^^^^^^^^
  |
