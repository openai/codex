mod domain;
mod spec;
mod tool;
mod validation;

pub const REQUEST_PLUGIN_INSTALLS_TOOL_NAME: &str = "request_plugin_installs";
pub(crate) const MAX_REQUEST_PLUGIN_INSTALLS_ENTRIES: usize = 16;

use std::future::Future;
use std::pin::Pin;

use codex_extension_api::FunctionCallError;
pub use domain::REQUEST_PLUGIN_INSTALL_PERSIST_ALWAYS_VALUE;
pub use domain::REQUEST_PLUGIN_INSTALL_PERSIST_KEY;
pub use domain::RequestPluginInstallEntryResult;
pub use domain::RequestPluginInstallInstalledEntry;
pub use domain::RequestPluginInstallPickerCategory;
pub use domain::RequestPluginInstallPickerEntry;
pub use domain::RequestPluginInstallResolvedPickerEntry;
pub use domain::RequestPluginInstallsArgs;
pub use domain::RequestPluginInstallsResult;
pub use domain::all_requested_connectors_picked_up;
pub use domain::build_request_plugin_installs_elicitation_request;
pub use domain::verified_connector_install_completed;
pub use spec::ToolSuggestPresentation;
pub use spec::create_request_plugin_installs_tool;
pub use spec::create_request_plugin_installs_tool_for_tui;
pub use tool::RequestPluginInstallsMode;
pub use tool::request_plugin_installs_tool;
pub use validation::request_plugin_install_picker_completed;

pub type RequestPluginInstallsBackendFuture<'a> = Pin<
    Box<dyn Future<Output = Result<RequestPluginInstallsResult, FunctionCallError>> + Send + 'a>,
>;

pub struct RequestPluginInstallsRequest {
    pub call_id: String,
    pub turn_id: String,
    pub args: RequestPluginInstallsArgs,
    pub resolved_entries: Vec<RequestPluginInstallResolvedPickerEntry>,
}

/// Host operations needed to complete a plugin install suggestion.
pub trait RequestPluginInstallsBackend: Send + Sync {
    fn execute(
        &self,
        request: RequestPluginInstallsRequest,
    ) -> RequestPluginInstallsBackendFuture<'_>;
}
