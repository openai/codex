# @generated
# To regenerate, run:
# ```
# STARLARK_RUST_REGENERATE_GOLDEN_TESTS=1 cargo test -p starlark --lib
# ```

error: Not a valid identifier: `bar baz`
 --> assert.bzl:1:8
  |
1 | f'foo {bar baz}'
  |        ^^^^^^^
  |
