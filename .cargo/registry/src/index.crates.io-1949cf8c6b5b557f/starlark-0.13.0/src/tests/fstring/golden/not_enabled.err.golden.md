# @generated
# To regenerate, run:
# ```
# STARLARK_RUST_REGENERATE_GOLDEN_TESTS=1 cargo test -p starlark --lib
# ```

error: Your Starlark dialect must enable f-strings to use them
 --> assert.bzl:1:1
  |
1 | f'{foo}'
  | ^^^^^^^^
  |
