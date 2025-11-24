/// Include generated proto server and client items.
///
/// You must specify the gRPC package name.
///
/// ```rust,ignore
/// mod pb {
///     tonic::include_proto!("helloworld");
/// }
/// ```
///
/// # Note:
/// **This only works if the tonic-build output directory has been unmodified**.
/// The default output directory is set to the [`OUT_DIR`] environment variable.
/// If the output directory has been modified, the following pattern may be used
/// instead of this macro.
///
/// ```rust,ignore
/// mod pb {
///     include!("/relative/protobuf/directory/helloworld.rs");
/// }
/// ```
/// You can also use a custom environment variable using the following pattern.
/// ```rust,ignore
/// mod pb {
///     include!(concat!(env!("PROTOBUFS"), "/helloworld.rs"));
/// }
/// ```
///
/// [`OUT_DIR`]: https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-build-scripts
#[macro_export]
macro_rules! include_proto {
    ($package: tt) => {
        include!(concat!(env!("OUT_DIR"), concat!("/", $package, ".rs")));
    };
}

/// Include an encoded `prost_types::FileDescriptorSet` as a `&'static [u8]`. The parameter must be
/// the stem of the filename passed to `file_descriptor_set_path` for the `tonic-build::Builder`,
/// excluding the `.bin` extension.
///
/// For example, a file descriptor set compiled like this in `build.rs`:
///
/// ```rust,ignore
/// let descriptor_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("my_descriptor.bin")
/// tonic_build::configure()
///     .file_descriptor_set_path(&descriptor_path)
///     .compile_protos(&["proto/reflection.proto"], &["proto/"])?;
/// ```
///
/// Can be included like this:
///
/// ```rust,ignore
/// mod pb {
///     pub(crate) const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("my_descriptor");
/// }
/// ```
///
/// # Note:
/// **This only works if the tonic-build output directory has been unmodified**.
/// The default output directory is set to the [`OUT_DIR`] environment variable.
/// If the output directory has been modified, the following pattern may be used
/// instead of this macro.
///
/// ```rust,ignore
/// mod pb {
///     pub(crate) const FILE_DESCRIPTOR_SET: &[u8] = include_bytes!("/relative/protobuf/directory/descriptor_name.bin");
/// }
/// ```
///
/// [`OUT_DIR`]: https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-build-scripts
#[macro_export]
macro_rules! include_file_descriptor_set {
    ($package: tt) => {
        include_bytes!(concat!(env!("OUT_DIR"), concat!("/", $package, ".bin")))
    };
}
