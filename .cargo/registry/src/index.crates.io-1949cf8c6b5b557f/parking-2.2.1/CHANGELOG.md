# Version 2.2.1

- Specify the reason for using `parking` in the docs. (#25)

# Version 2.2.0

- Implement `From<Unparker>` for `Waker`. This enables `Waker`s to be constructed from `Unparker`s without allocating. (#18)

# Version 2.1.1

- Update docs with new logo. (#14)

# Version 2.1.0

- Add will_unpark and same_parker methods to Unparker. (#10)

# Version 2.0.0

- Return `bool` from `unpark()` methods.

# Version 1.0.6

- Add more details on licensing.

# Version 1.0.5

- Implement `Default` for `Parker`.

# Version 1.0.4

- Forbid unsafe code.

# Version 1.0.3

- Improved documentation.

# Version 1.0.2

- Remove all unsafe code.

# Version 1.0.1

- Explain `Parker::park()` better.

# Version 1.0.0

- Initial version.
