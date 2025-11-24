use std::fmt;

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
enum Kind {
    Dev,
    Nightly,
    Beta,
    Stable,
}

/// Release channel: "dev", "nightly", "beta", or "stable".
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct Channel(Kind);

impl Channel {
    /// Reads the release channel of the running compiler. If it cannot be
    /// determined (see the [top-level documentation](crate)), returns `None`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use version_check::Channel;
    ///
    /// match Channel::read() {
    ///     Some(c) => format!("The channel is: {}", c),
    ///     None => format!("Failed to read the release channel.")
    /// };
    /// ```
    pub fn read() -> Option<Channel> {
        ::get_version_and_date()
            .and_then(|(version, _)| version)
            .and_then(|version| Channel::parse(&version))
    }

    /// Parse a Rust release channel from a Rust release version string (of the
    /// form `major[.minor[.patch[-channel]]]`). Returns `None` if `version` is
    /// not a valid Rust version string.
    ///
    /// # Example
    ///
    /// ```rust
    /// use version_check::Channel;
    ///
    /// let dev = Channel::parse("1.3.0-dev").unwrap();
    /// assert!(dev.is_dev());
    ///
    /// let nightly = Channel::parse("1.42.2-nightly").unwrap();
    /// assert!(nightly.is_nightly());
    ///
    /// let beta = Channel::parse("1.32.0-beta").unwrap();
    /// assert!(beta.is_beta());
    ///
    /// let stable = Channel::parse("1.4.0").unwrap();
    /// assert!(stable.is_stable());
    /// ```
    pub fn parse(version: &str) -> Option<Channel> {
        let version = version.trim();
        if version.contains("-dev") || version == "dev" {
            Some(Channel(Kind::Dev))
        } else if version.contains("-nightly") || version == "nightly" {
            Some(Channel(Kind::Nightly))
        } else if version.contains("-beta") || version == "beta" {
            Some(Channel(Kind::Beta))
        } else if !version.contains("-") {
            Some(Channel(Kind::Stable))
        } else {
            None
        }
    }

    /// Returns the name of the release channel.
    fn as_str(&self) -> &'static str {
        match self.0 {
            Kind::Dev => "dev",
            Kind::Beta => "beta",
            Kind::Nightly => "nightly",
            Kind::Stable => "stable",
        }
    }

    /// Returns `true` if this channel supports feature flags. In other words,
    /// returns `true` if the channel is either `dev` or `nightly`.
    ///
    /// **Please see the note on [feature detection](crate#feature-detection).**
    ///
    /// # Example
    ///
    /// ```rust
    /// use version_check::Channel;
    ///
    /// let dev = Channel::parse("1.3.0-dev").unwrap();
    /// assert!(dev.supports_features());
    ///
    /// let nightly = Channel::parse("1.42.2-nightly").unwrap();
    /// assert!(nightly.supports_features());
    ///
    /// let beta = Channel::parse("1.32.0-beta").unwrap();
    /// assert!(!beta.supports_features());
    ///
    /// let stable = Channel::parse("1.4.0").unwrap();
    /// assert!(!stable.supports_features());
    /// ```
    pub fn supports_features(&self) -> bool {
        match self.0 {
            Kind::Dev | Kind::Nightly => true,
            Kind::Beta | Kind::Stable => false
        }
    }

    /// Returns `true` if this channel is `dev` and `false` otherwise.
    ///
    /// # Example
    ///
    /// ```rust
    /// use version_check::Channel;
    ///
    /// let dev = Channel::parse("1.3.0-dev").unwrap();
    /// assert!(dev.is_dev());
    ///
    /// let stable = Channel::parse("1.0.0").unwrap();
    /// assert!(!stable.is_dev());
    /// ```
    pub fn is_dev(&self) -> bool {
        match self.0 {
            Kind::Dev => true,
            _ => false
        }
    }

    /// Returns `true` if this channel is `nightly` and `false` otherwise.
    ///
    /// # Example
    ///
    /// ```rust
    /// use version_check::Channel;
    ///
    /// let nightly = Channel::parse("1.3.0-nightly").unwrap();
    /// assert!(nightly.is_nightly());
    ///
    /// let stable = Channel::parse("1.0.0").unwrap();
    /// assert!(!stable.is_nightly());
    /// ```
    pub fn is_nightly(&self) -> bool {
        match self.0 {
            Kind::Nightly => true,
            _ => false
        }
    }

    /// Returns `true` if this channel is `beta` and `false` otherwise.
    ///
    /// # Example
    ///
    /// ```rust
    /// use version_check::Channel;
    ///
    /// let beta = Channel::parse("1.3.0-beta").unwrap();
    /// assert!(beta.is_beta());
    ///
    /// let stable = Channel::parse("1.0.0").unwrap();
    /// assert!(!stable.is_beta());
    /// ```
    pub fn is_beta(&self) -> bool {
        match self.0 {
            Kind::Beta => true,
            _ => false
        }
    }

    /// Returns `true` if this channel is `stable` and `false` otherwise.
    ///
    /// # Example
    ///
    /// ```rust
    /// use version_check::Channel;
    ///
    /// let stable = Channel::parse("1.0.0").unwrap();
    /// assert!(stable.is_stable());
    ///
    /// let beta = Channel::parse("1.3.0-beta").unwrap();
    /// assert!(!beta.is_stable());
    /// ```
    pub fn is_stable(&self) -> bool {
        match self.0 {
            Kind::Stable => true,
            _ => false
        }
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
