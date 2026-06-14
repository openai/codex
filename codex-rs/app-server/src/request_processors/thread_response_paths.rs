use crate::error_code::internal_error;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_core::ThreadConfigSnapshot;
use codex_utils_path_uri::ApiPathString;
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;

pub(super) fn thread_response_runtime_workspace_roots(
    config_snapshot: &ThreadConfigSnapshot,
    runtime_workspace_roots_explicit: bool,
) -> Result<Vec<ApiPathString>, JSONRPCErrorError> {
    let selected_environment_cwd = config_snapshot
        .environment_selections()
        .first()
        .map(|environment| &environment.cwd);

    config_snapshot
        .workspace_roots
        .iter()
        .map(|root| {
            if !runtime_workspace_roots_explicit
                && root == config_snapshot.cwd()
                && let Some(environment_cwd) = selected_environment_cwd
            {
                let convention = environment_cwd.infer_path_convention().ok_or_else(|| {
                    internal_error(format!(
                        "could not infer the path convention for runtime workspace root `{environment_cwd}`"
                    ))
                })?;
                return render_path_uri(environment_cwd, convention, "runtime workspace root");
            }

            render_path_uri(
                &PathUri::from_abs_path(root),
                PathConvention::native(),
                "runtime workspace root",
            )
        })
        .collect()
}

pub(super) fn thread_response_instruction_sources(
    instruction_sources: &[PathUri],
) -> Result<Vec<ApiPathString>, JSONRPCErrorError> {
    instruction_sources
        .iter()
        .map(|source| {
            let convention = source.infer_path_convention().ok_or_else(|| {
                internal_error(format!(
                    "could not infer the path convention for instruction source `{source}`"
                ))
            })?;
            render_path_uri(source, convention, "instruction source")
        })
        .collect()
}

fn render_path_uri(
    path: &PathUri,
    convention: PathConvention,
    description: &str,
) -> Result<ApiPathString, JSONRPCErrorError> {
    ApiPathString::from_path_uri(path, convention).map_err(|err| {
        internal_error(format!(
            "could not render {description} `{path}` using {convention}: {err}"
        ))
    })
}
