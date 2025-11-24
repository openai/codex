# Version 2.2.0

- Bump MSRV to 1.71. (#50)
- Update to `windows-sys` v0.61. (#50)

# Version 2.1.3

- Update `windows-sys` to v0.60. (#46)

# Version 2.1.2

- Ensure that the docs for `create_dir_all` are close to the equivalent function
  in libstd. (#35)

# Version 2.1.1

- Fix a copy/paste error in documentation. (#33)

# Version 2.1.0

- Bump `async-lock` and `futures-lite` to their latest versions. (#27, #28)

# Version 2.0.0

- **Breaking:** Seal extension traits. (#20)
- **Breaking:** Remove unsafe implementations of the `FromRawFd`/`FromRawHandle` traits. (#26)
- Avoid using a `build.rs` script for feature autodetection. (#17)
- Remove the `autocfg` dependency. (#18)
- Avoid a heap allocation in the `ReadDir` implementation. (#23)

# Version 1.6.0

- Implement I/O safety traits on Rust 1.63+ (#13)

# Version 1.5.0

- Replace `&mut self` with `&self` on the following methods:
    - `File::sync_data()`
    - `File::sync_all()`
    - `File::set_len()`

# Version 1.4.0

- Define new extension traits instead of implementing those from `std`.

# Version 1.3.0

- Implement `FromRawFd`/`FromRawHandle` for `File`.
- Implement `OpenOptionsExt` for `OpenOptions` on Windows.
- Re-export some extension traits into OS-specific modules.

# Version 1.2.1

- Optimization: Don't flush if the file is already flushed.

# Version 1.2.0

- Update `blocking` to v1.0

# Version 1.1.2

- Do not reposition the cursor if the file is not seekable.

# Version 1.1.1

- Update dependencies.

# Version 1.1.0

- Implement `From<std::fs::File>` for `File`.

# Version 1.0.1

- Fix build error on https://docs.rs

# Version 1.0.0

- Initial version
