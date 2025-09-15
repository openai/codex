// End-to-end integration tests for the /context command
use codex_tui::app_event::AppEvent;
use codex_tui::app_event_sender::AppEventSender;
use codex_tui::chatwidget::{ChatWidget, ChatWidgetInit};
use codex_tui::conversation_manager::ConversationManager;
use codex_tui::frame_requester::FrameRequester;
use codex_tui::slash_command::SlashCommand;
use codex_core::config::{Config, ConfigOverrides, ConfigToml};
use codex_core::protocol::{Event, EventMsg, TokenUsage, TokenUsageInfo, SessionConfiguredEvent};
use codex_core::CodexAuth;
use codex_protocol::mcp_protocol::ConversationId;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::WidgetRef;
use std::sync::Arc;
use tokio::sync::mpsc::unbounded_channel;

#[tokio::test]
async fn test_context_command_full_flow() {
    // Setup
    let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
    let tx = AppEventSender::new(tx_raw);
    let config = Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("config");
    
    let conversation_manager = Arc::new(ConversationManager::with_auth(
        CodexAuth::from_api_key("test"),
    ));
    
    let init = ChatWidgetInit {
        config: config.clone(),
        frame_requester: FrameRequester::test_dummy(),
        app_event_tx: tx,
        initial_prompt: None,
        initial_images: Vec::new(),
        enhanced_keys_supported: false,
    };
    
    let mut widget = ChatWidget::new(init, conversation_manager);
    
    // Simulate a session with token usage
    let session_id = ConversationId::new();
    widget.handle_codex_event(Event {
        id: "session-1".into(),
        msg: EventMsg::SessionConfigured(SessionConfiguredEvent {
            session_id: session_id.clone(),
            model: "claude-3-opus".to_string(),
            reasoning_effort: None,
            history_log_id: 0,
            history_entry_count: 0,
            initial_messages: None,
            rollout_path: std::env::temp_dir().join("test.rollout"),
        }),
    });
    
    // Add token usage
    widget.token_info = Some(TokenUsageInfo {
        model_context_window: Some(128000),
        total_token_usage: TokenUsage {
            input_tokens: 10000,
            output_tokens: 5000,
            total_tokens: 15000,
            cache_read_tokens: 1000,
            cache_write_tokens: 500,
        },
    });
    
    // User types /context
    widget.bottom_pane.set_composer_text("/context".to_string());
    widget.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    
    // Check that context info was displayed
    let mut found_context = false;
    while let Ok(event) = rx.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            let lines = cell.display_lines(80);
            let text: String = lines
                .iter()
                .flat_map(|line| line.spans.iter())
                .map(|span| span.content.as_ref())
                .collect();
            
            if text.contains("/context") {
                found_context = true;
                // Verify content
                assert!(text.contains("15000"), "Should show total tokens");
                assert!(text.contains("10000"), "Should show input tokens");
                assert!(text.contains("5000"), "Should show output tokens");
                assert!(text.contains("1000"), "Should show cache read");
                assert!(text.contains("500"), "Should show cache write");
                break;
            }
        }
    }
    
    assert!(found_context, "Context command output should be displayed");
}

#[tokio::test]
async fn test_context_command_during_task() {
    // Setup
    let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
    let tx = AppEventSender::new(tx_raw);
    let config = Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("config");
    
    let conversation_manager = Arc::new(ConversationManager::with_auth(
        CodexAuth::from_api_key("test"),
    ));
    
    let init = ChatWidgetInit {
        config,
        frame_requester: FrameRequester::test_dummy(),
        app_event_tx: tx,
        initial_prompt: None,
        initial_images: Vec::new(),
        enhanced_keys_supported: false,
    };
    
    let mut widget = ChatWidget::new(init, conversation_manager);
    
    // Start a task
    widget.handle_codex_event(Event {
        id: "task-1".into(),
        msg: EventMsg::TaskStarted(codex_core::protocol::TaskStartedEvent {
            model_context_window: Some(128000),
        }),
    });
    
    // Set token usage
    widget.token_info = Some(TokenUsageInfo {
        model_context_window: Some(128000),
        total_token_usage: TokenUsage {
            input_tokens: 50000,
            output_tokens: 25000,
            total_tokens: 75000,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        },
    });
    
    // Context command should work during task
    widget.dispatch_command(SlashCommand::Context);
    
    // Verify output
    let mut found_context = false;
    while let Ok(event) = rx.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            let lines = cell.display_lines(80);
            let text: String = lines
                .iter()
                .flat_map(|line| line.spans.iter())
                .map(|span| span.content.as_ref())
                .collect();
            
            if text.contains("/context") {
                found_context = true;
                // Should show ~58% usage
                assert!(text.contains("75000"), "Should show total tokens during task");
                assert!(text.contains("58") || text.contains("59"), "Should show percentage");
                break;
            }
        }
    }
    
    assert!(found_context, "Context command should work during task");
}

#[tokio::test]
async fn test_context_command_visual_rendering() {
    // Setup
    let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
    let tx = AppEventSender::new(tx_raw);
    let config = Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("config");
    
    let conversation_manager = Arc::new(ConversationManager::with_auth(
        CodexAuth::from_api_key("test"),
    ));
    
    let init = ChatWidgetInit {
        config,
        frame_requester: FrameRequester::test_dummy(),
        app_event_tx: tx,
        initial_prompt: None,
        initial_images: Vec::new(),
        enhanced_keys_supported: false,
    };
    
    let mut widget = ChatWidget::new(init, conversation_manager);
    
    // Set up a scenario with high token usage
    widget.token_info = Some(TokenUsageInfo {
        model_context_window: Some(128000),
        total_token_usage: TokenUsage {
            input_tokens: 90000,
            output_tokens: 30000,
            total_tokens: 120000,
            cache_read_tokens: 5000,
            cache_write_tokens: 2000,
        },
    });
    
    widget.conversation_id = Some(ConversationId::new());
    
    // Execute context command
    widget.dispatch_command(SlashCommand::Context);
    
    // Render to buffer and check visual output
    let area = Rect::new(0, 0, 80, 30);
    let mut buf = Buffer::empty(area);
    (&widget).render_ref(area, &mut buf);
    
    // Convert buffer to text for verification
    let mut visual_text = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            let cell = buf[(x, y)].symbol();
            visual_text.push_str(cell);
        }
        visual_text.push('\n');
    }
    
    // High usage (93.75%) should be visible somehow in the rendered output
    // The exact format depends on the implementation
    assert!(
        visual_text.contains("120000") || visual_text.contains("93") || visual_text.contains("94"),
        "High token usage should be visible in rendering"
    );
}

#[test]
fn test_context_command_parsing_from_user_input() {
    // Test that /context input is properly parsed
    let input = "/context";
    let without_slash = &input[1..];
    
    let parsed: Result<SlashCommand, _> = without_slash.parse();
    assert!(parsed.is_ok(), "Should parse 'context' string");
    assert_eq!(parsed.unwrap(), SlashCommand::Context);
    
    // Test case sensitivity (Strum is case-sensitive by default)
    let uppercase: Result<SlashCommand, _> = "CONTEXT".parse();
    assert!(uppercase.is_err(), "Should not parse uppercase");
    
    let mixed: Result<SlashCommand, _> = "Context".parse();
    assert!(mixed.is_err(), "Should not parse mixed case");
}

#[tokio::test]
async fn test_context_vs_status_command_comparison() {
    // Setup two widgets to compare outputs
    let (tx1_raw, mut rx1) = unbounded_channel::<AppEvent>();
    let (tx2_raw, mut rx2) = unbounded_channel::<AppEvent>();
    
    let config = Config::load_from_base_config_with_overrides(
        ConfigToml::default(),
        ConfigOverrides::default(),
        std::env::temp_dir(),
    )
    .expect("config");
    
    let conversation_manager = Arc::new(ConversationManager::with_auth(
        CodexAuth::from_api_key("test"),
    ));
    
    // Widget 1 for Status command
    let init1 = ChatWidgetInit {
        config: config.clone(),
        frame_requester: FrameRequester::test_dummy(),
        app_event_tx: AppEventSender::new(tx1_raw),
        initial_prompt: None,
        initial_images: Vec::new(),
        enhanced_keys_supported: false,
    };
    
    let mut widget1 = ChatWidget::new(init1, conversation_manager.clone());
    
    // Widget 2 for Context command
    let init2 = ChatWidgetInit {
        config: config.clone(),
        frame_requester: FrameRequester::test_dummy(),
        app_event_tx: AppEventSender::new(tx2_raw),
        initial_prompt: None,
        initial_images: Vec::new(),
        enhanced_keys_supported: false,
    };
    
    let mut widget2 = ChatWidget::new(init2, conversation_manager);
    
    // Set same token usage for both
    let token_info = TokenUsageInfo {
        model_context_window: Some(128000),
        total_token_usage: TokenUsage {
            input_tokens: 20000,
            output_tokens: 10000,
            total_tokens: 30000,
            cache_read_tokens: 2000,
            cache_write_tokens: 1000,
        },
    };
    
    widget1.token_info = Some(token_info.clone());
    widget2.token_info = Some(token_info);
    
    // Execute commands
    widget1.dispatch_command(SlashCommand::Status);
    widget2.dispatch_command(SlashCommand::Context);
    
    // Collect outputs
    let mut status_output = String::new();
    while let Ok(event) = rx1.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            let lines = cell.display_lines(80);
            for line in lines {
                for span in line.spans {
                    status_output.push_str(&span.content);
                }
                status_output.push('\n');
            }
        }
    }
    
    let mut context_output = String::new();
    while let Ok(event) = rx2.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            let lines = cell.display_lines(80);
            for line in lines {
                for span in line.spans {
                    context_output.push_str(&span.content);
                }
                context_output.push('\n');
            }
        }
    }
    
    // Compare outputs
    assert!(status_output.contains("/status"), "Status should have its header");
    assert!(context_output.contains("/context"), "Context should have its header");
    
    // Both should show token info
    assert!(status_output.contains("30000"), "Status should show tokens");
    assert!(context_output.contains("30000"), "Context should show tokens");
    
    // Context should be more focused on context window usage
    assert!(
        context_output.contains("Context Window") || 
        context_output.contains("context window") ||
        context_output.contains("128000"),
        "Context should emphasize context window"
    );
}