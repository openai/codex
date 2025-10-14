#[cfg(test)]
mod api_contract {
    use crate::approval::CommandDecision;
    use crate::approval::assess_command_safety;
    use crate::approval::assess_patch_safety;
    use crate::approval::get_platform_sandbox;
    use crate::exec::SandboxType;
    use crate::protocol::AskForApproval;
    use crate::protocol::SandboxPolicy;
    use codex_apply_patch::ApplyPatchAction;
    use std::collections::HashSet;
    use std::path::Path;

    #[test]
    fn function_signatures_are_stable() {
        let _patch: fn(
            &ApplyPatchAction,
            AskForApproval,
            &SandboxPolicy,
            &Path,
        ) -> CommandDecision = assess_patch_safety;

        let _command: fn(
            &[String],
            AskForApproval,
            &SandboxPolicy,
            &HashSet<Vec<String>>,
            bool,
        ) -> CommandDecision = assess_command_safety;

        let _sandbox: fn() -> Option<SandboxType> = get_platform_sandbox;
    }

    fn assert_traits<T: std::fmt::Debug + PartialEq>() {}

    #[test]
    fn command_decision_traits_and_variants_are_stable() {
        assert_traits::<CommandDecision>();

        let sandbox = SandboxType::None;
        let _ = CommandDecision::permit(sandbox, false);
        let _ = CommandDecision::require_approval();
        let _ = CommandDecision::deny("".to_string());
    }
}

#[cfg(test)]
mod approval_tests {
    use std::collections::HashSet;

    use pretty_assertions::assert_eq as pretty_assert_eq;

    use crate::approval::CommandDecision;
    use crate::approval::ast::CommandAst;
    use crate::approval::ast::SimpleAst;
    use crate::approval::ast::build_ast;
    use crate::approval::classifier;
    use crate::approval::command_rules::COMMAND_RULES;
    use crate::approval::command_rules::CommandCategory;
    use crate::approval::command_rules::CommandMatcher;
    use crate::approval::command_rules::CommandRule;
    use crate::approval::git_model::GitCommand;
    use crate::approval::git_model::GitCommitOptions;
    use crate::approval::git_model::GitResetOptions;
    use crate::approval::git_model::GitSubcommand;
    use crate::approval::git_parser::parse_git_command;
    use crate::approval::git_rules::classify_git_command;
    use crate::approval::parser;
    use crate::approval::policy;
    use crate::approval::rules::predicate_rules;
    use crate::exec::SandboxType;
    use crate::protocol::AskForApproval;
    use crate::protocol::SandboxPolicy;

    fn cmd(parts: &[&str]) -> Vec<String> {
        parts.iter().map(std::string::ToString::to_string).collect()
    }

    fn approved_cache(commands: &[&[&str]]) -> HashSet<Vec<String>> {
        commands
            .iter()
            .map(|command| cmd(command))
            .collect::<HashSet<Vec<String>>>()
    }

    fn classify(parts: &[&str]) -> CommandCategory {
        let simple = parser::normalize_simple(cmd(parts));
        classifier::classify_simple_ast(&simple)
    }

    mod classifier_tests {
        use super::*;

        #[test]
        fn matches_rule_requires_matching_tool() {
            let rule = CommandRule::with_subcommand(
                "tool",
                CommandMatcher::WithSubcommands(&["run"]),
                CommandCategory::ModifiesFilesystem,
            );

            let matching = SimpleAst {
                tool: "tool".to_string(),
                subcommand: Some("run".to_string()),
                flags: vec![],
                operands: vec![],
                raw: cmd(&["tool", "run"]),
            };
            assert!(classifier::matches_rule(&rule, &matching));

            let different_tool = SimpleAst {
                tool: "other".to_string(),
                subcommand: Some("run".to_string()),
                flags: vec![],
                operands: vec![],
                raw: cmd(&["other", "run"]),
            };
            assert!(!classifier::matches_rule(&rule, &different_tool));
        }

        #[test]
        fn classify_simple_ast_uses_git_rules() {
            let category = classify(&["git", "status"]);
            pretty_assert_eq!(category, CommandCategory::ReadsVcs);

            let category = classify(&["git", "reset", "--hard"]);
            pretty_assert_eq!(category, CommandCategory::DeletesData);
        }

        #[test]
        fn classify_simple_ast_falls_back_to_unrecognized() {
            let category = classify(&["npm", "install"]);
            pretty_assert_eq!(category, CommandCategory::Unrecognized);
        }
    }

    mod policy_tests {
        use super::*;
        use crate::approval::policy::aggregate_categories;

        #[test]
        fn read_only_commands_auto_approve_without_sandbox() {
            let command = cmd(&["ls"]);
            let result = policy::assess_command(
                &command,
                AskForApproval::OnRequest,
                &SandboxPolicy::ReadOnly,
                &approved_cache(&[]),
                false,
            );

            pretty_assert_eq!(result, CommandDecision::permit(SandboxType::None, false));
        }

        #[test]
        fn high_risk_commands_prompt_when_not_approved() {
            let command = cmd(&["rm", "-rf", "/"]);
            let result = policy::assess_command(
                &command,
                AskForApproval::OnRequest,
                &SandboxPolicy::ReadOnly,
                &approved_cache(&[]),
                false,
            );

            pretty_assert_eq!(result, CommandDecision::require_approval());
        }

        #[test]
        fn approved_cache_short_circuits_to_auto_approve() {
            let command = cmd(&["rm", "-rf", "/"]);
            let result = policy::assess_command(
                &command,
                AskForApproval::OnRequest,
                &SandboxPolicy::ReadOnly,
                &approved_cache(&[&["rm", "-rf", "/"]]),
                false,
            );

            pretty_assert_eq!(result, CommandDecision::permit(SandboxType::None, true));
        }

        #[test]
        fn escalated_permissions_require_user_confirmation() {
            let command = cmd(&["git", "commit"]);
            let result = policy::assess_command(
                &command,
                AskForApproval::OnRequest,
                &SandboxPolicy::ReadOnly,
                &approved_cache(&[]),
                true,
            );

            pretty_assert_eq!(result, CommandDecision::require_approval());
        }

        #[test]
        fn pipelines_take_highest_risk_category() {
            let command = cmd(&["bash", "-lc", "ls | rm -rf /"]);
            let result = policy::assess_command(
                &command,
                AskForApproval::OnRequest,
                &SandboxPolicy::ReadOnly,
                &approved_cache(&[]),
                false,
            );

            pretty_assert_eq!(result, CommandDecision::require_approval());
        }

        #[test]
        fn aggregate_retains_deletes_data_risk() {
            let categories = vec![
                CommandCategory::Unrecognized,
                CommandCategory::DeletesData,
                CommandCategory::ReadsFilesystem,
            ];
            pretty_assert_eq!(
                aggregate_categories(&categories),
                CommandCategory::DeletesData
            );
        }

        #[test]
        fn aggregate_defaults_to_unrecognized() {
            pretty_assert_eq!(aggregate_categories(&[]), CommandCategory::Unrecognized);
        }

        fn untrusted_on_danger_full_access_relies_on_policy_wrapper() {
            let outcome = policy::evaluate_decision_policy(
                AskForApproval::OnRequest,
                &SandboxPolicy::DangerFullAccess,
                false,
                false,
            );

            pretty_assert_eq!(outcome, CommandDecision::permit(SandboxType::None, false));
        }

        #[test]
        fn rm_classification_logic() {
            // rm without flags is Unrecognized
            assert_eq!(classify(&["rm", "file.txt"]), CommandCategory::Unrecognized);

            // rm with forbidden flags is DeletesData
            assert_eq!(
                classify(&["rm", "-f", "file.txt"]),
                CommandCategory::DeletesData
            );
            assert_eq!(
                classify(&["rm", "-r", "file.txt"]),
                CommandCategory::DeletesData
            );
            assert_eq!(
                classify(&["rm", "-rf", "file.txt"]),
                CommandCategory::DeletesData
            );
            assert_eq!(
                classify(&["rm", "--force", "file.txt"]),
                CommandCategory::DeletesData
            );
            assert_eq!(
                classify(&["rm", "-r", "-f", "file.txt"]),
                CommandCategory::DeletesData
            );
        }

        #[test]
        fn unrecognized_commands_require_approval_under_danger_full_access() {
            // Regression test: DangerFullAccess should not auto-permit Unrecognized commands
            // because we can't assess their safety. This prevents wrapped dangerous commands
            // like "sudo rm -rf /" from bypassing approval.
            let command = cmd(&["sudo", "rm", "-rf", "/"]);
            let result = policy::assess_command(
                &command,
                AskForApproval::OnRequest,
                &SandboxPolicy::DangerFullAccess,
                &approved_cache(&[]),
                false,
            );

            // Unrecognized commands should require approval even under DangerFullAccess
            pretty_assert_eq!(result, CommandDecision::require_approval());
        }

        #[test]
        fn shell_wrapped_dangerous_commands_require_approval() {
            // bash -lc "sudo rm -rf /" contains "sudo" which is Unrecognized
            let command = cmd(&["bash", "-lc", "sudo rm -rf /"]);
            let result = policy::assess_command(
                &command,
                AskForApproval::OnRequest,
                &SandboxPolicy::DangerFullAccess,
                &approved_cache(&[]),
                false,
            );

            // Should require approval since sudo is Unrecognized
            pretty_assert_eq!(result, CommandDecision::require_approval());
        }

        #[test]
        fn unrecognized_with_never_policy_is_denied() {
            // AskForApproval::Never + Unrecognized should deny (can't assess safety)
            let command = cmd(&["sudo", "rm", "-rf", "/"]);
            let result = policy::assess_command(
                &command,
                AskForApproval::Never,
                &SandboxPolicy::DangerFullAccess,
                &approved_cache(&[]),
                false,
            );

            match result {
                CommandDecision::Reject { .. } => {} // Expected
                other => panic!("Expected Reject, got {other:?}"),
            }
        }

        #[test]
        fn unless_trusted_requires_approval_for_modifying_commands() {
            // UnlessTrusted should require approval for ModifiesVcs commands
            let command = cmd(&["git", "commit", "-m", "test"]);
            let result = policy::assess_command(
                &command,
                AskForApproval::UnlessTrusted,
                &SandboxPolicy::DangerFullAccess,
                &approved_cache(&[]),
                false,
            );

            pretty_assert_eq!(result, CommandDecision::require_approval());
        }

        #[test]
        fn modifies_filesystem_under_danger_full_access_auto_approves() {
            // git commit is ModifiesVcs, should auto-approve under DangerFullAccess
            let command = cmd(&["git", "commit", "-m", "test"]);
            let result = policy::assess_command(
                &command,
                AskForApproval::OnRequest,
                &SandboxPolicy::DangerFullAccess,
                &approved_cache(&[]),
                false,
            );

            pretty_assert_eq!(result, CommandDecision::permit(SandboxType::None, false));
        }

        #[test]
        fn modifies_vcs_with_never_policy_under_danger_full_access() {
            // git commit with Never policy + DangerFullAccess should auto-permit
            let command = cmd(&["git", "commit", "-m", "test"]);
            let result = policy::assess_command(
                &command,
                AskForApproval::Never,
                &SandboxPolicy::DangerFullAccess,
                &approved_cache(&[]),
                false,
            );

            pretty_assert_eq!(result, CommandDecision::permit(SandboxType::None, false));
        }

        #[test]
        fn deletes_data_ignores_danger_full_access_policy() {
            // DeletesData should ALWAYS require approval, even with DangerFullAccess + Never
            let command = cmd(&["rm", "-rf", "/"]);
            let result = policy::assess_command(
                &command,
                AskForApproval::Never,
                &SandboxPolicy::DangerFullAccess,
                &approved_cache(&[]),
                false,
            );

            // DeletesData with Never should be denied (not auto-permitted)
            match result {
                CommandDecision::Reject { .. } => {} // Expected
                other => panic!("Expected Reject, got {other:?}"),
            }
        }

        #[test]
        fn workspace_write_policy_with_modifying_commands() {
            // ModifiesVcs under WorkspaceWrite should use sandbox if available, or require approval
            let command = cmd(&["git", "commit", "-m", "test"]);
            let workspace_write = SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![],
                network_access: false,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            };
            let result = policy::assess_command(
                &command,
                AskForApproval::OnRequest,
                &workspace_write,
                &approved_cache(&[]),
                false,
            );

            // Should either be sandboxed or require approval (not DangerFullAccess auto-permit)
            match result {
                CommandDecision::AutoApprove { sandbox_type, .. } => {
                    // Sandboxed approval is fine
                    assert!(sandbox_type != SandboxType::None, "Should be sandboxed");
                }
                CommandDecision::AskUser => {} // Also fine
                other => panic!("Expected sandboxed approval or AskUser, got {other:?}"),
            }
        }

        #[test]
        fn on_failure_policy_uses_sandbox_when_available() {
            // OnFailure: use sandbox when available, otherwise ask user
            let command = cmd(&["git", "commit", "-m", "test"]);
            let result = policy::assess_command(
                &command,
                AskForApproval::OnFailure,
                &SandboxPolicy::ReadOnly,
                &approved_cache(&[]),
                false,
            );

            // Should either be sandboxed or require approval
            match result {
                CommandDecision::AutoApprove { sandbox_type, .. } => {
                    // Sandboxed approval is fine
                    assert!(sandbox_type != SandboxType::None, "Should be sandboxed");
                }
                CommandDecision::AskUser => {} // Also fine if no sandbox available
                other => panic!("Expected sandboxed approval or AskUser, got {other:?}"),
            }
        }

        #[test]
        fn never_policy_with_sandbox_available_uses_sandbox() {
            // Never policy: use sandbox when available, otherwise reject
            let command = cmd(&["git", "commit", "-m", "test"]);
            let result = policy::assess_command(
                &command,
                AskForApproval::Never,
                &SandboxPolicy::ReadOnly,
                &approved_cache(&[]),
                false,
            );

            // Should either be sandboxed or rejected (never AskUser with Never policy)
            match result {
                CommandDecision::AutoApprove { sandbox_type, .. } => {
                    // Sandboxed approval is fine
                    assert!(sandbox_type != SandboxType::None, "Should be sandboxed");
                }
                CommandDecision::Reject { .. } => {} // Also fine if no sandbox available
                CommandDecision::AskUser => {
                    panic!("Never policy should not result in AskUser")
                }
            }
        }

        #[test]
        fn unrecognized_with_workspace_write_requires_approval() {
            // Unrecognized commands should require approval regardless of sandbox policy
            let command = cmd(&["sudo", "rm", "-rf", "/"]);
            let workspace_write = SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![],
                network_access: false,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            };
            let result = policy::assess_command(
                &command,
                AskForApproval::OnRequest,
                &workspace_write,
                &approved_cache(&[]),
                false,
            );

            // Unrecognized always requires approval
            pretty_assert_eq!(result, CommandDecision::require_approval());
        }

        #[test]
        fn read_only_commands_auto_approve_under_all_sandbox_policies() {
            // Read-only commands should auto-approve regardless of sandbox policy
            let workspace_write = SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![],
                network_access: false,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            };

            for sandbox_policy in [
                SandboxPolicy::ReadOnly,
                workspace_write,
                SandboxPolicy::DangerFullAccess,
            ] {
                let command = cmd(&["ls", "-la"]);
                let result = policy::assess_command(
                    &command,
                    AskForApproval::OnRequest,
                    &sandbox_policy,
                    &approved_cache(&[]),
                    false,
                );

                pretty_assert_eq!(
                    result,
                    CommandDecision::permit(SandboxType::None, false),
                    "Failed for {sandbox_policy:?}"
                );
            }
        }
    }

    mod predicate_rule_tests {
        use super::*;

        #[test]
        fn sed_read_only_patterns_are_permitted() {
            assert!(predicate_rules::is_sed_permitted(&cmd(&[
                "sed", "-n", "1,5p", "file.txt"
            ])));
            assert!(predicate_rules::is_sed_permitted(&cmd(&[
                "sed", "-n", "10p", "file.txt"
            ])));
            assert!(predicate_rules::is_sed_permitted(&cmd(&[
                "sed", "-n", "1,100p", "data.log"
            ])));
        }

        #[test]
        fn sed_write_patterns_are_rejected() {
            assert!(!predicate_rules::is_sed_permitted(&cmd(&[
                "sed",
                "s/foo/bar/",
                "file.txt"
            ])));
            assert!(!predicate_rules::is_sed_permitted(&cmd(&[
                "sed", "-n", "1,5", "file.txt"
            ])));
            assert!(!predicate_rules::is_sed_permitted(&cmd(&[
                "sed", "-i", "s/x/y/", "file.txt"
            ])));
            assert!(!predicate_rules::is_sed_permitted(&cmd(&["sed", "-n"])));
        }
    }

    mod shell_parser_tests {
        use super::*;
        use crate::approval::shell_parser::parse_shell_script_commands;

        #[test]
        fn accepts_bash_c_flag() {
            let command = cmd(&["bash", "-c", "ls"]);
            let result = parse_shell_script_commands(&command);
            pretty_assert_eq!(result, Some(vec![cmd(&["ls"])]));
        }

        #[test]
        fn accepts_bash_lc_flag() {
            let command = cmd(&["bash", "-lc", "pwd"]);
            let result = parse_shell_script_commands(&command);
            pretty_assert_eq!(result, Some(vec![cmd(&["pwd"])]));
        }

        #[test]
        fn accepts_sh_c_flag() {
            let command = cmd(&["sh", "-c", "echo hi"]);
            let result = parse_shell_script_commands(&command);
            pretty_assert_eq!(result, Some(vec![cmd(&["echo", "hi"])]));
        }

        #[test]
        fn accepts_zsh_c_flag() {
            let command = cmd(&["zsh", "-c", "ls -la"]);
            let result = parse_shell_script_commands(&command);
            pretty_assert_eq!(result, Some(vec![cmd(&["ls", "-la"])]));
        }

        #[test]
        fn accepts_sh_like_shell_name() {
            let command = cmd(&["mysh", "-c", "pwd"]);
            let result = parse_shell_script_commands(&command);
            pretty_assert_eq!(result, Some(vec![cmd(&["pwd"])]));
        }

        #[test]
        fn rejects_non_shell_like_names() {
            let command = cmd(&["not-a-shell", "-c", "pwd"]);
            pretty_assert_eq!(parse_shell_script_commands(&command), None);
        }

        #[test]
        fn accepts_command_sequences() {
            let command = cmd(&["bash", "-c", "ls && pwd"]);
            let result = parse_shell_script_commands(&command).unwrap();
            pretty_assert_eq!(result, vec![cmd(&["ls"]), cmd(&["pwd"])]);
        }

        #[test]
        fn rejects_redirections() {
            let command = cmd(&["bash", "-c", "ls > out.txt"]);
            pretty_assert_eq!(parse_shell_script_commands(&command), None);
        }

        #[test]
        fn rejects_subshells() {
            let command = cmd(&["bash", "-c", "(ls)"]);
            pretty_assert_eq!(parse_shell_script_commands(&command), None);
        }

        #[test]
        fn rejects_command_substitution() {
            let command = cmd(&["bash", "-c", "echo $(pwd)"]);
            pretty_assert_eq!(parse_shell_script_commands(&command), None);
        }

        #[test]
        fn rejects_wrong_flag() {
            let command = cmd(&["bash", "-x", "ls"]);
            pretty_assert_eq!(parse_shell_script_commands(&command), None);
        }

        #[test]
        fn rejects_if_insufficient_args_but_accepts_extra_args() {
            let too_short = cmd(&["bash", "-c"]);
            pretty_assert_eq!(parse_shell_script_commands(&too_short), None);

            let extra = cmd(&["bash", "-c", "ls", "extra"]);
            assert!(parse_shell_script_commands(&extra).is_some());
        }

        #[test]
        fn dangerous_command_still_parses() {
            let command = cmd(&["bash", "-c", "rm -rf /"]);
            let result = parse_shell_script_commands(&command).unwrap();
            pretty_assert_eq!(result, vec![cmd(&["rm", "-rf", "/"])]);
        }

        #[test]
        fn python_c_flag_is_rejected() {
            let command = cmd(&["python", "-c", "print('hello')"]);
            pretty_assert_eq!(parse_shell_script_commands(&command), None);
        }
    }

    mod parser_tests {
        use super::*;

        #[test]
        fn normalize_simple_extracts_flags_and_operands() {
            let simple = parser::normalize_simple(cmd(&["grep", "-R", "--", "needle", "haystack"]));
            pretty_assert_eq!(simple.tool, "grep");
            pretty_assert_eq!(simple.flags, vec!["-R"]);
            pretty_assert_eq!(simple.operands, vec!["needle", "haystack"]);
        }

        #[test]
        fn normalize_simple_preserves_dash_word_flags() {
            let simple = parser::normalize_simple(cmd(&["find", "/tmp", "-delete"]));
            pretty_assert_eq!(simple.flags, vec!["-delete"]);
            pretty_assert_eq!(simple.subcommand, None);
            pretty_assert_eq!(simple.operands, vec!["/tmp"]);
        }

        #[test]
        fn normalize_simple_expands_short_flag_clusters() {
            let simple = parser::normalize_simple(cmd(&["ls", "-la"]));
            pretty_assert_eq!(simple.flags, vec!["-la", "-l", "-a"]);
        }

        #[test]
        fn normalize_simple_keeps_operands_after_subcommand() {
            let simple = parser::normalize_simple(cmd(&["git", "commit", "-m", "msg"]));
            pretty_assert_eq!(simple.subcommand, Some("commit".to_string()));
            pretty_assert_eq!(simple.flags, vec!["-m"]);
            pretty_assert_eq!(simple.operands, vec!["msg"]);
        }

        #[test]
        fn normalize_simple_detects_cargo_subcommand() {
            let simple = parser::normalize_simple(cmd(&["cargo", "check", "--all"]));
            pretty_assert_eq!(simple.subcommand, Some("check".to_string()));
            pretty_assert_eq!(simple.flags, vec!["--all"]);
        }

        #[test]
        fn parse_to_ast_keeps_sudo() {
            // Sudo is no longer stripped - it's treated as unrecognized for security
            let ast = parser::parse_to_ast(&cmd(&["sudo", "sudo", "/bin/ls", "-l"]));
            let CommandAst::Sequence(simples) = ast else {
                panic!("expected sequence ast");
            };
            pretty_assert_eq!(simples.len(), 1);
            pretty_assert_eq!(simples[0].tool, "sudo");
            // The rest after sudo is treated as operands
        }

        #[test]
        fn parse_to_ast_handles_bash_pipelines() {
            let ast = parser::parse_to_ast(&cmd(&["bash", "-lc", "ls | wc -l"]));
            let CommandAst::Sequence(simples) = ast else {
                panic!("expected sequence ast");
            };
            pretty_assert_eq!(simples.len(), 2);
            pretty_assert_eq!(simples[0].tool, "ls");
            pretty_assert_eq!(simples[1].tool, "wc");
        }

        #[test]
        fn parse_to_ast_handles_bash_c_flag() {
            let ast = parser::parse_to_ast(&cmd(&["bash", "-c", "ls -la"]));
            let CommandAst::Sequence(simples) = ast else {
                panic!("expected sequence ast");
            };
            pretty_assert_eq!(simples.len(), 1);
            pretty_assert_eq!(simples[0].tool, "ls");
            pretty_assert_eq!(simples[0].flags, vec!["-la", "-l", "-a"]);
        }

        #[test]
        fn parse_to_ast_handles_sh_c_flag() {
            let ast = parser::parse_to_ast(&cmd(&["sh", "-c", "pwd"]));
            let CommandAst::Sequence(simples) = ast else {
                panic!("expected sequence ast");
            };
            pretty_assert_eq!(simples.len(), 1);
            pretty_assert_eq!(simples[0].tool, "pwd");
        }

        #[test]
        fn parse_to_ast_handles_zsh_lc_flag() {
            let ast = parser::parse_to_ast(&cmd(&["zsh", "-lc", "echo hi && pwd"]));
            let CommandAst::Sequence(simples) = ast else {
                panic!("expected sequence ast");
            };
            pretty_assert_eq!(simples.len(), 2);
            pretty_assert_eq!(simples[0].tool, "echo");
            pretty_assert_eq!(simples[1].tool, "pwd");
        }

        #[test]
        fn parse_to_ast_handles_dangerous_commands_with_c_flag() {
            let ast = parser::parse_to_ast(&cmd(&["bash", "-c", "rm -rf /"]));
            let CommandAst::Sequence(simples) = ast else {
                panic!("expected sequence ast");
            };
            pretty_assert_eq!(simples.len(), 1);
            pretty_assert_eq!(simples[0].tool, "rm");
            // Classification should still mark this as DeletesData
            let category = classifier::classify_simple_ast(&simples[0]);
            pretty_assert_eq!(category, CommandCategory::DeletesData);
        }

        #[test]
        fn build_ast_returns_tree_for_valid_script() {
            let script = "ls -l | grep src";
            let tree = build_ast(script).expect("expected tree");
            pretty_assert_eq!(tree.kind, "program");
            assert!(!tree.children.is_empty());
        }
    }

    mod git_parser_tests {
        use super::*;

        #[test]
        fn parse_git_command_extracts_commit_message() {
            let command = cmd(&["git", "commit", "-m", "feat: message"]);
            let parsed = parse_git_command(&command);

            pretty_assert_eq!(
                parsed.subcommand,
                GitSubcommand::Commit(GitCommitOptions {
                    message: Some("feat: message".to_string()),
                })
            );
        }

        #[test]
        fn parse_git_command_detects_reset_modes() {
            let hard = parse_git_command(&cmd(&["git", "reset", "--hard"]));
            pretty_assert_eq!(
                hard.subcommand,
                GitSubcommand::Reset(GitResetOptions { hard: true })
            );

            let soft = parse_git_command(&cmd(&["git", "reset", "--soft"]));
            pretty_assert_eq!(
                soft.subcommand,
                GitSubcommand::Reset(GitResetOptions { hard: false })
            );
        }
    }

    mod git_rules_tests {
        use super::*;

        #[test]
        fn classify_git_command_distinguishes_categories() {
            let status = GitCommand {
                subcommand: GitSubcommand::Status,
            };
            pretty_assert_eq!(classify_git_command(&status), CommandCategory::ReadsVcs);

            let add = GitCommand {
                subcommand: GitSubcommand::Add,
            };
            pretty_assert_eq!(classify_git_command(&add), CommandCategory::ModifiesVcs);

            let reset = GitCommand {
                subcommand: GitSubcommand::Reset(GitResetOptions { hard: true }),
            };
            pretty_assert_eq!(classify_git_command(&reset), CommandCategory::DeletesData);
        }
    }

    mod command_engine_tests {
        use super::*;
        use crate::approval::rules_index::build_index;

        #[test]
        fn build_index_contains_known_rules() {
            let index = build_index();
            assert!(index.contains_key("ls"));

            let total_rules: usize = index.values().map(std::vec::Vec::len).sum();
            pretty_assert_eq!(total_rules, COMMAND_RULES.len());
        }
    }
}
