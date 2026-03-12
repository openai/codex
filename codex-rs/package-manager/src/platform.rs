use crate::PackageManagerError;

/// Supported OS and CPU combinations for managed packages.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackagePlatform {
    /// macOS on Apple Silicon.
    DarwinArm64,
    /// macOS on x86_64.
    DarwinX64,
    /// Linux on AArch64.
    LinuxArm64,
    /// Linux on x86_64.
    LinuxX64,
    /// Windows on AArch64.
    WindowsArm64,
    /// Windows on x86_64.
    WindowsX64,
}

impl PackagePlatform {
    /// Detects the current process platform.
    pub fn detect_current() -> Result<Self, PackageManagerError> {
        match (std::env::consts::OS, std::env::consts::ARCH) {
            ("macos", "aarch64") | ("macos", "arm64") => Ok(Self::DarwinArm64),
            ("macos", "x86_64") => Ok(Self::DarwinX64),
            ("linux", "aarch64") | ("linux", "arm64") => Ok(Self::LinuxArm64),
            ("linux", "x86_64") => Ok(Self::LinuxX64),
            ("windows", "aarch64") | ("windows", "arm64") => Ok(Self::WindowsArm64),
            ("windows", "x86_64") => Ok(Self::WindowsX64),
            (os, arch) => Err(PackageManagerError::UnsupportedPlatform {
                os: os.to_string(),
                arch: arch.to_string(),
            }),
        }
    }

    /// Returns the manifest/cache string for this platform.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DarwinArm64 => "darwin-arm64",
            Self::DarwinX64 => "darwin-x64",
            Self::LinuxArm64 => "linux-arm64",
            Self::LinuxX64 => "linux-x64",
            Self::WindowsArm64 => "windows-arm64",
            Self::WindowsX64 => "windows-x64",
        }
    }
}
