# use std::{cmp, hash};
# #[derive(PartialEq, Hash)]
# struct Identifier;
pub struct Version {
    /// The major version.
    pub major: u64,
    /// The minor version.
    pub minor: u64,
    /// The patch version.
    pub patch: u64,
    /// The pre-release version identifier.
    pub pre: Vec<Identifier>,
    /// The build metadata, ignored when
    /// determining version precedence.
    pub build: Vec<Identifier>,
}

impl cmp::PartialEq for Version {
    #[inline]
    fn eq(&self, other: &Version) -> bool {
        // We should ignore build metadata
        // here, otherwise versions v1 and
        // v2 can exist such that !(v1 < v2)
        // && !(v1 > v2) && v1 != v2, which
        // violate strict total ordering rules.
        self.major == other.major &&
        self.minor == other.minor &&
        self.patch == other.patch &&
        self.pre == other.pre
    }
}

impl hash::Hash for Version {
    fn hash<H: hash::Hasher>(&self, into: &mut H) {
        self.major.hash(into);
        self.minor.hash(into);
        self.patch.hash(into);
        self.pre.hash(into);
    }
}