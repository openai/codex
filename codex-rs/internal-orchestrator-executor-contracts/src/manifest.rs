use std::fmt;
use std::fs::File;

use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;

const CONTRACT_SCHEMA_VERSION: u32 = 1;

/// Stable identity and complete source of an annotated behavioral contract file.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ContractTest {
    id: &'static str,
    #[serde(
        rename = "source_fingerprint",
        serialize_with = "serialize_source_fingerprint"
    )]
    source: &'static str,
}

impl ContractTest {
    pub const fn new(id: &'static str, source: &'static str) -> Self {
        Self { id, source }
    }
}

fn serialize_source_fingerprint<S>(source: &&str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let normalized = source.replace("\r\n", "\n");
    let digest = Sha256::digest(normalized.as_bytes());
    serializer.serialize_str(&format!("sha256:{digest:x}"))
}

inventory::collect!(ContractTest);

/// Canonical, cumulative description of the registered behavioral contracts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ContractManifest {
    schema_version: u32,
    parent_fingerprint: Option<String>,
    tests: Vec<ContractTest>,
}

impl ContractManifest {
    /// Generates the first contract revision from all annotated tests.
    pub fn root() -> Result<Self, ContractManifestError> {
        Self::from_tests(/*parent_fingerprint*/ None, registered_contracts())
    }

    /// Generates a cumulative revision extending an existing contract.
    pub fn successor(parent_fingerprint: impl Into<String>) -> Result<Self, ContractManifestError> {
        Self::from_tests(Some(parent_fingerprint.into()), registered_contracts())
    }

    /// Returns registered tests in their canonical manifest order.
    pub fn tests(&self) -> &[ContractTest] {
        &self.tests
    }

    /// Returns stable JSON bytes whose field and test ordering is deterministic.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Fingerprints the compiled verifier, including all behavioral dependencies.
    pub fn fingerprint(&self) -> Result<String, std::io::Error> {
        let mut executable = File::open(std::env::current_exe()?)?;
        let mut verifier = Sha256::new();
        std::io::copy(&mut executable, &mut verifier)?;
        let digest = verifier.finalize();
        Ok(format!("sha256:{digest:x}"))
    }

    fn from_tests(
        parent_fingerprint: Option<String>,
        mut tests: Vec<ContractTest>,
    ) -> Result<Self, ContractManifestError> {
        if tests.is_empty() {
            return Err(ContractManifestError::EmptySuite);
        }

        tests.sort_unstable_by_key(|contract| contract.id);

        for contracts in tests.windows(2) {
            if contracts[0].id == contracts[1].id {
                return Err(ContractManifestError::DuplicateTestId {
                    id: contracts[0].id.to_string(),
                });
            }
        }

        Ok(Self {
            schema_version: CONTRACT_SCHEMA_VERSION,
            parent_fingerprint,
            tests,
        })
    }
}

fn registered_contracts() -> Vec<ContractTest> {
    inventory::iter::<ContractTest>
        .into_iter()
        .copied()
        .collect()
}

/// A registered suite cannot be represented as an unambiguous contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContractManifestError {
    EmptySuite,
    DuplicateTestId { id: String },
}

impl fmt::Display for ContractManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySuite => formatter.write_str("behavioral contract suite is empty"),
            Self::DuplicateTestId { id } => {
                write!(
                    formatter,
                    "behavioral contract ID is registered twice: {id}"
                )
            }
        }
    }
}

impl std::error::Error for ContractManifestError {}

#[cfg(test)]
#[path = "manifest_tests.rs"]
mod tests;
