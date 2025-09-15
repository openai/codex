// Simple implementation for /context command
// Add this to chatwidget.rs after the Status command handler (around line 872)

// In the dispatch_command function, add:
            SlashCommand::Context => {
                // Show context window usage using existing info infrastructure
                let token_info = self.token_info
                    .as_ref()
                    .map(|ti| format!(
                        "Tokens: {} used / 128,000 available ({:.1}%)",
                        ti.total_token_usage.total_tokens,
                        (ti.total_token_usage.total_tokens as f64 / 128000.0 * 100.0)
                    ))
                    .unwrap_or_else(|| "No tokens used yet".to_string());
                
                self.add_info_message(
                    "/context - Context Window Usage".to_string(),
                    Some(token_info)
                );
                
                // Optionally show more details via status
                if self.token_info.is_some() {
                    self.add_status_output();
                }
            }