use core::fmt;

use crate::enums::ProtocolVersion;

/// A TLS protocol version supported by rustls.
///
/// All possible instances of this class are provided by the library in
/// the [`ALL_VERSIONS`] array, as well as individually as [`TLS12`]
/// and [`TLS13`].
#[non_exhaustive]
#[derive(Eq, PartialEq)]
pub struct SupportedProtocolVersion {
    /// The TLS enumeration naming this version.
    pub version: ProtocolVersion,
}

impl fmt::Debug for SupportedProtocolVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.version.fmt(f)
    }
}

/// TLS1.2
#[cfg(feature = "tls12")]
pub static TLS12: SupportedProtocolVersion = SupportedProtocolVersion {
    version: ProtocolVersion::TLSv1_2,
};

/// TLS1.3
pub static TLS13: SupportedProtocolVersion = SupportedProtocolVersion {
    version: ProtocolVersion::TLSv1_3,
};

/// A list of all the protocol versions supported by rustls.
pub static ALL_VERSIONS: &[&SupportedProtocolVersion] = &[
    &TLS13,
    #[cfg(feature = "tls12")]
    &TLS12,
];

/// The version configuration that an application should use by default.
///
/// This will be [`ALL_VERSIONS`] for now, but gives space in the future
/// to remove a version from here and require users to opt-in to older
/// versions.
pub static DEFAULT_VERSIONS: &[&SupportedProtocolVersion] = ALL_VERSIONS;

#[derive(Clone, Copy)]
pub(crate) struct EnabledVersions {
    #[cfg(feature = "tls12")]
    tls12: Option<&'static SupportedProtocolVersion>,
    tls13: Option<&'static SupportedProtocolVersion>,
}

impl fmt::Debug for EnabledVersions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut list = &mut f.debug_list();
        #[cfg(feature = "tls12")]
        if let Some(v) = self.tls12 {
            list = list.entry(v);
        }
        if let Some(v) = self.tls13 {
            list = list.entry(v);
        }
        list.finish()
    }
}

impl EnabledVersions {
    pub(crate) fn new(versions: &[&'static SupportedProtocolVersion]) -> Self {
        let mut ev = Self {
            #[cfg(feature = "tls12")]
            tls12: None,
            tls13: None,
        };

        for v in versions {
            match v.version {
                #[cfg(feature = "tls12")]
                ProtocolVersion::TLSv1_2 => ev.tls12 = Some(v),
                ProtocolVersion::TLSv1_3 => ev.tls13 = Some(v),
                _ => {}
            }
        }

        ev
    }

    pub(crate) fn contains(&self, version: ProtocolVersion) -> bool {
        match version {
            #[cfg(feature = "tls12")]
            ProtocolVersion::TLSv1_2 => self.tls12.is_some(),
            ProtocolVersion::TLSv1_3 => self.tls13.is_some(),
            _ => false,
        }
    }
}
