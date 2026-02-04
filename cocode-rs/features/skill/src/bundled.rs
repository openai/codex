//! Bundled skills with fingerprinting.
//!
//! Bundled skills are compiled into the binary and serve as defaults.
//! Each bundled skill includes a SHA-256 fingerprint of its prompt content
//! so that changes can be detected when comparing against user-overridden
//! versions.

use sha2::Digest;
use sha2::Sha256;

// Bundled skill prompt templates (embedded at compile time)
const OUTPUT_STYLE_PROMPT: &str = include_str!("bundled/output_style_prompt.md");

/// A skill bundled with the binary.
///
/// Contains the full prompt text and a SHA-256 fingerprint for change
/// detection.
#[derive(Debug, Clone)]
pub struct BundledSkill {
    /// Skill name.
    pub name: String,

    /// Human-readable description.
    pub description: String,

    /// Prompt text.
    pub prompt: String,

    /// SHA-256 hex fingerprint of the prompt content.
    pub fingerprint: String,
}

/// Computes a SHA-256 hex fingerprint of the given content.
///
/// This is used to detect changes between bundled and user-overridden
/// skill prompts.
///
/// # Example
///
/// ```
/// # use cocode_skill::compute_fingerprint;
/// let fp = compute_fingerprint(b"hello world");
/// assert_eq!(fp.len(), 64); // SHA-256 hex is 64 chars
/// ```
pub fn compute_fingerprint(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    let result = hasher.finalize();
    hex_encode(&result)
}

/// Encodes bytes as a lowercase hex string.
fn hex_encode(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            use std::fmt::Write;
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// Returns the list of bundled skills.
///
/// Bundled skills are compiled into the binary and provide essential
/// system commands like output-style management.
pub fn bundled_skills() -> Vec<BundledSkill> {
    vec![BundledSkill {
        name: "output-style".to_string(),
        description: "Manage response output styles (explanatory, learning, etc.)".to_string(),
        prompt: OUTPUT_STYLE_PROMPT.to_string(),
        fingerprint: compute_fingerprint(OUTPUT_STYLE_PROMPT.as_bytes()),
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_fingerprint_deterministic() {
        let fp1 = compute_fingerprint(b"hello world");
        let fp2 = compute_fingerprint(b"hello world");
        assert_eq!(fp1, fp2);
    }

    #[test]
    fn test_compute_fingerprint_different_input() {
        let fp1 = compute_fingerprint(b"hello");
        let fp2 = compute_fingerprint(b"world");
        assert_ne!(fp1, fp2);
    }

    #[test]
    fn test_compute_fingerprint_known_value() {
        // SHA-256 of "hello world" is well-known
        let fp = compute_fingerprint(b"hello world");
        assert_eq!(
            fp,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_compute_fingerprint_empty() {
        let fp = compute_fingerprint(b"");
        // SHA-256 of empty string
        assert_eq!(
            fp,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_compute_fingerprint_length() {
        let fp = compute_fingerprint(b"test");
        assert_eq!(fp.len(), 64);
    }

    #[test]
    fn test_bundled_skills_returns_vec() {
        let skills = bundled_skills();
        // Should contain output-style skill
        assert!(!skills.is_empty());
        assert!(skills.iter().any(|s| s.name == "output-style"));
    }

    #[test]
    fn test_output_style_skill() {
        let skills = bundled_skills();
        let output_style = skills.iter().find(|s| s.name == "output-style").unwrap();
        assert_eq!(
            output_style.description,
            "Manage response output styles (explanatory, learning, etc.)"
        );
        assert!(output_style.prompt.contains("/output-style"));
        assert_eq!(output_style.fingerprint.len(), 64);
    }

    #[test]
    fn test_bundled_skill_struct() {
        let skill = BundledSkill {
            name: "test".to_string(),
            description: "Test skill".to_string(),
            prompt: "Do something".to_string(),
            fingerprint: compute_fingerprint(b"Do something"),
        };
        assert_eq!(skill.name, "test");
        assert_eq!(skill.fingerprint.len(), 64);
    }
}
