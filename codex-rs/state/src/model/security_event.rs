use anyhow::anyhow;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

/// High-level class for security-relevant audit rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityEventKind {
    /// Sandbox enforcement observed and blocked an attempted action.
    SandboxViolation,
    /// A reviewer assessed a proposed action.
    GuardianAssessment,
}

impl SecurityEventKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SandboxViolation => "sandbox_violation",
            Self::GuardianAssessment => "guardian_assessment",
        }
    }

    pub fn from_db_value(value: &str) -> Option<Self> {
        match value {
            "sandbox_violation" => Some(Self::SandboxViolation),
            "guardian_assessment" => Some(Self::GuardianAssessment),
            _ => None,
        }
    }
}

/// Resource class for sandbox-related security events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityEventResource {
    FileSystem,
    Network,
}

impl SecurityEventResource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FileSystem => "filesystem",
            Self::Network => "network",
        }
    }

    pub fn from_db_value(value: &str) -> Option<Self> {
        match value {
            "filesystem" => Some(Self::FileSystem),
            "network" => Some(Self::Network),
            _ => None,
        }
    }
}

/// Insert payload for a security event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityEventCreateParams {
    pub created_at: i64,
    pub kind: SecurityEventKind,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub call_id: Option<String>,
    pub tool_name: Option<String>,
    pub resource: Option<SecurityEventResource>,
    pub sandbox_type: Option<String>,
    pub reason: Option<String>,
    pub path: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub protocol: Option<String>,
    pub method: Option<String>,
    pub decision: Option<String>,
    pub source: Option<String>,
    pub review_id: Option<String>,
    pub reviewer: Option<String>,
    pub review_decision: Option<String>,
    pub details_json: Option<String>,
}

/// Persisted security event row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityEvent {
    pub id: i64,
    pub created_at: i64,
    pub kind: SecurityEventKind,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub call_id: Option<String>,
    pub tool_name: Option<String>,
    pub resource: Option<SecurityEventResource>,
    pub sandbox_type: Option<String>,
    pub reason: Option<String>,
    pub path: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub protocol: Option<String>,
    pub method: Option<String>,
    pub decision: Option<String>,
    pub source: Option<String>,
    pub review_id: Option<String>,
    pub reviewer: Option<String>,
    pub review_decision: Option<String>,
    pub details_json: Option<String>,
}

impl SecurityEvent {
    pub(crate) fn from_row(row: SqliteRow) -> anyhow::Result<Self> {
        let kind_text: String = row.try_get("kind")?;
        let kind = SecurityEventKind::from_db_value(&kind_text)
            .ok_or_else(|| anyhow!("unknown security event kind `{kind_text}`"))?;
        let resource_text: Option<String> = row.try_get("resource")?;
        let resource = resource_text
            .map(|resource_text| {
                SecurityEventResource::from_db_value(&resource_text)
                    .ok_or_else(|| anyhow!("unknown security event resource `{resource_text}`"))
            })
            .transpose()?;
        let port = row
            .try_get::<Option<i64>, _>("port")?
            .map(|port| {
                u16::try_from(port).map_err(|_| anyhow!("security event port out of range {port}"))
            })
            .transpose()?;

        Ok(Self {
            id: row.try_get("id")?,
            created_at: row.try_get("created_at")?,
            kind,
            thread_id: row.try_get("thread_id")?,
            turn_id: row.try_get("turn_id")?,
            call_id: row.try_get("call_id")?,
            tool_name: row.try_get("tool_name")?,
            resource,
            sandbox_type: row.try_get("sandbox_type")?,
            reason: row.try_get("reason")?,
            path: row.try_get("path")?,
            host: row.try_get("host")?,
            port,
            protocol: row.try_get("protocol")?,
            method: row.try_get("method")?,
            decision: row.try_get("decision")?,
            source: row.try_get("source")?,
            review_id: row.try_get("review_id")?,
            reviewer: row.try_get("reviewer")?,
            review_decision: row.try_get("review_decision")?,
            details_json: row.try_get("details_json")?,
        })
    }
}

/// Filters for reading security events.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SecurityEventQuery {
    pub thread_id: Option<String>,
    pub kind: Option<SecurityEventKind>,
    pub limit: Option<u32>,
}
