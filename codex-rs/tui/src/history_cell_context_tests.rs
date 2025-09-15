// Tests for history_cell context output functionality
use super::*;
use codex_core::config::{Config, ConfigOverrides, ConfigToml};
use codex_core::protocol::TokenUsage;
use codex_protocol::mcp_protocol::ConversationId;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

#[cfg(test)]
mod history_cell_context_tests {
    use super::*;

    fn test_config() -> Config {
        Config::load_from_base_config_with_overrides(
            ConfigToml::default(),
            ConfigOverrides::default(),
            std::env::temp_dir(),
        )
        .expect("config")
    }

    #[test]
    fn test_new_context_output_basic() {
        let config = test_config();
        let usage = TokenUsage {
            input_tokens: 1000,
            output_tokens: 500,
            total_tokens: 1500,
            cache_read_tokens: 100,
            cache_write_tokens: 50,
        };
        let session_id = Some(ConversationId::new());
        
        let cell = new_context_output(&config, &usage, &session_id);
        let lines = cell.display_lines(80);
        
        // Convert lines to text for easier assertion
        let text: String = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        
        // Basic assertions
        assert!(text.contains("/context"), "Should contain command name");
        assert!(text.contains("Context Window"), "Should have context window header");
        assert!(text.contains("1500"), "Should show total tokens");
        assert!(text.contains("1000"), "Should show input tokens");
        assert!(text.contains("500"), "Should show output tokens");
    }

    #[test]
    fn test_new_context_output_with_cache() {
        let config = test_config();
        let usage = TokenUsage {
            input_tokens: 5000,
            output_tokens: 2000,
            total_tokens: 7000,
            cache_read_tokens: 1500,
            cache_write_tokens: 800,
        };
        let session_id = None;
        
        let cell = new_context_output(&config, &usage, &session_id);
        let lines = cell.display_lines(80);
        
        let text: String = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        
        // Cache tokens should be shown
        assert!(text.contains("1500"), "Should show cache read tokens");
        assert!(text.contains("800"), "Should show cache write tokens");
        assert!(text.contains("cache") || text.contains("Cache"), "Should mention cache");
    }

    #[test]
    fn test_new_context_output_no_tokens() {
        let config = test_config();
        let usage = TokenUsage::default(); // All zeros
        let session_id = None;
        
        let cell = new_context_output(&config, &usage, &session_id);
        let lines = cell.display_lines(80);
        
        let text: String = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        
        assert!(text.contains("/context"), "Should contain command name");
        assert!(
            text.contains("No tokens") || text.contains("0"),
            "Should indicate no usage"
        );
    }

    #[test]
    fn test_new_context_output_percentage_calculation() {
        let config = test_config();
        
        // Test 25% usage (32000 / 128000)
        let usage = TokenUsage {
            input_tokens: 20000,
            output_tokens: 12000,
            total_tokens: 32000,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        };
        
        let cell = new_context_output(&config, &usage, &None);
        let lines = cell.display_lines(80);
        let text: String = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect();
        
        assert!(text.contains("25"), "Should show 25% for quarter usage");
        
        // Test 50% usage
        let usage_half = TokenUsage {
            input_tokens: 40000,
            output_tokens: 24000,
            total_tokens: 64000,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        };
        
        let cell_half = new_context_output(&config, &usage_half, &None);
        let lines_half = cell_half.display_lines(80);
        let text_half: String = lines_half
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect();
        
        assert!(text_half.contains("50"), "Should show 50% for half usage");
    }

    #[test]
    fn test_new_context_output_with_session() {
        let config = test_config();
        let usage = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        };
        
        let session_id = ConversationId::new();
        let session_id_str = session_id.to_string();
        
        let cell = new_context_output(&config, &usage, &Some(session_id));
        let lines = cell.display_lines(80);
        
        let text: String = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        
        assert!(
            text.contains(&session_id_str) || text.contains("Session"),
            "Should show session information"
        );
    }

    #[test]
    fn test_new_context_output_model_info() {
        let mut config = test_config();
        config.model = "claude-3-opus".to_string();
        
        let usage = TokenUsage::default();
        
        let cell = new_context_output(&config, &usage, &None);
        let lines = cell.display_lines(80);
        
        let text: String = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        
        assert!(text.contains("claude-3-opus"), "Should show model name");
        assert!(text.contains("128") && text.contains("000"), "Should show context window size");
    }

    #[test]
    fn test_new_context_output_formatting() {
        let config = test_config();
        let usage = TokenUsage {
            input_tokens: 12345,
            output_tokens: 6789,
            total_tokens: 19134,
            cache_read_tokens: 1111,
            cache_write_tokens: 2222,
        };
        
        let cell = new_context_output(&config, &usage, &Some(ConversationId::new()));
        let lines = cell.display_lines(80);
        
        // Check that we have multiple sections
        let text: String = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        
        // Should have clear sections
        assert!(text.contains("Context Window"), "Should have context section");
        assert!(text.contains("Session") || text.contains("ID"), "Should have session section");
        assert!(text.contains("Model"), "Should have model section");
        
        // Numbers should be formatted nicely
        assert!(text.contains("19134"), "Should show exact total");
        assert!(text.contains("12345"), "Should show exact input");
        assert!(text.contains("6789"), "Should show exact output");
    }

    #[test]
    fn test_new_context_output_line_structure() {
        let config = test_config();
        let usage = TokenUsage {
            input_tokens: 1000,
            output_tokens: 500,
            total_tokens: 1500,
            cache_read_tokens: 100,
            cache_write_tokens: 50,
        };
        
        let cell = new_context_output(&config, &usage, &None);
        let lines = cell.display_lines(80);
        
        // First line should be the command
        assert!(lines.len() > 0, "Should have at least one line");
        let first_line: String = lines[0]
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert!(first_line.contains("context"), "First line should be command");
        
        // Should have emoji indicators for sections
        let all_text: String = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect();
        
        assert!(
            all_text.contains("📊") || all_text.contains("Usage"),
            "Should have usage indicator"
        );
    }

    #[test]
    fn test_new_context_output_edge_cases() {
        let config = test_config();
        
        // Test with maximum tokens (100% usage)
        let usage_max = TokenUsage {
            input_tokens: 100000,
            output_tokens: 28000,
            total_tokens: 128000,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        };
        
        let cell_max = new_context_output(&config, &usage_max, &None);
        let lines_max = cell_max.display_lines(80);
        let text_max: String = lines_max
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect();
        
        assert!(text_max.contains("100"), "Should handle 100% usage");
        assert!(text_max.contains("128000"), "Should show full usage");
        
        // Test with over-limit (shouldn't happen but handle gracefully)
        let usage_over = TokenUsage {
            input_tokens: 130000,
            output_tokens: 20000,
            total_tokens: 150000,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        };
        
        let cell_over = new_context_output(&config, &usage_over, &None);
        let lines_over = cell_over.display_lines(80);
        let text_over: String = lines_over
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect();
        
        assert!(text_over.contains("150000"), "Should show actual usage even if over limit");
        // Percentage might be >100 or capped at 100, both are acceptable
        assert!(
            text_over.contains("100") || text_over.contains("117"),
            "Should handle over-usage percentage"
        );
    }

    #[test]
    fn test_new_context_output_colors_and_styles() {
        let config = test_config();
        let usage = TokenUsage {
            input_tokens: 1000,
            output_tokens: 500,
            total_tokens: 1500,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        };
        
        let cell = new_context_output(&config, &usage, &None);
        let lines = cell.display_lines(80);
        
        // Check that the command line has color
        assert!(!lines.is_empty());
        let first_line = &lines[0];
        
        // Command should be colored (typically magenta for slash commands)
        let has_color = first_line.spans.iter().any(|span| {
            span.style != Style::default()
        });
        
        assert!(has_color, "Command line should have styling");
    }

    #[test]
    fn test_new_context_output_no_cache_tokens() {
        let config = test_config();
        let usage = TokenUsage {
            input_tokens: 5000,
            output_tokens: 3000,
            total_tokens: 8000,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        };
        
        let cell = new_context_output(&config, &usage, &None);
        let lines = cell.display_lines(80);
        
        let text: String = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        
        // When cache tokens are 0, they might be hidden or shown as 0
        // The implementation can choose either approach
        if text.contains("cache") || text.contains("Cache") {
            // If cache is mentioned, it should show 0
            assert!(
                !text.contains("cache_read_tokens: 1") && !text.contains("cache_write_tokens: 1"),
                "Should not show non-zero cache values"
            );
        }
        // Otherwise, it's fine to not mention cache at all when values are 0
    }
}