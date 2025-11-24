# extern crate derivative;
# use derivative::Derivative;
# #[derive(PartialEq, Hash)]
# struct Identifier;
#[derive(Derivative)]
#[derivative(PartialEq, Hash)]
pub struct Version {
    /// The major version.
    pub major: u64,
    /// The minor version.
    pub minor: u64,
    /// The patch version.
    pub patch: u64,
    /// The pre-release version identifier.
    pub pre: Vec<Identifier>,
    // We should ignore build metadata
    // here, otherwise versions v1 and
    // v2 can exist such that !(v1 < v2)
    // && !(v1 > v2) && v1 != v2, which
    // violate strict total ordering rules.
    #[derivative(PartialEq="ignore")]
    #[derivative(Hash="ignore")]
    /// The build metadata, ignored when
    /// determining version precedence.
    pub build: Vec<Identifier>,
}