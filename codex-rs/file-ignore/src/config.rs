//! Configuration for ignore behavior.

/// Configuration for file ignore behavior.
///
/// Controls which ignore files are respected and how files are filtered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IgnoreConfig {
    /// Whether to respect .gitignore files.
    ///
    /// When `true`, applies rules from:
    /// - `.gitignore` files in the directory tree
    /// - Global gitignore (`~/.config/git/ignore`)
    /// - `.git/info/exclude`
    ///
    /// Default: `true`
    pub respect_gitignore: bool,

    /// Whether to respect `.ignore` files (ripgrep native support).
    ///
    /// Uses the same syntax as `.gitignore`.
    ///
    /// Default: `true`
    pub respect_ignore: bool,

    /// Whether to include hidden files (dotfiles).
    ///
    /// When `false`, files and directories starting with `.` are excluded
    /// (unless explicitly un-ignored).
    ///
    /// Default: `false`
    pub include_hidden: bool,

    /// Whether to follow symbolic links.
    ///
    /// When `true`, symbolic links are followed and their targets are included.
    /// Be careful of cycles when enabling this.
    ///
    /// Default: `false`
    pub follow_links: bool,

    /// Additional custom exclude patterns.
    ///
    /// Uses gitignore syntax (e.g., `*.log`, `**/temp/**`).
    /// These patterns are applied in addition to ignore files.
    pub custom_excludes: Vec<String>,
}

impl Default for IgnoreConfig {
    fn default() -> Self {
        Self {
            respect_gitignore: true,
            respect_ignore: true,
            include_hidden: false,
            follow_links: false,
            custom_excludes: Vec::new(),
        }
    }
}

impl IgnoreConfig {
    /// Create a config that respects all ignore files.
    pub fn respecting_all() -> Self {
        Self::default()
    }

    /// Create a config that ignores all ignore files (show everything).
    pub fn ignoring_none() -> Self {
        Self {
            respect_gitignore: false,
            respect_ignore: false,
            include_hidden: true,
            follow_links: false,
            custom_excludes: Vec::new(),
        }
    }

    /// Builder method: set whether to respect gitignore.
    pub fn with_gitignore(mut self, respect: bool) -> Self {
        self.respect_gitignore = respect;
        self
    }

    /// Builder method: set whether to respect `.ignore` files.
    pub fn with_ignore(mut self, respect: bool) -> Self {
        self.respect_ignore = respect;
        self
    }

    /// Builder method: set whether to include hidden files.
    pub fn with_hidden(mut self, include: bool) -> Self {
        self.include_hidden = include;
        self
    }

    /// Builder method: set whether to follow symlinks.
    pub fn with_follow_links(mut self, follow: bool) -> Self {
        self.follow_links = follow;
        self
    }

    /// Builder method: add custom exclude patterns.
    pub fn with_excludes(mut self, excludes: Vec<String>) -> Self {
        self.custom_excludes = excludes;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = IgnoreConfig::default();
        assert!(config.respect_gitignore);
        assert!(config.respect_ignore);
        assert!(!config.include_hidden);
        assert!(!config.follow_links);
        assert!(config.custom_excludes.is_empty());
    }

    #[test]
    fn test_respecting_all() {
        let config = IgnoreConfig::respecting_all();
        assert!(config.respect_gitignore);
        assert!(config.respect_ignore);
    }

    #[test]
    fn test_ignoring_none() {
        let config = IgnoreConfig::ignoring_none();
        assert!(!config.respect_gitignore);
        assert!(!config.respect_ignore);
        assert!(config.include_hidden);
    }

    #[test]
    fn test_builder_pattern() {
        let config = IgnoreConfig::default()
            .with_gitignore(true)
            .with_ignore(true)
            .with_hidden(true)
            .with_follow_links(true)
            .with_excludes(vec!["*.log".to_string()]);

        assert!(config.respect_gitignore);
        assert!(config.respect_ignore);
        assert!(config.include_hidden);
        assert!(config.follow_links);
        assert_eq!(config.custom_excludes, vec!["*.log"]);
    }
}
