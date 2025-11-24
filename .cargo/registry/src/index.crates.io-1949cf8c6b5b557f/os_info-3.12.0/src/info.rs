// spell-checker:ignore itertools, iproduct, bitnesses

use std::fmt::{self, Display, Formatter};

use super::{Bitness, Type, Version};

/// Holds information about operating system (type, version, etc.).
///
/// The best way to get string representation of the operation system information is to use its
/// `Display` implementation.
///
/// # Examples
///
/// ```
/// use os_info;
///
/// let info = os_info::get();
/// println!("OS information: {info}");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Info {
    /// Operating system type. See `Type` for details.
    pub(crate) os_type: Type,
    /// Operating system version. See `Version` for details.
    pub(crate) version: Version,
    /// Operating system edition.
    pub(crate) edition: Option<String>,
    /// Operating system codename.
    pub(crate) codename: Option<String>,
    /// Operating system architecture in terms of how many bits compose the basic values it can deal
    /// with. See `Bitness` for details.
    pub(crate) bitness: Bitness,
    /// Processor architecture.
    pub(crate) architecture: Option<String>,
}

impl Info {
    /// Constructs a new `Info` instance with unknown type, version and bitness.
    ///
    /// # Examples
    ///
    /// ```
    /// use os_info::{Info, Type, Version, Bitness};
    ///
    /// let info = Info::unknown();
    /// assert_eq!(Type::Unknown, info.os_type());
    /// assert_eq!(&Version::Unknown, info.version());
    /// assert_eq!(None, info.edition());
    /// assert_eq!(None, info.codename());
    /// assert_eq!(Bitness::Unknown, info.bitness());
    /// assert_eq!(None, info.architecture());
    /// ```
    pub fn unknown() -> Self {
        Self {
            os_type: Type::Unknown,
            version: Version::Unknown,
            edition: None,
            codename: None,
            bitness: Bitness::Unknown,
            architecture: None,
        }
    }

    /// Constructs a new `Info` instance with the specified operating system type.
    ///
    /// # Examples
    ///
    /// ```
    /// use os_info::{Info, Type, Version, Bitness};
    ///
    /// let os_type = Type::Linux;
    /// let info = Info::with_type(os_type);
    /// assert_eq!(os_type, info.os_type());
    /// assert_eq!(&Version::Unknown, info.version());
    /// assert_eq!(None, info.edition());
    /// assert_eq!(None, info.codename());
    /// assert_eq!(Bitness::Unknown, info.bitness());
    /// assert_eq!(None, info.architecture());
    /// ```
    pub fn with_type(os_type: Type) -> Self {
        Self {
            os_type,
            ..Default::default()
        }
    }

    /// Returns operating system type. See `Type` for details.
    ///
    /// # Examples
    ///
    /// ```
    /// use os_info::{Info, Type};
    ///
    /// let info = Info::unknown();
    /// assert_eq!(Type::Unknown, info.os_type());
    /// ```
    pub fn os_type(&self) -> Type {
        self.os_type
    }

    /// Returns operating system version. See `Version` for details.
    ///
    /// # Examples
    ///
    /// ```
    /// use os_info::{Info, Version};
    ///
    /// let info = Info::unknown();
    /// assert_eq!(&Version::Unknown, info.version());
    /// ```
    pub fn version(&self) -> &Version {
        &self.version
    }

    /// Returns optional operation system edition.
    ///
    /// # Examples
    ///
    /// ```
    /// use os_info::Info;
    ///
    /// let info = Info::unknown();
    /// assert_eq!(None, info.edition());
    pub fn edition(&self) -> Option<&str> {
        self.edition.as_ref().map(String::as_ref)
    }

    /// Returns optional operation system 'codename'.
    ///
    /// # Examples
    ///
    /// ```
    /// use os_info::Info;
    ///
    /// let info = Info::unknown();
    /// assert_eq!(None, info.codename());
    pub fn codename(&self) -> Option<&str> {
        self.codename.as_ref().map(String::as_ref)
    }

    /// Returns operating system bitness. See `Bitness` for details.
    ///
    /// # Examples
    ///
    /// ```
    /// use os_info::{Info, Bitness};
    ///
    /// let info = Info::unknown();
    /// assert_eq!(Bitness::Unknown, info.bitness());
    /// ```
    pub fn bitness(&self) -> Bitness {
        self.bitness
    }

    /// Returns operating system architecture.
    ///
    /// # Examples
    ///
    /// ```
    /// use os_info::Info;
    ///
    /// let info = Info::unknown();
    /// assert_eq!(None, info.architecture());
    pub fn architecture(&self) -> Option<&str> {
        self.architecture.as_ref().map(String::as_ref)
    }
}

impl Default for Info {
    fn default() -> Self {
        Self::unknown()
    }
}

impl Display for Info {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.os_type)?;
        if self.version != Version::Unknown {
            write!(f, " {}", self.version)?;
        }
        if let Some(ref edition) = self.edition {
            write!(f, " ({edition})")?;
        }
        if let Some(ref codename) = self.codename {
            write!(f, " ({codename})")?;
        }
        write!(f, " [{}]", self.bitness)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn unknown() {
        let info = Info::unknown();
        assert_eq!(Type::Unknown, info.os_type());
        assert_eq!(&Version::Unknown, info.version());
        assert_eq!(None, info.edition());
        assert_eq!(None, info.codename());
        assert_eq!(Bitness::Unknown, info.bitness());
        assert_eq!(None, info.architecture());
    }

    #[test]
    fn with_type() {
        let types = [
            Type::AIX,
            Type::Redox,
            Type::Alpaquita,
            Type::Alpine,
            Type::Amazon,
            Type::Android,
            Type::AOSC,
            Type::Arch,
            Type::Artix,
            Type::Bluefin,
            Type::CachyOS,
            Type::CentOS,
            Type::Debian,
            Type::Emscripten,
            Type::EndeavourOS,
            Type::Fedora,
            Type::Gentoo,
            Type::Linux,
            Type::Macos,
            Type::Manjaro,
            Type::Mariner,
            Type::NixOS,
            Type::Nobara,
            Type::Uos,
            Type::OpenCloudOS,
            Type::openEuler,
            Type::openSUSE,
            Type::OracleLinux,
            Type::Pop,
            Type::Redhat,
            Type::RedHatEnterprise,
            Type::Redox,
            Type::Solus,
            Type::SUSE,
            Type::Ubuntu,
            Type::Ultramarine,
            Type::Void,
            Type::Mint,
            Type::Unknown,
            Type::Windows,
        ];

        for t in &types {
            let info = Info::with_type(*t);
            assert_eq!(t, &info.os_type());
        }
    }

    #[test]
    fn default() {
        assert_eq!(Info::default(), Info::unknown());
    }

    #[test]
    fn display() {
        let data = [
            // All unknown.
            (Info::unknown(), "Unknown [unknown bitness]"),
            // Type.
            (
                Info {
                    os_type: Type::Redox,
                    ..Default::default()
                },
                "Redox [unknown bitness]",
            ),
            // Type and version.
            (
                Info {
                    os_type: Type::Linux,
                    version: Version::Semantic(2, 3, 4),
                    ..Default::default()
                },
                "Linux 2.3.4 [unknown bitness]",
            ),
            (
                Info {
                    os_type: Type::AOSC,
                    version: Version::Semantic(12, 1, 3),
                    ..Default::default()
                },
                "AOSC OS 12.1.3 [unknown bitness]",
            ),
            (
                Info {
                    os_type: Type::Arch,
                    version: Version::Rolling(None),
                    ..Default::default()
                },
                "Arch Linux Rolling Release [unknown bitness]",
            ),
            (
                Info {
                    os_type: Type::Artix,
                    version: Version::Rolling(None),
                    ..Default::default()
                },
                "Artix Linux Rolling Release [unknown bitness]",
            ),
            (
                Info {
                    os_type: Type::Manjaro,
                    version: Version::Rolling(Some("2020.05.24".to_owned())),
                    ..Default::default()
                },
                "Manjaro Rolling Release (2020.05.24) [unknown bitness]",
            ),
            (
                Info {
                    os_type: Type::Windows,
                    version: Version::Custom("Special Version".to_owned()),
                    ..Default::default()
                },
                "Windows Special Version [unknown bitness]",
            ),
            // Bitness.
            (
                Info {
                    bitness: Bitness::X32,
                    ..Default::default()
                },
                "Unknown [32-bit]",
            ),
            (
                Info {
                    bitness: Bitness::X64,
                    ..Default::default()
                },
                "Unknown [64-bit]",
            ),
            // All info.
            (
                Info {
                    os_type: Type::Macos,
                    version: Version::Semantic(10, 2, 0),
                    edition: Some("edition".to_owned()),
                    codename: Some("codename".to_owned()),
                    bitness: Bitness::X64,
                    architecture: Some("architecture".to_owned()),
                },
                "Mac OS 10.2.0 (edition) (codename) [64-bit]",
            ),
        ];

        for (info, expected) in &data {
            assert_eq!(expected, &info.to_string());
        }
    }
}
