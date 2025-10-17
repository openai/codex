use super::types::Language;
use std::path::Path;

/// Language detector for code files
pub struct LanguageDetector;

impl LanguageDetector {
    /// Detect language from file path
    pub fn detect_from_path(path: &Path) -> Option<Language> {
        let ext = path.extension()?.to_str()?;
        let mut lang = Language::from_extension(ext)?;

        // Special case: Unity C# detection
        if lang == Language::CSharp {
            if let Ok(content) = std::fs::read_to_string(path) {
                if Self::is_unity_script(&content) {
                    lang = Language::CSharpUnity;
                }
            }
        }

        Some(lang)
    }

    /// Check if C# file is Unity script
    fn is_unity_script(content: &str) -> bool {
        content.contains("UnityEngine")
            || content.contains("MonoBehaviour")
            || content.contains("ScriptableObject")
            || content.contains("using UnityEngine")
    }

    /// Detect language from content
    pub fn detect_from_content(content: &str, hint: Option<&str>) -> Option<Language> {
        // Use hint if provided
        if let Some(ext) = hint {
            if let Some(lang) = Language::from_extension(ext) {
                return Some(lang);
            }
        }

        // Heuristic detection
        if content.contains("fn main()") || content.contains("pub fn") {
            Some(Language::Rust)
        } else if content.contains("def ") || content.contains("import ") {
            Some(Language::Python)
        } else if content.contains("function ") || content.contains("const ") {
            if content.contains(": ") && content.contains("=>") {
                Some(Language::TypeScript)
            } else {
                Some(Language::JavaScript)
            }
        } else if content.contains("class ") && content.contains("namespace ") {
            if Self::is_unity_script(content) {
                Some(Language::CSharpUnity)
            } else {
                Some(Language::CSharp)
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    #[test]
    fn test_detect_rust() {
        let path = PathBuf::from("src/main.rs");
        assert_eq!(
            LanguageDetector::detect_from_path(&path),
            Some(Language::Rust)
        );
    }

    #[test]
    fn test_detect_typescript() {
        let path = PathBuf::from("src/app.tsx");
        assert_eq!(
            LanguageDetector::detect_from_path(&path),
            Some(Language::TypeScript)
        );
    }

    #[test]
    fn test_detect_python() {
        let path = PathBuf::from("main.py");
        assert_eq!(
            LanguageDetector::detect_from_path(&path),
            Some(Language::Python)
        );
    }

    #[test]
    fn test_detect_unity_csharp() {
        let content = r#"
using UnityEngine;

public class Player : MonoBehaviour {
    void Start() { }
}
"#;
        assert_eq!(
            LanguageDetector::detect_from_content(content, Some("cs")),
            Some(Language::CSharpUnity)
        );
    }
}

