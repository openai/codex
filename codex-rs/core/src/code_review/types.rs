use serde::Deserialize;
use serde::Serialize;

/// Supported programming languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    Rust,
    TypeScript,
    JavaScript,
    Python,
    CSharp,
    CSharpUnity,
}

impl Language {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "rs" => Some(Self::Rust),
            "ts" | "tsx" => Some(Self::TypeScript),
            "js" | "jsx" => Some(Self::JavaScript),
            "py" | "pyw" => Some(Self::Python),
            "cs" => Some(Self::CSharp),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::TypeScript => "TypeScript",
            Self::JavaScript => "JavaScript",
            Self::Python => "Python",
            Self::CSharp => "C#",
            Self::CSharpUnity => "C# (Unity)",
        }
    }

    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            Self::Rust => &["rs"],
            Self::TypeScript => &["ts", "tsx"],
            Self::JavaScript => &["js", "jsx"],
            Self::Python => &["py", "pyw"],
            Self::CSharp | Self::CSharpUnity => &["cs"],
        }
    }
}

/// Code review severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ReviewSeverity {
    Critical = 4,
    High = 3,
    Medium = 2,
    Low = 1,
    Info = 0,
}

impl ReviewSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Critical => "Critical",
            Self::High => "High",
            Self::Medium => "Medium",
            Self::Low => "Low",
            Self::Info => "Info",
        }
    }
}

/// Code review result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewResult {
    pub file_path: String,
    pub language: Language,
    pub issues: Vec<ReviewIssue>,
    pub summary: ReviewSummary,
}

/// Individual review issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewIssue {
    pub line: usize,
    pub column: Option<usize>,
    pub severity: ReviewSeverity,
    pub category: String,
    pub message: String,
    pub suggestion: Option<String>,
    pub rule: Option<String>,
}

/// Review summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewSummary {
    pub total_issues: usize,
    pub critical_count: usize,
    pub high_count: usize,
    pub medium_count: usize,
    pub low_count: usize,
    pub info_count: usize,
}

impl ReviewSummary {
    pub fn from_issues(issues: &[ReviewIssue]) -> Self {
        let mut summary = Self {
            total_issues: issues.len(),
            critical_count: 0,
            high_count: 0,
            medium_count: 0,
            low_count: 0,
            info_count: 0,
        };

        for issue in issues {
            match issue.severity {
                ReviewSeverity::Critical => summary.critical_count += 1,
                ReviewSeverity::High => summary.high_count += 1,
                ReviewSeverity::Medium => summary.medium_count += 1,
                ReviewSeverity::Low => summary.low_count += 1,
                ReviewSeverity::Info => summary.info_count += 1,
            }
        }

        summary
    }
}
