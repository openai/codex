use crate::DependencyCheckRequest;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeSet;
use thiserror::Error;
use url::Url;

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct NpmPackage {
    pub name: String,
    pub version: String,
    pub resolved: String,
    pub integrity: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NpmGraph {
    packages: BTreeSet<NpmPackage>,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct NpmInstalledPackage {
    pub name: String,
    pub version: String,
    pub resolved: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NpmInstalledGraph {
    packages: BTreeSet<NpmInstalledPackage>,
}

impl NpmGraph {
    pub fn from_query_json(json: &str) -> Result<Self, NpmGraphError> {
        let nodes: Vec<NpmQueryNode> = serde_json::from_str(json)?;
        let mut packages = BTreeSet::new();

        for node in nodes {
            if node.location.as_deref() == Some("") {
                continue;
            }

            let name = node.name.ok_or(NpmGraphError::MissingField("name"))?;
            let version = node.version.ok_or(NpmGraphError::MissingField("version"))?;
            let resolved = node
                .resolved
                .ok_or_else(|| NpmGraphError::UnsupportedSource {
                    name: name.clone(),
                    version: version.clone(),
                    resolved: None,
                })?;
            let parsed = Url::parse(&resolved).map_err(|_| NpmGraphError::UnsupportedSource {
                name: name.clone(),
                version: version.clone(),
                resolved: Some(resolved.clone()),
            })?;
            if parsed.scheme() != "https" {
                return Err(NpmGraphError::UnsupportedSource {
                    name,
                    version,
                    resolved: Some(resolved),
                });
            }
            let integrity = node
                .integrity
                .filter(|integrity| !integrity.is_empty())
                .ok_or_else(|| NpmGraphError::MissingIntegrity {
                    name: name.clone(),
                    version: version.clone(),
                })?;

            packages.insert(NpmPackage {
                name,
                version,
                resolved,
                integrity,
            });
        }

        if packages.is_empty() {
            return Err(NpmGraphError::EmptyGraph);
        }

        Ok(Self { packages })
    }

    pub fn packages(&self) -> &BTreeSet<NpmPackage> {
        &self.packages
    }

    pub fn coordinates(&self) -> BTreeSet<(String, String)> {
        self.packages
            .iter()
            .map(|package| (package.name.clone(), package.version.clone()))
            .collect()
    }

    pub fn compare(&self, actual: &Self) -> Result<(), NpmGraphMismatch> {
        if self == actual {
            return Ok(());
        }

        Err(NpmGraphMismatch {
            expected_only: self
                .packages
                .difference(&actual.packages)
                .cloned()
                .collect(),
            actual_only: actual
                .packages
                .difference(&self.packages)
                .cloned()
                .collect(),
        })
    }

    pub fn compare_installed(
        &self,
        actual: &NpmInstalledGraph,
    ) -> Result<(), NpmInstalledGraphMismatch> {
        let expected = self
            .packages
            .iter()
            .map(|package| NpmInstalledPackage {
                name: package.name.clone(),
                version: package.version.clone(),
                resolved: package.resolved.clone(),
            })
            .collect::<BTreeSet<_>>();
        if expected == actual.packages {
            return Ok(());
        }

        Err(NpmInstalledGraphMismatch {
            expected_only: expected.difference(&actual.packages).cloned().collect(),
            actual_only: actual.packages.difference(&expected).cloned().collect(),
        })
    }
}

impl NpmInstalledGraph {
    pub fn from_query_json(json: &str) -> Result<Self, NpmGraphError> {
        let nodes: Vec<NpmQueryNode> = serde_json::from_str(json)?;
        let mut packages = BTreeSet::new();

        for node in nodes {
            let Some(node) = parse_query_node(node)? else {
                continue;
            };
            packages.insert(NpmInstalledPackage {
                name: node.name,
                version: node.version,
                resolved: node.resolved,
            });
        }

        if packages.is_empty() {
            return Err(NpmGraphError::EmptyGraph);
        }

        Ok(Self { packages })
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NpmGraphMismatch {
    pub expected_only: Vec<NpmPackage>,
    pub actual_only: Vec<NpmPackage>,
}

impl std::fmt::Display for NpmGraphMismatch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "checked and installed npm graphs differ ({} checked-only entries, {} installed-only entries)",
            self.expected_only.len(),
            self.actual_only.len()
        )
    }
}

impl std::error::Error for NpmGraphMismatch {}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NpmInstalledGraphMismatch {
    pub expected_only: Vec<NpmInstalledPackage>,
    pub actual_only: Vec<NpmInstalledPackage>,
}

impl std::fmt::Display for NpmInstalledGraphMismatch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "checked and installed npm artifact graphs differ ({} checked-only entries, {} installed-only entries)",
            self.expected_only.len(),
            self.actual_only.len()
        )
    }
}

impl std::error::Error for NpmInstalledGraphMismatch {}

#[derive(Debug, Error)]
pub enum NpmGraphError {
    #[error("npm query returned invalid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("npm query result is missing required field `{0}`")]
    MissingField(&'static str),
    #[error("npm package `{name}@{version}` has an unsupported resolved source: {resolved:?}")]
    UnsupportedSource {
        name: String,
        version: String,
        resolved: Option<String>,
    },
    #[error("npm package `{name}@{version}` has no integrity value")]
    MissingIntegrity { name: String, version: String },
    #[error("npm resolved an empty dependency graph")]
    EmptyGraph,
}

#[derive(Debug, Error)]
pub enum NpmManifestError {
    #[error("package.json is not valid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("npm workspaces are not supported by the first dependency_check implementation")]
    WorkspacesUnsupported,
    #[error(
        "project declares unsupported package manager `{0}`; this implementation supports npm projects only"
    )]
    UnsupportedPackageManager(String),
}

pub fn validate_npm_manifest(contents: &str) -> Result<(), NpmManifestError> {
    let manifest: Value = serde_json::from_str(contents)?;
    if manifest
        .get("workspaces")
        .is_some_and(|workspaces| !workspaces.is_null())
    {
        return Err(NpmManifestError::WorkspacesUnsupported);
    }
    if let Some(package_manager) = manifest.get("packageManager").and_then(Value::as_str)
        && !package_manager.starts_with("npm@")
    {
        return Err(NpmManifestError::UnsupportedPackageManager(
            package_manager.to_string(),
        ));
    }
    Ok(())
}

pub fn npm_install_command(
    request: Option<&DependencyCheckRequest>,
    package_lock_only: bool,
) -> Vec<String> {
    let mut command = vec!["npm".to_string(), "install".to_string()];
    command.push("--ignore-scripts".to_string());
    if package_lock_only {
        command.push("--package-lock-only".to_string());
    }
    command.extend([
        "--no-audit".to_string(),
        "--no-fund".to_string(),
        "--save-exact".to_string(),
    ]);

    if let Some(request) = request {
        if let Some(flag) = request.dependency_kind.npm_save_flag() {
            command.push(flag.to_string());
        }
        command.extend(
            request
                .dependencies
                .iter()
                .map(crate::DependencySpec::npm_specifier),
        );
    }

    command
}

pub fn npm_query_lock_command() -> Vec<String> {
    vec![
        "npm".to_string(),
        "query".to_string(),
        "*".to_string(),
        "--json".to_string(),
        "--package-lock-only".to_string(),
    ]
}

pub fn npm_ci_command() -> Vec<String> {
    vec![
        "npm".to_string(),
        "ci".to_string(),
        "--ignore-scripts".to_string(),
        "--no-audit".to_string(),
        "--no-fund".to_string(),
    ]
}

pub fn npm_query_installed_command() -> Vec<String> {
    vec![
        "npm".to_string(),
        "query".to_string(),
        "*".to_string(),
        "--json".to_string(),
    ]
}

pub fn npm_rebuild_command() -> Vec<String> {
    vec!["npm".to_string(), "rebuild".to_string()]
}

#[derive(Debug, Deserialize)]
struct NpmQueryNode {
    name: Option<String>,
    version: Option<String>,
    resolved: Option<String>,
    integrity: Option<String>,
    location: Option<String>,
}

struct ParsedNpmQueryNode {
    name: String,
    version: String,
    resolved: String,
}

fn parse_query_node(node: NpmQueryNode) -> Result<Option<ParsedNpmQueryNode>, NpmGraphError> {
    if node.location.as_deref() == Some("") {
        return Ok(None);
    }

    let name = node.name.ok_or(NpmGraphError::MissingField("name"))?;
    let version = node.version.ok_or(NpmGraphError::MissingField("version"))?;
    let resolved = node
        .resolved
        .ok_or_else(|| NpmGraphError::UnsupportedSource {
            name: name.clone(),
            version: version.clone(),
            resolved: None,
        })?;
    let parsed = Url::parse(&resolved).map_err(|_| NpmGraphError::UnsupportedSource {
        name: name.clone(),
        version: version.clone(),
        resolved: Some(resolved.clone()),
    })?;
    if parsed.scheme() != "https" {
        return Err(NpmGraphError::UnsupportedSource {
            name,
            version,
            resolved: Some(resolved),
        });
    }

    Ok(Some(ParsedNpmQueryNode {
        name,
        version,
        resolved,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DependencyEcosystem;
    use crate::DependencyKind;
    use crate::DependencySpec;
    use pretty_assertions::assert_eq;

    fn request() -> DependencyCheckRequest {
        DependencyCheckRequest {
            ecosystem: DependencyEcosystem::Npm,
            dependencies: vec![DependencySpec {
                name: "zod".to_string(),
                version: "3.23.8".to_string(),
            }],
            dependency_kind: DependencyKind::Development,
            workdir: None,
        }
    }

    #[test]
    fn builds_script_disabled_npm_commands() {
        assert_eq!(
            npm_install_command(Some(&request()), /*package_lock_only*/ true),
            vec![
                "npm",
                "install",
                "--ignore-scripts",
                "--package-lock-only",
                "--no-audit",
                "--no-fund",
                "--save-exact",
                "--save-dev",
                "zod@3.23.8",
            ]
        );
        assert_eq!(
            npm_ci_command(),
            vec!["npm", "ci", "--ignore-scripts", "--no-audit", "--no-fund"]
        );
        assert_eq!(npm_rebuild_command(), vec!["npm", "rebuild"]);
    }

    #[test]
    fn parses_and_compares_lock_graphs() {
        let graph = NpmGraph::from_query_json(
            r#"[
                {"name":"example","version":"1.0.0","location":"","resolved":null},
                {"name":"zod","version":"3.23.8","location":"node_modules/zod","resolved":"https://registry.npmjs.org/zod/-/zod-3.23.8.tgz","integrity":"sha512-example"}
            ]"#,
        )
        .expect("parse graph");
        assert_eq!(
            graph.coordinates(),
            BTreeSet::from([("zod".to_string(), "3.23.8".to_string())])
        );
        assert_eq!(graph.compare(&graph), Ok(()));

        let installed = NpmInstalledGraph::from_query_json(
            r#"[
                {"name":"example","version":"1.0.0","location":"","resolved":null},
                {"name":"zod","version":"3.23.8","location":"node_modules/zod","resolved":"https://registry.npmjs.org/zod/-/zod-3.23.8.tgz"}
            ]"#,
        )
        .expect("parse installed graph");
        assert_eq!(graph.compare_installed(&installed), Ok(()));
    }

    #[test]
    fn reports_lock_and_installed_graph_mismatches() {
        let checked = NpmGraph::from_query_json(
            r#"[{"name":"zod","version":"3.23.8","location":"node_modules/zod","resolved":"https://registry.npmjs.org/zod/-/zod-3.23.8.tgz","integrity":"sha512-checked"}]"#,
        )
        .expect("checked graph");
        let changed = NpmGraph::from_query_json(
            r#"[{"name":"zod","version":"3.24.0","location":"node_modules/zod","resolved":"https://registry.npmjs.org/zod/-/zod-3.24.0.tgz","integrity":"sha512-changed"}]"#,
        )
        .expect("changed graph");
        let installed = NpmInstalledGraph::from_query_json(
            r#"[{"name":"zod","version":"3.24.0","location":"node_modules/zod","resolved":"https://registry.npmjs.org/zod/-/zod-3.24.0.tgz"}]"#,
        )
        .expect("installed graph");

        assert!(checked.compare(&changed).is_err());
        assert!(checked.compare_installed(&installed).is_err());
    }

    #[test]
    fn rejects_unverifiable_sources() {
        let err = NpmGraph::from_query_json(
            r#"[{"name":"local","version":"1.0.0","location":"node_modules/local","resolved":"file:../local"}]"#,
        )
        .expect_err("local source should fail");
        assert!(matches!(err, NpmGraphError::UnsupportedSource { .. }));
    }

    #[test]
    fn rejects_workspaces_and_non_npm_projects() {
        assert!(matches!(
            validate_npm_manifest(r#"{"workspaces":["packages/*"]}"#),
            Err(NpmManifestError::WorkspacesUnsupported)
        ));
        assert!(matches!(
            validate_npm_manifest(r#"{"packageManager":"pnpm@10.0.0"}"#),
            Err(NpmManifestError::UnsupportedPackageManager(manager)) if manager == "pnpm@10.0.0"
        ));
    }
}
