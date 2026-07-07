use crate::NpmGraph;
use reqwest::Client;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeSet;
use std::time::Duration;
use thiserror::Error;
use url::Url;

const DEFAULT_OSV_BATCH_URL: &str = "https://api.osv.dev/v1/querybatch";
const OSV_BATCH_SIZE: usize = 1_000;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DependencyPolicyAction {
    Allow,
    Warn,
    Block,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DependencyRiskKind {
    Malware,
    Vulnerability,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DependencyRisk {
    pub package_name: String,
    pub package_version: String,
    pub kind: DependencyRiskKind,
    pub advisory_id: String,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DependencyPolicyReport {
    pub action: DependencyPolicyAction,
    pub checked_packages: usize,
    pub risks: Vec<DependencyRisk>,
}

pub struct OsvClient {
    client: Client,
    endpoint: Url,
}

impl OsvClient {
    pub fn new() -> Result<Self, OsvError> {
        Self::with_endpoint(Url::parse(DEFAULT_OSV_BATCH_URL).map_err(OsvError::InvalidEndpoint)?)
    }

    pub fn with_endpoint(endpoint: Url) -> Result<Self, OsvError> {
        let client = Client::builder().timeout(Duration::from_secs(30));
        let client = codex_client::build_reqwest_client_with_custom_ca(client)
            .map_err(|err| OsvError::BuildClient(err.to_string()))?;
        Ok(Self { client, endpoint })
    }

    pub async fn evaluate(&self, graph: &NpmGraph) -> Result<DependencyPolicyReport, OsvError> {
        let coordinates = graph.coordinates().into_iter().collect::<Vec<_>>();
        let mut risks = Vec::new();

        for chunk in coordinates.chunks(OSV_BATCH_SIZE) {
            let request = BatchRequest {
                queries: chunk
                    .iter()
                    .map(|(name, version)| Query {
                        package: Package {
                            ecosystem: "npm",
                            name,
                        },
                        version,
                    })
                    .collect(),
            };
            let response = self
                .client
                .post(self.endpoint.clone())
                .json(&request)
                .send()
                .await
                .map_err(OsvError::Request)?
                .error_for_status()
                .map_err(OsvError::Request)?
                .json::<BatchResponse>()
                .await
                .map_err(OsvError::Request)?;
            risks.extend(risks_from_response(chunk, response)?);
        }

        let action = if risks
            .iter()
            .any(|risk| risk.kind == DependencyRiskKind::Malware)
        {
            DependencyPolicyAction::Block
        } else if risks.is_empty() {
            DependencyPolicyAction::Allow
        } else {
            DependencyPolicyAction::Warn
        };

        Ok(DependencyPolicyReport {
            action,
            checked_packages: coordinates.len(),
            risks,
        })
    }
}

#[derive(Debug, Error)]
pub enum OsvError {
    #[error("invalid OSV endpoint: {0}")]
    InvalidEndpoint(url::ParseError),
    #[error("failed to build OSV HTTP client: {0}")]
    BuildClient(String),
    #[error("OSV request failed: {0}")]
    Request(reqwest::Error),
    #[error("OSV returned {actual} results for {expected} package queries")]
    ResultCountMismatch { expected: usize, actual: usize },
}

fn risks_from_response(
    coordinates: &[(String, String)],
    response: BatchResponse,
) -> Result<Vec<DependencyRisk>, OsvError> {
    if coordinates.len() != response.results.len() {
        return Err(OsvError::ResultCountMismatch {
            expected: coordinates.len(),
            actual: response.results.len(),
        });
    }

    let mut seen = BTreeSet::new();
    let mut risks = Vec::new();
    for ((name, version), result) in coordinates.iter().zip(response.results) {
        for vulnerability in result.vulns {
            if !seen.insert((name.clone(), version.clone(), vulnerability.id.clone())) {
                continue;
            }
            risks.push(DependencyRisk {
                package_name: name.clone(),
                package_version: version.clone(),
                kind: if vulnerability.id.starts_with("MAL-") {
                    DependencyRiskKind::Malware
                } else {
                    DependencyRiskKind::Vulnerability
                },
                advisory_id: vulnerability.id,
                summary: vulnerability.summary,
            });
        }
    }
    Ok(risks)
}

#[derive(Serialize)]
struct BatchRequest<'a> {
    queries: Vec<Query<'a>>,
}

#[derive(Serialize)]
struct Query<'a> {
    package: Package<'a>,
    version: &'a str,
}

#[derive(Serialize)]
struct Package<'a> {
    ecosystem: &'static str,
    name: &'a str,
}

#[derive(Deserialize)]
struct BatchResponse {
    results: Vec<QueryResult>,
}

#[derive(Deserialize)]
struct QueryResult {
    #[serde(default)]
    vulns: Vec<Vulnerability>,
}

#[derive(Deserialize)]
struct Vulnerability {
    id: String,
    summary: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn maps_malware_to_block_and_cves_to_warn() {
        let coordinates = vec![
            ("malicious".to_string(), "1.0.0".to_string()),
            ("vulnerable".to_string(), "2.0.0".to_string()),
        ];
        let risks = risks_from_response(
            &coordinates,
            BatchResponse {
                results: vec![
                    QueryResult {
                        vulns: vec![Vulnerability {
                            id: "MAL-2024-1".to_string(),
                            summary: Some("malware".to_string()),
                        }],
                    },
                    QueryResult {
                        vulns: vec![Vulnerability {
                            id: "CVE-2026-1".to_string(),
                            summary: Some("vulnerability".to_string()),
                        }],
                    },
                ],
            },
        )
        .expect("map response");

        assert_eq!(
            risks
                .iter()
                .map(|risk| risk.kind)
                .collect::<Vec<DependencyRiskKind>>(),
            vec![
                DependencyRiskKind::Malware,
                DependencyRiskKind::Vulnerability
            ]
        );
    }

    #[test]
    fn rejects_partial_provider_responses() {
        let err = risks_from_response(
            &[("zod".to_string(), "3.23.8".to_string())],
            BatchResponse {
                results: Vec::new(),
            },
        )
        .expect_err("partial response should fail");
        assert!(matches!(err, OsvError::ResultCountMismatch { .. }));
    }
}
