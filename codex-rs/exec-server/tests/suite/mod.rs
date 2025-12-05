// TODO(mbolin): Open this up to more OS's once the Bash DotSlash file includes other platforms.
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod auto_approve;
