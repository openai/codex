# Examples and generated code of each feature of pin-project

## Basic usage of `#[pin_project]` on structs

- [example](struct-default.rs)
- [generated code](struct-default-expanded.rs)

## Basic usage of `#[pin_project]` on enums

- [example](enum-default.rs)
- [generated code](enum-default-expanded.rs)

## Manual implementation of `Unpin` by `UnsafeUnpin`

- [example](unsafe_unpin.rs)
- [generated code](unsafe_unpin-expanded.rs)
- [`UnsafeUnpin` documentation](https://docs.rs/pin-project/latest/pin_project/trait.UnsafeUnpin.html)

## Manual implementation of `Drop` by `#[pinned_drop]`

- [example](pinned_drop.rs)
- [generated code](pinned_drop-expanded.rs)
- [`#[pinned_drop]` documentation](https://docs.rs/pin-project/latest/pin_project/attr.pinned_drop.html)

## `project_replace()` method

- [example](project_replace.rs)
- [generated code](project_replace-expanded.rs)
- [`project_replace()` documentation](https://docs.rs/pin-project/latest/pin_project/attr.pin_project.html#project_replace-method)

## Ensure `!Unpin` by `#[pin_project(!Unpin)]`

- [example](not_unpin.rs)
- [generated code](not_unpin-expanded.rs)
- [`!Unpin` documentation](https://docs.rs/pin-project/latest/pin_project/attr.pin_project.html#unpin)

---

Note: These generated code examples are the little simplified version of the
actual generated code. See [expansion tests](../tests#expansion-tests-expand-expandtestrs) if you
want to see the exact version of the actual generated code.
