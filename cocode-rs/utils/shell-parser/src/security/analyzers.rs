//! Security analyzers for detecting specific risk patterns.

use once_cell::sync::Lazy;
use regex::Regex;

use crate::parser::ParsedCommand;
use crate::redirects::extract_redirects_from_tree;
use crate::segments::extract_segments_from_tree;
use crate::tokenizer::TokenKind;

use super::risks::RiskKind;
use super::risks::RiskLevel;
use super::risks::SecurityAnalysis;
use super::risks::SecurityRisk;

/// Trait for security analyzers.
pub trait Analyzer {
    /// Analyze a parsed command and add any detected risks to the analysis.
    fn analyze(&self, cmd: &ParsedCommand, analysis: &mut SecurityAnalysis);
}

// =============================================================================
// Allow Phase Analyzers
// =============================================================================

/// Detects dangerous jq operations (system() calls).
pub struct JqDangerAnalyzer;

impl Analyzer for JqDangerAnalyzer {
    fn analyze(&self, cmd: &ParsedCommand, analysis: &mut SecurityAnalysis) {
        static JQ_SYSTEM_RE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r#"\bsystem\s*\("#).expect("valid regex"));

        let commands = cmd.extract_commands();
        for args in &commands {
            // Check if this is a jq command
            if args.first().is_some_and(|c| c == "jq") {
                // Check arguments for system() calls
                for arg in args.iter().skip(1) {
                    if JQ_SYSTEM_RE.is_match(arg) {
                        analysis.add_risk(
                            SecurityRisk::new(
                                RiskKind::JqDanger,
                                "jq command contains system() call which can execute arbitrary commands",
                            )
                            .with_matched_text(arg),
                        );
                    }
                }
            }
        }
    }
}

/// Detects obfuscated flags using $'...' or $"..." syntax.
pub struct ObfuscatedFlagsAnalyzer;

impl Analyzer for ObfuscatedFlagsAnalyzer {
    fn analyze(&self, cmd: &ParsedCommand, analysis: &mut SecurityAnalysis) {
        for token in cmd.tokens() {
            match token.kind {
                TokenKind::AnsiCQuoted => {
                    analysis.add_risk(
                        SecurityRisk::new(
                            RiskKind::ObfuscatedFlags,
                            "ANSI-C quoting ($'...') can hide shell escape sequences",
                        )
                        .with_span(token.span)
                        .with_matched_text(&token.text),
                    );
                }
                TokenKind::LocalizedString => {
                    analysis.add_risk(
                        SecurityRisk::new(
                            RiskKind::ObfuscatedFlags,
                            "localized string ($\"...\") may contain hidden expansions",
                        )
                        .with_span(token.span)
                        .with_matched_text(&token.text),
                    );
                }
                _ => {}
            }
        }
    }
}

/// Detects shell metacharacters in command arguments.
pub struct ShellMetacharactersAnalyzer;

impl Analyzer for ShellMetacharactersAnalyzer {
    fn analyze(&self, cmd: &ParsedCommand, analysis: &mut SecurityAnalysis) {
        static DANGEROUS_METACHAR_RE: Lazy<Regex> = Lazy::new(|| {
            // Look for semicolons, pipes, or ampersands that might be injection
            Regex::new(r#"[;|&]"#).expect("valid regex")
        });

        // Check for dangerous patterns in find/grep -exec or similar
        let commands = cmd.extract_commands();
        for args in &commands {
            let cmd_name = args.first().map(String::as_str).unwrap_or("");
            if matches!(cmd_name, "find" | "xargs") {
                // Check for -exec or similar flags with embedded metacharacters
                for (i, arg) in args.iter().enumerate() {
                    if (arg == "-exec" || arg == "-execdir" || arg == "-ok") && i + 1 < args.len() {
                        // Check the command being executed
                        for exec_arg in &args[i + 1..] {
                            if exec_arg == ";" || exec_arg == "+" {
                                break;
                            }
                            if DANGEROUS_METACHAR_RE.is_match(exec_arg) {
                                analysis.add_risk(
                                    SecurityRisk::new(
                                        RiskKind::ShellMetacharacters,
                                        format!("shell metacharacter in {cmd_name} -exec argument may allow command injection"),
                                    )
                                    .with_matched_text(exec_arg),
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Detects dangerous variable patterns.
pub struct DangerousVariablesAnalyzer;

impl Analyzer for DangerousVariablesAnalyzer {
    fn analyze(&self, cmd: &ParsedCommand, analysis: &mut SecurityAnalysis) {
        // Look for patterns like $VAR | or ${VAR} | that could inject commands
        static VAR_PIPE_RE: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r#"\$\{?[A-Za-z_][A-Za-z0-9_]*\}?\s*\|"#).expect("valid regex")
        });

        let source = cmd.source();
        if VAR_PIPE_RE.is_match(source) {
            analysis.add_risk(SecurityRisk::new(
                RiskKind::DangerousVariables,
                "variable followed by pipe may allow command injection if variable contains newlines",
            ));
        }
    }
}

/// Detects newline injection attempts.
pub struct NewlineInjectionAnalyzer;

impl Analyzer for NewlineInjectionAnalyzer {
    fn analyze(&self, cmd: &ParsedCommand, analysis: &mut SecurityAnalysis) {
        // Check for literal \n followed by what looks like a command
        static NEWLINE_CMD_RE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r#"\\n\s*[a-zA-Z]+"#).expect("valid regex"));

        // Check in double-quoted strings and ANSI-C quotes
        for token in cmd.tokens() {
            let text = match token.kind {
                TokenKind::DoubleQuoted | TokenKind::AnsiCQuoted => &token.text,
                _ => continue,
            };

            if NEWLINE_CMD_RE.is_match(text) {
                analysis.add_risk(
                    SecurityRisk::new(
                        RiskKind::NewlineInjection,
                        "newline escape followed by text may inject commands in some contexts",
                    )
                    .with_span(token.span)
                    .with_matched_text(text),
                );
            }
        }
    }
}

/// Detects IFS manipulation.
pub struct IfsInjectionAnalyzer;

impl Analyzer for IfsInjectionAnalyzer {
    fn analyze(&self, cmd: &ParsedCommand, analysis: &mut SecurityAnalysis) {
        let source = cmd.source();

        // Check for IFS assignment
        if source.contains("IFS=") || source.contains("$IFS") {
            analysis.add_risk(SecurityRisk::new(
                RiskKind::IfsInjection,
                "IFS manipulation can alter word splitting behavior",
            ));
        }
    }
}

/// Detects access to /proc/*/environ.
pub struct ProcEnvironAnalyzer;

impl Analyzer for ProcEnvironAnalyzer {
    fn analyze(&self, cmd: &ParsedCommand, analysis: &mut SecurityAnalysis) {
        static PROC_ENVIRON_RE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r#"/proc/[^/]+/environ"#).expect("valid regex"));

        let source = cmd.source();
        if PROC_ENVIRON_RE.is_match(source) {
            analysis.add_risk(SecurityRisk::new(
                RiskKind::ProcEnvironAccess,
                "accessing /proc/*/environ can expose sensitive environment variables",
            ));
        }
    }
}

// =============================================================================
// Ask Phase Analyzers
// =============================================================================

/// Detects dangerous substitutions ($(), ${}, <(), etc.).
pub struct DangerousSubstitutionAnalyzer;

impl Analyzer for DangerousSubstitutionAnalyzer {
    fn analyze(&self, cmd: &ParsedCommand, analysis: &mut SecurityAnalysis) {
        for token in cmd.tokens() {
            match token.kind {
                TokenKind::CommandSubstitution => {
                    analysis.add_risk(
                        SecurityRisk::new(
                            RiskKind::DangerousSubstitution,
                            "command substitution executes embedded command",
                        )
                        .with_span(token.span)
                        .with_matched_text(&token.text),
                    );
                }
                TokenKind::ProcessSubstitution => {
                    analysis.add_risk(
                        SecurityRisk::new(
                            RiskKind::DangerousSubstitution,
                            "process substitution executes embedded command",
                        )
                        .with_span(token.span)
                        .with_matched_text(&token.text),
                    );
                }
                TokenKind::VariableExpansion => {
                    // Complex expansions like ${VAR:-default} can execute code
                    if token.text.contains(":-")
                        || token.text.contains(":+")
                        || token.text.contains(":?")
                        || token.text.contains("//")
                        || token.text.contains("%%")
                        || token.text.contains("##")
                    {
                        analysis.add_risk(
                            SecurityRisk::new(
                                RiskKind::DangerousSubstitution,
                                "complex parameter expansion may have side effects",
                            )
                            .with_span(token.span)
                            .with_matched_text(&token.text),
                        );
                    }
                }
                _ => {}
            }
        }
    }
}

/// Detects malformed tokens.
pub struct MalformedTokensAnalyzer;

impl Analyzer for MalformedTokensAnalyzer {
    fn analyze(&self, cmd: &ParsedCommand, analysis: &mut SecurityAnalysis) {
        // Check for parse errors in the tree
        if cmd.has_errors() {
            analysis.add_risk(
                SecurityRisk::new(
                    RiskKind::MalformedTokens,
                    "command contains syntax errors which may indicate obfuscation",
                )
                .with_level(RiskLevel::Low),
            );
        }

        // Check for unbalanced brackets/quotes
        let source = cmd.source();
        let mut paren_depth = 0i32;
        let mut brace_depth = 0i32;
        let mut bracket_depth = 0i32;

        for ch in source.chars() {
            match ch {
                '(' => paren_depth += 1,
                ')' => paren_depth -= 1,
                '{' => brace_depth += 1,
                '}' => brace_depth -= 1,
                '[' => bracket_depth += 1,
                ']' => bracket_depth -= 1,
                _ => {}
            }
        }

        if paren_depth != 0 || brace_depth != 0 || bracket_depth != 0 {
            analysis.add_risk(
                SecurityRisk::new(RiskKind::MalformedTokens, "unbalanced brackets detected")
                    .with_level(RiskLevel::Low),
            );
        }
    }
}

/// Detects sensitive file redirections.
pub struct SensitiveRedirectAnalyzer;

impl Analyzer for SensitiveRedirectAnalyzer {
    fn analyze(&self, cmd: &ParsedCommand, analysis: &mut SecurityAnalysis) {
        static SENSITIVE_PATHS: Lazy<Vec<&str>> = Lazy::new(|| {
            vec![
                "/etc/passwd",
                "/etc/shadow",
                "/etc/sudoers",
                "~/.ssh/",
                ".ssh/",
                "id_rsa",
                "id_ed25519",
                ".env",
                ".netrc",
                ".npmrc",
                ".pypirc",
                "credentials",
                "secrets",
                "/dev/tcp",
                "/dev/udp",
            ]
        });

        if let Some(tree) = cmd.tree() {
            let redirects = extract_redirects_from_tree(tree, cmd.source());
            for redirect in redirects {
                for sensitive in SENSITIVE_PATHS.iter() {
                    if redirect.target.contains(sensitive) {
                        let direction = if redirect.kind.is_output() {
                            "writing to"
                        } else {
                            "reading from"
                        };
                        analysis.add_risk(
                            SecurityRisk::new(
                                RiskKind::SensitiveRedirect,
                                format!("{direction} sensitive path: {}", redirect.target),
                            )
                            .with_span(redirect.span),
                        );
                    }
                }

                // Check for /dev/tcp and /dev/udp (network redirects)
                if redirect.target.starts_with("/dev/tcp")
                    || redirect.target.starts_with("/dev/udp")
                {
                    analysis.add_risk(
                        SecurityRisk::new(
                            RiskKind::NetworkExfiltration,
                            format!("network redirection via {}", redirect.target),
                        )
                        .with_span(redirect.span),
                    );
                }
            }
        }
    }
}

/// Detects network exfiltration attempts.
pub struct NetworkExfiltrationAnalyzer;

impl Analyzer for NetworkExfiltrationAnalyzer {
    fn analyze(&self, cmd: &ParsedCommand, analysis: &mut SecurityAnalysis) {
        static EXFIL_CMDS: Lazy<Vec<&str>> = Lazy::new(|| {
            vec![
                "curl", "wget", "nc", "netcat", "ncat", "telnet", "ssh", "scp", "rsync", "ftp",
            ]
        });

        let commands = cmd.extract_commands();
        for args in &commands {
            let cmd_name = args.first().map(String::as_str).unwrap_or("");

            if EXFIL_CMDS.contains(&cmd_name) {
                // Check for data being sent
                let has_data_flag = args.iter().any(|a| {
                    a == "-d"
                        || a == "--data"
                        || a == "-X"
                        || a == "POST"
                        || a == "-F"
                        || a == "--form"
                });

                // Check for piped input
                if let Some(tree) = cmd.tree() {
                    let segments = extract_segments_from_tree(tree, cmd.source());
                    let is_piped = segments
                        .iter()
                        .any(|s| s.command_name() == Some(cmd_name) && s.is_piped);

                    if has_data_flag || is_piped {
                        analysis.add_risk(SecurityRisk::new(
                            RiskKind::NetworkExfiltration,
                            format!("{cmd_name} command may exfiltrate data"),
                        ));
                    }
                }
            }
        }
    }
}

/// Detects privilege escalation attempts.
pub struct PrivilegeEscalationAnalyzer;

impl Analyzer for PrivilegeEscalationAnalyzer {
    fn analyze(&self, cmd: &ParsedCommand, analysis: &mut SecurityAnalysis) {
        static PRIV_ESC_CMDS: Lazy<Vec<&str>> =
            Lazy::new(|| vec!["sudo", "su", "doas", "pkexec", "gksudo", "kdesudo", "runas"]);

        let commands = cmd.extract_commands();
        for args in &commands {
            let cmd_name = args.first().map(String::as_str).unwrap_or("");

            if PRIV_ESC_CMDS.contains(&cmd_name) {
                analysis.add_risk(SecurityRisk::new(
                    RiskKind::PrivilegeEscalation,
                    format!("{cmd_name} command requests elevated privileges"),
                ));
            }

            // Check for setuid/setgid operations
            if cmd_name == "chmod" {
                for arg in args.iter().skip(1) {
                    if arg.contains("s")
                        && (arg.starts_with("u+") || arg.starts_with("g+") || arg.starts_with('+'))
                    {
                        analysis.add_risk(SecurityRisk::new(
                            RiskKind::PrivilegeEscalation,
                            "chmod with setuid/setgid bit",
                        ));
                    }
                    // Numeric mode with setuid/setgid
                    if arg.len() == 4 && arg.chars().all(|c| c.is_ascii_digit()) {
                        let first_digit: i32 = arg[..1].parse().unwrap_or(0);
                        if first_digit >= 4 {
                            analysis.add_risk(SecurityRisk::new(
                                RiskKind::PrivilegeEscalation,
                                "chmod with setuid/setgid bit (numeric mode)",
                            ));
                        }
                    }
                }
            }
        }
    }
}

/// Detects file system tampering.
pub struct FileSystemTamperingAnalyzer;

impl Analyzer for FileSystemTamperingAnalyzer {
    fn analyze(&self, cmd: &ParsedCommand, analysis: &mut SecurityAnalysis) {
        let commands = cmd.extract_commands();

        for args in &commands {
            let cmd_name = args.first().map(String::as_str).unwrap_or("");

            match cmd_name {
                "rm" => {
                    // Check for dangerous rm flags
                    let has_recursive = args
                        .iter()
                        .any(|a| a == "-r" || a == "-R" || a == "--recursive" || a.contains('r'));
                    let has_force = args
                        .iter()
                        .any(|a| a == "-f" || a == "--force" || a.contains('f'));

                    if has_recursive && has_force {
                        analysis.add_risk(SecurityRisk::new(
                            RiskKind::FileSystemTampering,
                            "rm -rf can recursively delete files without confirmation",
                        ));
                    }

                    // Check for dangerous paths
                    for arg in args.iter().skip(1) {
                        if !arg.starts_with('-') {
                            if arg == "/" || arg == "/*" || arg == "~" || arg == "~/*" {
                                analysis.add_risk(
                                    SecurityRisk::new(
                                        RiskKind::FileSystemTampering,
                                        format!("rm targeting dangerous path: {arg}"),
                                    )
                                    .with_level(RiskLevel::Critical),
                                );
                            }
                        }
                    }
                }

                "mkfs" | "dd" | "shred" | "wipefs" => {
                    analysis.add_risk(SecurityRisk::new(
                        RiskKind::FileSystemTampering,
                        format!("{cmd_name} can cause data loss"),
                    ));
                }

                "chown" | "chgrp" => {
                    // Check for recursive operations
                    if args.iter().any(|a| a == "-R" || a == "--recursive") {
                        analysis.add_risk(SecurityRisk::new(
                            RiskKind::FileSystemTampering,
                            format!("recursive {cmd_name} can change ownership of many files"),
                        ));
                    }
                }

                _ => {}
            }
        }
    }
}

/// Detects code execution risks.
pub struct CodeExecutionAnalyzer;

impl Analyzer for CodeExecutionAnalyzer {
    fn analyze(&self, cmd: &ParsedCommand, analysis: &mut SecurityAnalysis) {
        let commands = cmd.extract_commands();

        for args in &commands {
            let cmd_name = args.first().map(String::as_str).unwrap_or("");

            // Direct code execution commands
            if matches!(cmd_name, "eval" | "exec") {
                analysis.add_risk(SecurityRisk::new(
                    RiskKind::CodeExecution,
                    format!("{cmd_name} executes arbitrary code"),
                ));
            }

            // Shell invocations with -c flag
            if matches!(cmd_name, "bash" | "sh" | "zsh") {
                if args.iter().any(|a| a == "-c" || a == "-lc") {
                    analysis.add_risk(SecurityRisk::new(
                        RiskKind::CodeExecution,
                        format!("{cmd_name} -c executes shell code"),
                    ));
                }
            }

            // Interpreter invocations with -c flag or code arguments
            if matches!(cmd_name, "python" | "python3" | "perl" | "ruby" | "php") {
                if args.iter().any(|a| a == "-c" || a == "-e") {
                    analysis.add_risk(SecurityRisk::new(
                        RiskKind::CodeExecution,
                        format!("{cmd_name} executes inline code"),
                    ));
                }
            }

            // Node with -e flag
            if cmd_name == "node" && args.iter().any(|a| a == "-e" || a == "--eval") {
                analysis.add_risk(SecurityRisk::new(
                    RiskKind::CodeExecution,
                    "node --eval executes JavaScript code",
                ));
            }
        }
    }
}

/// Get all default analyzers.
pub fn default_analyzers() -> Vec<Box<dyn Analyzer>> {
    vec![
        // Allow phase
        Box::new(JqDangerAnalyzer),
        Box::new(ObfuscatedFlagsAnalyzer),
        Box::new(ShellMetacharactersAnalyzer),
        Box::new(DangerousVariablesAnalyzer),
        Box::new(NewlineInjectionAnalyzer),
        Box::new(IfsInjectionAnalyzer),
        Box::new(ProcEnvironAnalyzer),
        // Ask phase
        Box::new(DangerousSubstitutionAnalyzer),
        Box::new(MalformedTokensAnalyzer),
        Box::new(SensitiveRedirectAnalyzer),
        Box::new(NetworkExfiltrationAnalyzer),
        Box::new(PrivilegeEscalationAnalyzer),
        Box::new(FileSystemTamperingAnalyzer),
        Box::new(CodeExecutionAnalyzer),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ShellParser;

    fn analyze_command(source: &str) -> SecurityAnalysis {
        let mut parser = ShellParser::new();
        let cmd = parser.parse(source);
        let mut analysis = SecurityAnalysis::new();
        for analyzer in default_analyzers() {
            analyzer.analyze(&cmd, &mut analysis);
        }
        analysis
    }

    #[test]
    fn test_jq_danger() {
        let analysis = analyze_command("jq 'system(\"id\")'");
        assert!(analysis.risks.iter().any(|r| r.kind == RiskKind::JqDanger));
    }

    #[test]
    fn test_obfuscated_flags() {
        let analysis = analyze_command("echo $'hello\\nworld'");
        assert!(
            analysis
                .risks
                .iter()
                .any(|r| r.kind == RiskKind::ObfuscatedFlags)
        );
    }

    #[test]
    fn test_command_substitution() {
        let analysis = analyze_command("echo $(pwd)");
        assert!(
            analysis
                .risks
                .iter()
                .any(|r| r.kind == RiskKind::DangerousSubstitution)
        );
    }

    #[test]
    fn test_privilege_escalation() {
        let analysis = analyze_command("sudo rm -rf /");
        assert!(
            analysis
                .risks
                .iter()
                .any(|r| r.kind == RiskKind::PrivilegeEscalation)
        );
    }

    #[test]
    fn test_rm_rf() {
        let analysis = analyze_command("rm -rf /tmp/*");
        assert!(
            analysis
                .risks
                .iter()
                .any(|r| r.kind == RiskKind::FileSystemTampering)
        );
    }

    #[test]
    fn test_code_execution_eval() {
        let analysis = analyze_command("eval $cmd");
        assert!(
            analysis
                .risks
                .iter()
                .any(|r| r.kind == RiskKind::CodeExecution)
        );
    }

    #[test]
    fn test_safe_command() {
        let analysis = analyze_command("ls -la");
        // Should have no high/critical risks
        assert!(analysis.risks.iter().all(|r| r.level < RiskLevel::High));
    }
}
