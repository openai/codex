use crate::exec_env::create_env;
use crate::exec_policy::ExecApprovalRequest;
use crate::function_tool::FunctionCallError;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::session::turn_context::TurnEnvironment;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::dependency_check_output::blocked_output;
use crate::tools::handlers::dependency_check_output::command_failure_message;
use crate::tools::handlers::dependency_check_output::command_failure_output;
use crate::tools::handlers::dependency_check_output::format_blocked_policy;
use crate::tools::handlers::dependency_check_output::format_success;
use crate::tools::handlers::dependency_check_output::graph_mismatch_output;
use crate::tools::handlers::dependency_check_output::installed_graph_mismatch_output;
use crate::tools::handlers::parse_arguments;
use crate::tools::handlers::resolve_tool_environment;
use crate::tools::orchestrator::ToolOrchestrator;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use crate::tools::runtimes::shell::ShellRequest;
use crate::tools::runtimes::shell::ShellRuntime;
use crate::tools::runtimes::shell::ShellRuntimeBackend;
use crate::tools::sandboxing::ApprovalReviewMode;
use crate::tools::sandboxing::ToolCtx;
use codex_dependency_check::DependencyCheckRequest;
use codex_dependency_check::DependencyInstallCommand;
use codex_dependency_check::DependencyPolicyAction;
use codex_dependency_check::NpmGraph;
use codex_dependency_check::NpmInstalledGraph;
use codex_dependency_check::OsvClient;
use codex_dependency_check::detect_dependency_install_command;
use codex_dependency_check::npm_ci_command;
use codex_dependency_check::npm_install_command;
use codex_dependency_check::npm_query_installed_command;
use codex_dependency_check::npm_query_lock_command;
use codex_dependency_check::npm_rebuild_command;
use codex_dependency_check::validate_npm_manifest;
use codex_protocol::exec_output::ExecToolCallOutput;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::SandboxPermissions;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use codex_tools::create_dependency_check_tool;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::path::Path;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub struct DependencyCheckHandler;

const UNSUPPORTED_LOCKFILES: &[&str] = &[
    "npm-shrinkwrap.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "bun.lock",
    "bun.lockb",
];

impl ToolExecutor<ToolInvocation> for DependencyCheckHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain("dependency_check")
    }

    fn spec(&self) -> ToolSpec {
        create_dependency_check_tool()
    }

    fn handle(&self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'_> {
        Box::pin(async move { self.handle_call(invocation).await.map(boxed_tool_output) })
    }
}

impl DependencyCheckHandler {
    async fn handle_call(
        &self,
        invocation: ToolInvocation,
    ) -> Result<FunctionToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            step_context,
            cancellation_token,
            call_id,
            tool_name,
            payload,
            ..
        } = invocation;
        let ToolPayload::Function { arguments } = payload else {
            return Err(FunctionCallError::RespondToModel(
                "dependency_check received an unsupported payload".to_string(),
            ));
        };
        let request: DependencyCheckRequest = parse_arguments(&arguments)?;
        request
            .validate()
            .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))?;
        let Some(turn_environment) =
            resolve_tool_environment(&step_context.environments, /*environment_id*/ None)?
        else {
            return Err(FunctionCallError::RespondToModel(
                "dependency_check requires a primary environment".to_string(),
            ));
        };
        let workdir = request
            .workdir
            .as_deref()
            .filter(|workdir| !workdir.is_empty())
            .map_or_else(
                || Ok(turn_environment.cwd().clone()),
                |workdir| turn_environment.cwd().join(workdir),
            )
            .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))?;
        let workdir = workdir
            .to_abs_path()
            .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))?;

        let manifest_path = workdir.join("package.json");
        let manifest = read_regular_file(&manifest_path).await.map_err(|err| {
            FunctionCallError::RespondToModel(format!(
                "dependency_check could not read {}: {err}",
                manifest_path.display()
            ))
        })?;
        validate_npm_manifest(&manifest).map_err(|err| {
            FunctionCallError::RespondToModel(format!(
                "dependency_check cannot use this project: {err}"
            ))
        })?;
        if let Some(lockfile) = first_existing_file(&workdir, UNSUPPORTED_LOCKFILES).await {
            return Ok(blocked_output(format!(
                "Dependency Check did not modify the project because `{lockfile}` is not supported by the npm-first implementation."
            )));
        }

        let resolution_dir = tempfile::Builder::new()
            .prefix("codex-dependency-check-")
            .tempdir()
            .map_err(|err| {
                FunctionCallError::RespondToModel(format!(
                    "dependency_check could not create its temporary resolution directory: {err}"
                ))
            })?;
        copy_resolution_inputs(&workdir, resolution_dir.path()).await?;

        let runner = DependencyCommandRunner {
            session: session.clone(),
            turn: turn.clone(),
            turn_environment: turn_environment.clone(),
            cancellation_token,
            call_id: call_id.clone(),
            tool_name: tool_name.clone(),
        };
        let resolve = runner
            .run(
                npm_install_command(Some(&request), /*package_lock_only*/ true),
                resolution_dir.path(),
                ScriptPolicy::Disabled,
                WorkingDirectoryAccess::PreapprovedScratch,
                "Resolve the npm dependency graph with lifecycle scripts disabled.",
            )
            .await?;
        if resolve.exit_code != 0 {
            return Ok(command_failure_output(
                "temporary graph resolution",
                &resolve,
            ));
        }
        let checked_graph = match query_graph(
            &runner,
            resolution_dir.path(),
            WorkingDirectoryAccess::PreapprovedScratch,
        )
        .await?
        {
            Ok(graph) => graph,
            Err(message) => return Ok(blocked_output(message)),
        };

        let policy = match OsvClient::new() {
            Ok(client) => match client.evaluate(&checked_graph).await {
                Ok(report) => report,
                Err(err) => {
                    return Ok(blocked_output(format!(
                        "Dependency Check stopped before modifying the project because OSV did not return complete evidence: {err}"
                    )));
                }
            },
            Err(err) => {
                return Ok(blocked_output(format!(
                    "Dependency Check stopped before modifying the project because the OSV client could not start: {err}"
                )));
            }
        };
        if policy.action == DependencyPolicyAction::Block {
            return Ok(blocked_output(format_blocked_policy(&policy)));
        }

        let lock_update = runner
            .run(
                npm_install_command(Some(&request), /*package_lock_only*/ true),
                &workdir,
                ScriptPolicy::Disabled,
                WorkingDirectoryAccess::ProjectDependencyFiles,
                "Update package.json and package-lock.json only after the resolved graph passed dependency policy.",
            )
            .await?;
        if lock_update.exit_code != 0 {
            return Ok(command_failure_output("project lock update", &lock_update));
        }
        let locked_graph =
            match query_graph(&runner, &workdir, WorkingDirectoryAccess::Default).await? {
                Ok(graph) => graph,
                Err(message) => return Ok(blocked_output(message)),
            };
        if let Err(mismatch) = checked_graph.compare(&locked_graph) {
            return Ok(graph_mismatch_output("project lock update", &mismatch));
        }

        let install = runner
            .run(
                npm_ci_command(),
                &workdir,
                ScriptPolicy::Disabled,
                WorkingDirectoryAccess::Default,
                "Cleanly install the already-checked npm graph while lifecycle scripts remain disabled.",
            )
            .await?;
        if install.exit_code != 0 {
            return Ok(command_failure_output("script-disabled install", &install));
        }
        let installed_graph = match query_installed_graph(&runner, &workdir).await? {
            Ok(graph) => graph,
            Err(message) => return Ok(blocked_output(message)),
        };
        if let Err(mismatch) = checked_graph.compare_installed(&installed_graph) {
            return Ok(installed_graph_mismatch_output(&mismatch));
        }

        let rebuild = runner
            .run(
                npm_rebuild_command(),
                &workdir,
                ScriptPolicy::Enabled,
                WorkingDirectoryAccess::Default,
                "Run npm lifecycle scripts only after the installed lock graph matches the graph checked by OSV.",
            )
            .await?;
        if rebuild.exit_code != 0 {
            return Ok(command_failure_output("npm rebuild", &rebuild));
        }

        Ok(FunctionToolOutput::from_text(
            format_success(&request, &policy, checked_graph.packages().len()),
            Some(true),
        ))
    }
}

impl CoreToolRuntime for DependencyCheckHandler {}

pub(crate) fn dependency_install_redirect(command: &[String]) -> Option<String> {
    let DependencyInstallCommand { package_manager } = detect_dependency_install_command(command)?;
    Some(format!(
        "Dependency Check is enabled. Do not add dependencies with a raw `{}` command. Use the `dependency_check` tool with exact package names and versions so Codex can resolve and inspect the complete graph before lifecycle scripts run. The first implementation supports npm projects only.",
        package_manager.command_name()
    ))
}

pub(crate) fn dependency_manifest_edit_message() -> String {
    "Dependency Check is enabled. Do not edit JavaScript dependency manifests or lockfiles directly. Use the `dependency_check` tool with exact npm package names and versions so the generated graph is checked before lifecycle scripts run. The first implementation supports npm projects only."
        .to_string()
}

async fn query_graph(
    runner: &DependencyCommandRunner,
    cwd: &Path,
    working_directory_access: WorkingDirectoryAccess,
) -> Result<Result<NpmGraph, String>, FunctionCallError> {
    let query = runner
        .run(
            npm_query_lock_command(),
            cwd,
            ScriptPolicy::Disabled,
            working_directory_access,
            "Read the exact npm lock graph for dependency verification.",
        )
        .await?;
    if query.exit_code != 0 {
        return Ok(Err(command_failure_message("npm graph query", &query)));
    }
    Ok(NpmGraph::from_query_json(&query.stdout.text)
        .map_err(|err| format!("Dependency Check stopped before lifecycle scripts because npm returned an unverifiable graph: {err}")))
}

async fn query_installed_graph(
    runner: &DependencyCommandRunner,
    cwd: &Path,
) -> Result<Result<NpmInstalledGraph, String>, FunctionCallError> {
    let query = runner
        .run(
            npm_query_installed_command(),
            cwd,
            ScriptPolicy::Disabled,
            WorkingDirectoryAccess::Default,
            "Read the installed npm artifact graph for dependency verification.",
        )
        .await?;
    if query.exit_code != 0 {
        return Ok(Err(command_failure_message(
            "installed npm graph query",
            &query,
        )));
    }
    Ok(NpmInstalledGraph::from_query_json(&query.stdout.text).map_err(|err| {
        format!(
            "Dependency Check stopped before lifecycle scripts because npm returned an unverifiable installed graph: {err}"
        )
    }))
}

struct DependencyCommandRunner {
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    turn_environment: TurnEnvironment,
    cancellation_token: CancellationToken,
    call_id: String,
    tool_name: ToolName,
}

#[derive(Clone, Copy)]
enum ScriptPolicy {
    Disabled,
    Enabled,
}

#[derive(Clone, Copy)]
enum WorkingDirectoryAccess {
    Default,
    PreapprovedScratch,
    ProjectDependencyFiles,
}

struct CommandPermissions {
    sandbox_permissions: SandboxPermissions,
    additional_permissions: Option<AdditionalPermissionProfile>,
    #[cfg(unix)]
    additional_permissions_preapproved: bool,
}

impl DependencyCommandRunner {
    async fn run(
        &self,
        command: Vec<String>,
        cwd: &Path,
        script_policy: ScriptPolicy,
        working_directory_access: WorkingDirectoryAccess,
        justification: &str,
    ) -> Result<ExecToolCallOutput, FunctionCallError> {
        let mut env = create_env(
            &self.turn.config.permissions.shell_environment_policy,
            Some(self.session.thread_id),
        );
        let ignore_scripts = if matches!(script_policy, ScriptPolicy::Disabled) {
            "true"
        } else {
            "false"
        };
        env.insert(
            "NPM_CONFIG_IGNORE_SCRIPTS".to_string(),
            ignore_scripts.to_string(),
        );
        let mut explicit_env_overrides = self
            .turn
            .config
            .permissions
            .shell_environment_policy
            .r#set
            .clone();
        explicit_env_overrides.insert(
            "NPM_CONFIG_IGNORE_SCRIPTS".to_string(),
            ignore_scripts.to_string(),
        );

        let cwd = AbsolutePathBuf::from_absolute_path(cwd).map_err(|err| {
            FunctionCallError::RespondToModel(format!(
                "dependency_check command paths must be absolute: {err}"
            ))
        })?;
        let command_permissions = command_permissions(cwd.as_path(), working_directory_access)?;
        #[cfg(unix)]
        let approval_sandbox_permissions = if command_permissions.additional_permissions_preapproved
        {
            SandboxPermissions::UseDefault
        } else {
            command_permissions.sandbox_permissions
        };
        #[cfg(not(unix))]
        let approval_sandbox_permissions = command_permissions.sandbox_permissions;
        let exec_approval_requirement = self
            .session
            .services
            .exec_policy
            .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                command: &command,
                approval_policy: self.turn.approval_policy.value(),
                permission_profile: self.turn.permission_profile(),
                windows_sandbox_level: self.turn.windows_sandbox_level,
                sandbox_permissions: approval_sandbox_permissions,
                prefix_rule: None,
            })
            .await;
        let hook_command = codex_shell_command::parse_command::shlex_join(&command);
        let request = ShellRequest {
            command,
            turn_environment: self.turn_environment.clone(),
            shell_type: None,
            hook_command,
            cwd,
            timeout_ms: Some(120_000),
            cancellation_token: self.cancellation_token.clone(),
            env,
            explicit_env_overrides,
            network: self.turn.network.clone(),
            sandbox_permissions: command_permissions.sandbox_permissions,
            additional_permissions: command_permissions.additional_permissions,
            #[cfg(unix)]
            additional_permissions_preapproved: command_permissions
                .additional_permissions_preapproved,
            justification: Some(justification.to_string()),
            exec_approval_requirement,
            approval_review_mode: approval_review_mode(working_directory_access),
        };
        let tool_ctx = ToolCtx {
            session: self.session.clone(),
            turn: self.turn.clone(),
            call_id: self.call_id.clone(),
            tool_name: self.tool_name.clone(),
        };
        let mut orchestrator = ToolOrchestrator::new();
        let mut runtime = ShellRuntime::for_shell_command(ShellRuntimeBackend::ShellCommandClassic);
        orchestrator
            .run(
                &mut runtime,
                &request,
                &tool_ctx,
                self.turn.as_ref(),
                self.turn.approval_policy.value(),
            )
            .await
            .map(|result| result.output)
            .map_err(|err| {
                FunctionCallError::RespondToModel(format!(
                    "dependency_check command execution failed: {err:?}"
                ))
            })
    }
}

fn approval_review_mode(working_directory_access: WorkingDirectoryAccess) -> ApprovalReviewMode {
    match working_directory_access {
        WorkingDirectoryAccess::ProjectDependencyFiles => ApprovalReviewMode::User,
        WorkingDirectoryAccess::Default | WorkingDirectoryAccess::PreapprovedScratch => {
            ApprovalReviewMode::Configured
        }
    }
}

fn command_permissions(
    cwd: &Path,
    working_directory_access: WorkingDirectoryAccess,
) -> Result<CommandPermissions, FunctionCallError> {
    let (write_paths, _additional_permissions_preapproved) = match working_directory_access {
        WorkingDirectoryAccess::Default => {
            return Ok(CommandPermissions {
                sandbox_permissions: SandboxPermissions::UseDefault,
                additional_permissions: None,
                #[cfg(unix)]
                additional_permissions_preapproved: false,
            });
        }
        WorkingDirectoryAccess::PreapprovedScratch => {
            #[cfg(not(unix))]
            return Ok(CommandPermissions {
                sandbox_permissions: SandboxPermissions::UseDefault,
                additional_permissions: None,
            });

            #[cfg(unix)]
            (vec![cwd.to_path_buf()], true)
        }
        WorkingDirectoryAccess::ProjectDependencyFiles => (
            vec![cwd.join("package.json"), cwd.join("package-lock.json")],
            false,
        ),
    };
    let write_paths = write_paths
        .into_iter()
        .map(AbsolutePathBuf::from_absolute_path)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| {
            FunctionCallError::RespondToModel(format!(
                "dependency_check command paths must be absolute: {err}"
            ))
        })?;
    Ok(CommandPermissions {
        sandbox_permissions: SandboxPermissions::WithAdditionalPermissions,
        additional_permissions: Some(AdditionalPermissionProfile {
            file_system: Some(FileSystemPermissions {
                entries: write_paths
                    .into_iter()
                    .map(|path| FileSystemSandboxEntry {
                        path: FileSystemPath::Path { path },
                        access: FileSystemAccessMode::Write,
                    })
                    .collect(),
                glob_scan_max_depth: None,
            }),
            ..Default::default()
        }),
        #[cfg(unix)]
        additional_permissions_preapproved: _additional_permissions_preapproved,
    })
}

async fn read_regular_file(path: &Path) -> std::io::Result<String> {
    let metadata = tokio::fs::symlink_metadata(path).await?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(std::io::Error::other("expected a regular file"));
    }
    tokio::fs::read_to_string(path).await
}

async fn first_existing_file(cwd: &Path, names: &[&str]) -> Option<String> {
    for name in names {
        if tokio::fs::symlink_metadata(cwd.join(name)).await.is_ok() {
            return Some((*name).to_string());
        }
    }
    None
}

async fn copy_resolution_inputs(
    source: &Path,
    destination: &Path,
) -> Result<(), FunctionCallError> {
    for name in ["package.json", "package-lock.json", ".npmrc"] {
        let source_path = source.join(name);
        let metadata = match tokio::fs::symlink_metadata(&source_path).await {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => {
                return Err(FunctionCallError::RespondToModel(format!(
                    "dependency_check could not inspect {}: {err}",
                    source_path.display()
                )));
            }
        };
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(FunctionCallError::RespondToModel(format!(
                "dependency_check requires {} to be a regular file",
                source_path.display()
            )));
        }
        tokio::fs::copy(&source_path, destination.join(name))
            .await
            .map_err(|err| {
                FunctionCallError::RespondToModel(format!(
                    "dependency_check could not copy {} into its temporary resolution directory: {err}",
                    source_path.display()
                ))
            })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_permissions(paths: Vec<AbsolutePathBuf>) -> AdditionalPermissionProfile {
        AdditionalPermissionProfile {
            file_system: Some(FileSystemPermissions {
                entries: paths
                    .into_iter()
                    .map(|path| FileSystemSandboxEntry {
                        path: FileSystemPath::Path { path },
                        access: FileSystemAccessMode::Write,
                    })
                    .collect(),
                glob_scan_max_depth: None,
            }),
            ..Default::default()
        }
    }

    #[test]
    fn raw_install_redirect_names_package_manager() {
        let message = dependency_install_redirect(&[
            "/bin/zsh".to_string(),
            "-lc".to_string(),
            "pnpm add zod@3.23.8".to_string(),
        ])
        .expect("redirect");
        assert!(message.contains("raw `pnpm` command"));
        assert!(message.contains("dependency_check"));
    }

    #[cfg(unix)]
    #[test]
    fn scratch_commands_receive_preapproved_directory_access() {
        let scratch = tempfile::tempdir().expect("scratch directory");
        let permissions =
            command_permissions(scratch.path(), WorkingDirectoryAccess::PreapprovedScratch)
                .expect("command permissions");

        assert_eq!(
            permissions.sandbox_permissions,
            SandboxPermissions::WithAdditionalPermissions
        );
        assert!(permissions.additional_permissions_preapproved);
        assert_eq!(
            permissions.additional_permissions,
            Some(write_permissions(vec![
                AbsolutePathBuf::from_absolute_path(scratch.path()).expect("absolute scratch path")
            ]))
        );
    }

    #[test]
    fn project_lock_update_requests_dependency_file_access() {
        let project = tempfile::tempdir().expect("project directory");
        let permissions = command_permissions(
            project.path(),
            WorkingDirectoryAccess::ProjectDependencyFiles,
        )
        .expect("command permissions");

        assert_eq!(
            permissions.sandbox_permissions,
            SandboxPermissions::WithAdditionalPermissions
        );
        #[cfg(unix)]
        assert!(!permissions.additional_permissions_preapproved);
        assert_eq!(
            permissions.additional_permissions,
            Some(write_permissions(vec![
                AbsolutePathBuf::from_absolute_path(project.path().join("package.json"))
                    .expect("absolute package.json path"),
                AbsolutePathBuf::from_absolute_path(project.path().join("package-lock.json"))
                    .expect("absolute package-lock.json path"),
            ]))
        );
        assert_eq!(
            approval_review_mode(WorkingDirectoryAccess::ProjectDependencyFiles),
            ApprovalReviewMode::User
        );
    }
}
