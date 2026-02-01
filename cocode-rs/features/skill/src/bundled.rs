//! Bundled skills with fingerprinting.
//!
//! Bundled skills are compiled into the binary and serve as defaults.
//! Each bundled skill includes a SHA-256 fingerprint of its prompt content
//! so that changes can be detected when comparing against user-overridden
//! versions.

use sha2::Digest;
use sha2::Sha256;

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
/// Currently returns an empty list. New bundled skills should be added
/// here by constructing [`BundledSkill`] values with pre-computed
/// fingerprints.
pub fn bundled_skills() -> Vec<BundledSkill> {
    // Placeholder: add bundled skills here as they are developed.
    Vec::new()
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
        // Currently empty; when skills are added, this test should be updated
        assert!(skills.is_empty());
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
