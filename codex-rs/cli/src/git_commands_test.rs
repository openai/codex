//! Git analysis commands tests

#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn test_generate_author_color() {
        let color1 = generate_author_color("user1@example.com");
        let color2 = generate_author_color("user2@example.com");
        
        // Different emails should produce different colors
        assert_ne!(color1, color2);
        
        // Same email should produce same color (deterministic)
        let color1_again = generate_author_color("user1@example.com");
        assert_eq!(color1, color1_again);
        
        // Should be valid HSL format
        assert!(color1.starts_with("hsl("));
        assert!(color1.ends_with(")"));
    }

    #[test]
    fn test_commit3d_structure() {
        use crate::git_commands::Commit3D;
        
        let commit = Commit3D {
            sha: "abc123".to_string(),
            message: "Test commit".to_string(),
            author: "Test User".to_string(),
            author_email: "test@example.com".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            branch: "main".to_string(),
            parents: vec![],
            x: 0.0,
            y: 100.0,
            z: 0.0,
            color: "hsl(180, 70%, 60%)".to_string(),
        };
        
        assert_eq!(commit.sha, "abc123");
        assert_eq!(commit.x, 0.0);
        assert_eq!(commit.y, 100.0);
        assert_eq!(commit.z, 0.0);
    }

    #[test]
    fn test_file_heat_structure() {
        use crate::git_commands::FileHeat;
        
        let heat = FileHeat {
            path: "src/main.rs".to_string(),
            change_count: 10,
            additions: 100,
            deletions: 50,
            last_modified: chrono::Utc::now().to_rfc3339(),
            authors: vec!["user1".to_string(), "user2".to_string()],
            heat_level: 0.75,
            size: Some(1024),
        };
        
        assert_eq!(heat.change_count, 10);
        assert_eq!(heat.heat_level, 0.75);
        assert_eq!(heat.authors.len(), 2);
    }
}
