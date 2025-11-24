# Version 2.5.0

- Add a `force_push` method that can be used to add an element to the queue by displacing another. (#58)
- Make `ConcurrentQueue::unbounded()` into a `const` function. (#67)
- Fix a compilation error in the Loom implementation. (#65)

# Version 2.4.0

- Remove unnecessary heap allocations from inside of the `ConcurrentQueue` type. (#53)

# Version 2.3.0

- Implement `UnwindSafe` without libstd. (#49)
- Bump `fastrand` to `v2.0.0`. (#43)
- Use inline assembly in the `full_fence` funtion. (#47)

# Version 2.2.0

- Add the try_iter method. (#36)

# Version 2.1.0

- Update `portable-atomic` to 1.0. (#33)

# Version 2.0.0

- Add support for the `portable-atomic` and `loom` crates. (#27)
- **Breaking:** Add an `std` feature that can be disabled to use this crate on `no_std` platforms. (#22)
- Replace usage of `cache-padded` with `crossbeam-utils`. (#26)

# Version 1.2.4

- Fix fence on x86 and miri. (#18)
- Revert 1.2.3. (#18)

# Version 1.2.3

**Note:** This release has been yanked, see #17 for details.

- Fix fence on non-x86 architectures and miri. (#16)

# Version 1.2.2

- Add a special, efficient `bounded(1)` implementation.

# Version 1.2.1

- In the bounded queue, use boxed slice instead of raw pointers.

# Version 1.2.0

- Update dependencies.
- Implement `UnwindSafe` and `RefUnwindSafe` for `ConcurrentQueue`.

# Version 1.1.2

- Optimize `SeqCst` fences.

# Version 1.1.1

- Clarify errors in docs.

# Version 1.1.0

- Add extra methods to error types.

# Version 1.0.0

- Initial version
