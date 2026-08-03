use crate::FunctionCallError;
use crate::ToolName;
use crate::ToolOutput;
use crate::ToolSearchInfo;
use crate::ToolSpec;
use codex_protocol::config_types::ToolExposureSurface;
use std::future::Future;
use std::pin::Pin;

/// The boxed future returned by [`ToolExecutor::handle`].
pub type ToolExecutorFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Box<dyn ToolOutput>, FunctionCallError>> + Send + 'a>>;

bitflags::bitflags! {
    /// Independent model-facing surfaces supported by a tool.
    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    pub struct ToolExposures: u8 {
        /// Keep the tool registered without making it model-visible.
        const NONE = 0;
        /// Include the tool in the initial model-visible tool list.
        const DIRECT = 0b001;
        /// Make the tool discoverable through tool search.
        const DEFERRED = 0b010;
        /// Make the tool callable from nested Code Mode scripts.
        const CODE_MODE = 0b100;
        /// Permit every supported model-facing surface.
        const ALL = Self::DIRECT.bits() | Self::DEFERRED.bits() | Self::CODE_MODE.bits();
    }
}

impl From<ToolExposureSurface> for ToolExposures {
    fn from(surface: ToolExposureSurface) -> Self {
        match surface {
            ToolExposureSurface::CodeMode => Self::CODE_MODE,
            ToolExposureSurface::Deferred => Self::DEFERRED,
            ToolExposureSurface::Direct => Self::DIRECT,
        }
    }
}

impl FromIterator<ToolExposureSurface> for ToolExposures {
    fn from_iter<T: IntoIterator<Item = ToolExposureSurface>>(surfaces: T) -> Self {
        surfaces
            .into_iter()
            .fold(Self::NONE, |exposures, surface| exposures | surface.into())
    }
}

/// Controls where a tool is exposed to the model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolExposure {
    /// Include this tool in the initial model-visible tool list.
    ///
    /// When code mode is enabled, this tool is also available as a nested
    /// code-mode tool.
    Direct,

    /// Register this tool for later discovery, but omit it from the initial
    /// model-visible tool list. Deferred tools must provide search metadata via
    /// [`ToolExecutor::search_info`]. The default implementation derives
    /// metadata from function and namespace specs.
    Deferred,

    /// Make this tool discoverable through tool search without allowing nested
    /// Code Mode calls.
    DeferredModelOnly,

    /// Include this tool in the initial model-visible tool list only.
    ///
    /// In code-mode-only sessions, this keeps the tool callable as a normal
    /// model tool while excluding it from the nested code-mode tool surface.
    DirectModelOnly,

    /// Expose this tool only to nested Code Mode calls, without including it in
    /// the initial model-visible tool list or making it available to tool search.
    CodeModeOnly,

    /// Keep this tool registered for dispatch without exposing it to the model.
    Hidden,
}

impl ToolExposure {
    pub fn is_direct(self) -> bool {
        matches!(self, Self::Direct | Self::DirectModelOnly)
    }

    /// Returns whether this tool can be discovered through tool search.
    pub fn is_deferred(self) -> bool {
        matches!(self, Self::Deferred | Self::DeferredModelOnly)
    }

    /// Returns whether this tool can participate in code mode.
    pub fn is_available_in_code_mode(self) -> bool {
        match self {
            Self::Direct | Self::Deferred | Self::CodeModeOnly => true,
            Self::DirectModelOnly | Self::DeferredModelOnly | Self::Hidden => false,
        }
    }
}

/// Shared runtime contract for model-visible tools.
///
/// Implementations keep the model-visible spec tied to the executable runtime.
/// Host crates can layer routing, hooks, telemetry, or other orchestration on
/// top without reopening the spec/runtime split.
pub trait ToolExecutor<Invocation>: Send + Sync {
    /// The concrete tool name handled by this runtime instance.
    fn tool_name(&self) -> ToolName;

    fn spec(&self) -> ToolSpec;

    /// The preferred exposure before the host applies step-specific policy.
    fn exposure(&self) -> ToolExposure {
        ToolExposure::Direct
    }

    fn search_info(&self) -> Option<ToolSearchInfo> {
        let spec = self.spec();
        ToolSearchInfo::from_tool_spec(spec, /*source_info*/ None)
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        false
    }

    fn handle(&self, invocation: Invocation) -> ToolExecutorFuture<'_>;
}
