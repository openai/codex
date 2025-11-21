//! Execution history cell docs and tests.
//!
//! The rendering itself lives in `exec_cell`, but the History Cell refactor keeps the tests for
//! this cell co-located with the other history cell modules and documents what the execution cell
//! is expected to show.
//!
//! # Output
//!
//! ```plain
//! • Ran bash -lc echo ok
//!   └ (no output)
//! ```

#[cfg(test)]
mod tests {
    use crate::exec_cell::CommandOutput;
    use crate::exec_cell::ExecCall;
    use crate::exec_cell::ExecCell;
    use crate::history_cell::HistoryCell;
    use codex_core::protocol::ExecCommandSource;
    use codex_protocol::parse_command::ParsedCommand;
    use std::time::Duration;
    use std::time::Instant;

    #[test]
    fn coalesces_sequential_reads_within_one_call() {
        // Build one exec cell with a Search followed by two Reads
        let call_id = "c1".to_string();
        let mut cell = ExecCell::new(
            ExecCall {
                call_id: call_id.clone(),
                command: vec!["bash".into(), "-lc".into(), "echo".into()],
                parsed: vec![
                    ParsedCommand::Search {
                        query: Some("shimmer_spans".into()),
                        path: None,
                        cmd: "rg shimmer_spans".into(),
                    },
                    ParsedCommand::Read {
                        name: "shimmer.rs".into(),
                        cmd: "cat shimmer.rs".into(),
                        path: "shimmer.rs".into(),
                    },
                    ParsedCommand::Read {
                        name: "status_indicator_widget.rs".into(),
                        cmd: "cat status_indicator_widget.rs".into(),
                        path: "status_indicator_widget.rs".into(),
                    },
                ],
                output: None,
                source: ExecCommandSource::Agent,
                start_time: Some(Instant::now()),
                duration: None,
                interaction_input: None,
            },
            true,
        );
        // Mark call complete so markers are ✓
        cell.complete_call(&call_id, CommandOutput::default(), Duration::from_millis(1));

        let rendered = cell.display_string(80);
        insta::assert_snapshot!(rendered);
    }

    #[test]
    fn coalesces_reads_across_multiple_calls() {
        let mut cell = ExecCell::new(
            ExecCall {
                call_id: "c1".to_string(),
                command: vec!["bash".into(), "-lc".into(), "echo".into()],
                parsed: vec![ParsedCommand::Search {
                    query: Some("shimmer_spans".into()),
                    path: None,
                    cmd: "rg shimmer_spans".into(),
                }],
                output: None,
                source: ExecCommandSource::Agent,
                start_time: Some(Instant::now()),
                duration: None,
                interaction_input: None,
            },
            true,
        );
        // Call 1: Search only
        cell.complete_call("c1", CommandOutput::default(), Duration::from_millis(1));
        // Call 2: Read A
        cell = cell
            .with_added_call(
                "c2".into(),
                vec!["bash".into(), "-lc".into(), "echo".into()],
                vec![ParsedCommand::Read {
                    name: "shimmer.rs".into(),
                    cmd: "cat shimmer.rs".into(),
                    path: "shimmer.rs".into(),
                }],
                ExecCommandSource::Agent,
                None,
            )
            .unwrap();
        cell.complete_call("c2", CommandOutput::default(), Duration::from_millis(1));
        // Call 3: Read B
        cell = cell
            .with_added_call(
                "c3".into(),
                vec!["bash".into(), "-lc".into(), "echo".into()],
                vec![ParsedCommand::Read {
                    name: "status_indicator_widget.rs".into(),
                    cmd: "cat status_indicator_widget.rs".into(),
                    path: "status_indicator_widget.rs".into(),
                }],
                ExecCommandSource::Agent,
                None,
            )
            .unwrap();
        cell.complete_call("c3", CommandOutput::default(), Duration::from_millis(1));

        let rendered = cell.display_string(80);
        insta::assert_snapshot!(rendered);
    }

    #[test]
    fn coalesced_reads_dedupe_names() {
        let mut cell = ExecCell::new(
            ExecCall {
                call_id: "c1".to_string(),
                command: vec!["bash".into(), "-lc".into(), "echo".into()],
                parsed: vec![
                    ParsedCommand::Read {
                        name: "auth.rs".into(),
                        cmd: "cat auth.rs".into(),
                        path: "auth.rs".into(),
                    },
                    ParsedCommand::Read {
                        name: "auth.rs".into(),
                        cmd: "cat auth.rs".into(),
                        path: "auth.rs".into(),
                    },
                    ParsedCommand::Read {
                        name: "shimmer.rs".into(),
                        cmd: "cat shimmer.rs".into(),
                        path: "shimmer.rs".into(),
                    },
                ],
                output: None,
                source: ExecCommandSource::Agent,
                start_time: Some(Instant::now()),
                duration: None,
                interaction_input: None,
            },
            true,
        );
        cell.complete_call("c1", CommandOutput::default(), Duration::from_millis(1));
        let rendered = cell.display_string(80);
        insta::assert_snapshot!(rendered);
    }

    #[test]
    fn multiline_command_wraps_with_extra_indent_on_subsequent_lines() {
        // Create a completed exec cell with a multiline command
        let cmd = "set -o pipefail\ncargo test --all-features --quiet".to_string();
        let call_id = "c1".to_string();
        let mut cell = ExecCell::new(
            ExecCall {
                call_id: call_id.clone(),
                command: vec!["bash".into(), "-lc".into(), cmd],
                parsed: Vec::new(),
                output: None,
                source: ExecCommandSource::Agent,
                start_time: Some(Instant::now()),
                duration: None,
                interaction_input: None,
            },
            true,
        );
        // Mark call complete so it renders as "Ran"
        cell.complete_call(&call_id, CommandOutput::default(), Duration::from_millis(1));

        // Small width to force wrapping on both lines
        let width: u16 = 28;
        let rendered = cell.display_string(width);
        insta::assert_snapshot!(rendered);
    }

    #[test]
    fn single_line_command_compact_when_fits() {
        let call_id = "c1".to_string();
        let mut cell = ExecCell::new(
            ExecCall {
                call_id: call_id.clone(),
                command: vec!["echo".into(), "ok".into()],
                parsed: Vec::new(),
                output: None,
                source: ExecCommandSource::Agent,
                start_time: Some(Instant::now()),
                duration: None,
                interaction_input: None,
            },
            true,
        );
        cell.complete_call(&call_id, CommandOutput::default(), Duration::from_millis(1));
        // Wide enough that it fits inline
        let rendered = cell.display_string(80);
        insta::assert_snapshot!(rendered);
    }

    #[test]
    fn single_line_command_wraps_with_four_space_continuation() {
        let call_id = "c1".to_string();
        let long = "a_very_long_token_without_spaces_to_force_wrapping".to_string();
        let mut cell = ExecCell::new(
            ExecCall {
                call_id: call_id.clone(),
                command: vec!["bash".into(), "-lc".into(), long],
                parsed: Vec::new(),
                output: None,
                source: ExecCommandSource::Agent,
                start_time: Some(Instant::now()),
                duration: None,
                interaction_input: None,
            },
            true,
        );
        cell.complete_call(&call_id, CommandOutput::default(), Duration::from_millis(1));
        let rendered = cell.display_string(24);
        insta::assert_snapshot!(rendered);
    }

    #[test]
    fn multiline_command_without_wrap_uses_branch_then_eight_spaces() {
        let call_id = "c1".to_string();
        let cmd = "echo one\necho two".to_string();
        let mut cell = ExecCell::new(
            ExecCall {
                call_id: call_id.clone(),
                command: vec!["bash".into(), "-lc".into(), cmd],
                parsed: Vec::new(),
                output: None,
                source: ExecCommandSource::Agent,
                start_time: Some(Instant::now()),
                duration: None,
                interaction_input: None,
            },
            true,
        );
        cell.complete_call(&call_id, CommandOutput::default(), Duration::from_millis(1));
        let rendered = cell.display_string(80);
        insta::assert_snapshot!(rendered);
    }

    #[test]
    fn multiline_command_both_lines_wrap_with_correct_prefixes() {
        let call_id = "c1".to_string();
        let cmd = "first_token_is_long_enough_to_wrap\nsecond_token_is_also_long_enough_to_wrap"
            .to_string();
        let mut cell = ExecCell::new(
            ExecCall {
                call_id: call_id.clone(),
                command: vec!["bash".into(), "-lc".into(), cmd],
                parsed: Vec::new(),
                output: None,
                source: ExecCommandSource::Agent,
                start_time: Some(Instant::now()),
                duration: None,
                interaction_input: None,
            },
            true,
        );
        cell.complete_call(&call_id, CommandOutput::default(), Duration::from_millis(1));
        let rendered = cell.display_string(28);
        insta::assert_snapshot!(rendered);
    }

    #[test]
    fn stderr_tail_more_than_five_lines_snapshot() {
        // Build an exec cell with a non-zero exit and 10 lines on stderr to exercise
        // the head/tail rendering and gutter prefixes.
        let call_id = "c_err".to_string();
        let mut cell = ExecCell::new(
            ExecCall {
                call_id: call_id.clone(),
                command: vec!["bash".into(), "-lc".into(), "seq 1 10 1>&2 && false".into()],
                parsed: Vec::new(),
                output: None,
                source: ExecCommandSource::Agent,
                start_time: Some(Instant::now()),
                duration: None,
                interaction_input: None,
            },
            true,
        );
        let stderr: String = (1..=10)
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        cell.complete_call(
            &call_id,
            CommandOutput {
                exit_code: 1,
                formatted_output: String::new(),
                aggregated_output: stderr,
            },
            Duration::from_millis(1),
        );

        let rendered = cell.display_string(80);
        insta::assert_snapshot!(rendered);
    }

    #[test]
    fn ran_cell_multiline_with_stderr_snapshot() {
        // Build an exec cell that completes (so it renders as "Ran") with a
        // command long enough that it must render on its own line under the
        // header, and include a couple of stderr lines to verify the output
        // block prefixes and wrapping.
        let call_id = "c_wrap_err".to_string();
        let long_cmd =
            "echo this_is_a_very_long_single_token_that_will_wrap_across_the_available_width";
        let mut cell = ExecCell::new(
            ExecCall {
                call_id: call_id.clone(),
                command: vec!["bash".into(), "-lc".into(), long_cmd.to_string()],
                parsed: Vec::new(),
                output: None,
                source: ExecCommandSource::Agent,
                start_time: Some(Instant::now()),
                duration: None,
                interaction_input: None,
            },
            true,
        );

        let stderr = "error: first line on stderr\nerror: second line on stderr".to_string();
        cell.complete_call(
            &call_id,
            CommandOutput {
                exit_code: 1,
                formatted_output: String::new(),
                aggregated_output: stderr,
            },
            Duration::from_millis(5),
        );

        // Narrow width to force the command to render under the header line.
        let width: u16 = 28;
        let rendered = cell.display_string(width);
        insta::assert_snapshot!(rendered);
    }
}
