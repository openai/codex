/*
 * Copyright 2019 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

//! Provides utilities useful for implementation of the debug adapter protocol (DAP, see
//! <https://microsoft.github.io/debug-adapter-protocol/>), primarily the DapAdapter/DapAdapterEvalHook
//! that provide for debugging a starlark Evaluation.

use std::fmt::Debug;
use std::fmt::Display;

use debugserver_types::*;
use dupe::Dupe;

use crate::codemap::FileSpan;
use crate::eval::Evaluator;
use crate::syntax::AstModule;
use crate::values::dict::DictRef;
use crate::values::layout::heap::heap_type::Heap;
use crate::values::layout::value::Value;

mod implementation;
mod tests;

/// The DapAdapterClient is implemented by the user and provides functionality required by the DapAdapter.
pub trait DapAdapterClient: Debug + Send + Sync + 'static {
    /// Indicates that the evaluation stopped at a breakpoint.
    fn event_stopped(&self) -> crate::Result<()>;
}

/// Information about the variables scopes
pub struct ScopesInfo {
    /// Number of local variables.
    pub num_locals: usize,
}

/// Information about a "structural variable" inspected by a debugger
/// this currently has DAP-like semantic meaning that every complex object returned
/// by debugger from the stack or from the heap can be broken down into "variables"
/// this is how structured data is managed by the debugger.
/// Something similar to LLDB's SBValue
pub struct Variable {
    /// Name of the variable.
    pub name: PathSegment,
    /// The value as a String.
    pub value: String,
    /// The variables type.
    pub type_: String,
    /// Indicates whether there are children available for a given variable.
    pub has_children: bool,
}

/// Represents the scope of a variable.
#[derive(Clone, Debug)]
pub enum Scope {
    /// A local variable's scope, identified by its name.
    Local(String),
    /// A scope determined by a particular expression.
    #[allow(dead_code)]
    Expr(String),
}

/// Represents a variable's "access path" for a local variable or watch expression.
///
/// # Examples
///
/// - For path `var1.field1[0]`, the scope is `Local("var1")` and the access path is `["field1", 0]`.
/// - For path `someObject.method().something`, the scope is `Expr("someObject.method().something")`. The access path
///   includes segments inside the evaluated result of `someObject.method().something` if it returns a complex object.
#[derive(Clone, Debug)]
pub struct VariablePath {
    scope: Scope,
    access_path: Vec<PathSegment>,
}

impl VariablePath {
    /// creates new instance of VariablePath from a given expression
    pub fn new_expression(expr: impl Into<String>) -> VariablePath {
        VariablePath {
            scope: Scope::Expr(expr.into()),
            access_path: vec![],
        }
    }

    /// creates new instance of VariablePath from a given local variable
    pub fn new_local(scope: impl Into<String>) -> VariablePath {
        VariablePath {
            scope: Scope::Local(scope.into()),
            access_path: vec![],
        }
    }
    /// creates a child segment of given access path
    pub fn make_child(&self, path: PathSegment) -> VariablePath {
        // TODO(vmakaev): figure out if need to optimize memory usage and build persistent data structure
        let mut access_path = self.access_path.clone();
        access_path.push(path);

        Self {
            scope: self.scope.clone(),
            access_path,
        }
    }
}

/// Represents a segment in an access expression.
///
/// For the given expression `name.field1.array\[0\]`, the segments are "field1", "array", and "0".
#[derive(Clone, Debug)]
pub enum PathSegment {
    /// Represents a path segment that accesses array-like types (i.e., types indexable by numbers).
    Index(i32),
    /// Represents a path segment that accesses object-like types (i.e., types keyed by strings).
    Attr(String),
    /// Represents a path segment that accesses dict items by key.
    Key(String),
}

impl Display for PathSegment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PathSegment::Index(x) => write!(f, "{}", x),
            PathSegment::Attr(x) => f.write_str(x),
            PathSegment::Key(x) => write!(f, "\"{}\"", x),
        }
    }
}

impl PathSegment {
    fn get<'v>(&self, v: &Value<'v>, heap: &'v Heap) -> crate::Result<Value<'v>> {
        match self {
            PathSegment::Index(i) => v.at(heap.alloc(*i), heap).map_err(Into::into),
            PathSegment::Attr(key) => v.get_attr_error(key.as_str(), heap),
            PathSegment::Key(i) => v.at(heap.alloc(i.to_owned()), heap).map_err(Into::into),
        }
    }
}

impl Variable {
    /// Helper to convert to the DAP Variable type.
    pub fn to_dap(self) -> debugserver_types::Variable {
        debugserver_types::Variable {
            name: self.name.to_string(),
            value: self.value,
            type_: Some(self.type_),
            evaluate_name: None,
            indexed_variables: None,
            named_variables: None,
            presentation_hint: None,
            variables_reference: 0,
        }
    }

    fn tuple_value_as_str<'v>(v: Value<'v>) -> String {
        match v.length() {
            Ok(size) if size > 0 => format!("<tuple, size={}>", size),
            _ => "()".to_owned(),
        }
    }

    fn list_value_as_str<'v>(v: Value<'v>) -> String {
        match v.length() {
            Ok(size) if size > 0 => format!("<list, size={}>", size),
            _ => "[]".to_owned(),
        }
    }

    fn dict_value_as_str<'v>(v: Value<'v>) -> String {
        match v.length() {
            Ok(size) if size > 0 => format!("<dict, size={}>", size),
            _ => "{}".to_owned(),
        }
    }

    fn struct_like_value_as_str<'v>(v: Value<'v>) -> String {
        let attrs = v.dir_attr();
        format!("<type:{}, size={}>", v.get_type(), attrs.len())
    }

    pub(crate) fn truncate_string(mut str_value: String, mut max_len: usize) -> String {
        if str_value.len() > max_len {
            // Find a valid UTF-8 cut-off point that's within our max length
            while max_len > 0 && !str_value.is_char_boundary(max_len) {
                max_len -= 1;
            }
            if max_len > 0 {
                str_value.truncate(max_len);
                str_value.push_str("...(truncated)");
            }
        }
        str_value
    }

    pub(crate) fn value_as_str<'v>(v: &Value<'v>) -> String {
        if Self::has_children(v) {
            match v.get_type() {
                "list" => Self::list_value_as_str(*v),
                "tuple" => Self::tuple_value_as_str(*v),
                "dict" => Self::dict_value_as_str(*v),
                _ => Self::struct_like_value_as_str(*v),
            }
        } else {
            match v.get_type() {
                "function" => "<function>".to_owned(),
                _ => {
                    const MAX_STR_LEN: usize = 10000;
                    Self::truncate_string(v.to_str(), MAX_STR_LEN)
                }
            }
        }
    }

    /// creates a new instance of Variable from a given starlark value
    pub fn from_value<'v>(name: PathSegment, v: Value<'v>) -> Self {
        Self {
            name,
            value: Self::value_as_str(&v),
            type_: v.get_type().to_owned(),
            has_children: Self::has_children(&v),
        }
    }

    pub(crate) fn has_children<'v>(v: &Value<'v>) -> bool {
        match v.get_type() {
            "function" | "never" | "NoneType" | "bool" | "int" | "float" | "string" => false,
            "list" | "tuple" | "dict" => match v.length() {
                Ok(length) => length > 0,
                _ => false,
            },
            _ => true,
        }
    }
}

/// The kind of debugger step, used for next/stepin/stepout requests.
#[derive(Debug, Clone, Dupe, Copy)]
pub enum StepKind {
    /// Step "into" the statement. This is generally used on a function call to stop in the
    /// function call. In practice, this will stop on the next statement.
    Into,
    /// Step "over" the statement. This will stop on the next statement in the current function
    /// after the current one (so will step "over" a function call).
    Over,
    /// Step "out" of the current function. This will stop on the next statement after this
    /// function returns.
    Out,
}

/// Information about variables in scope.
pub struct VariablesInfo {
    /// Local variables.
    pub locals: Vec<Variable>,
}

/// Information about variable child "sub-values"
#[derive(Default)]
pub struct InspectVariableInfo {
    /// Child variables.
    pub sub_values: Vec<Variable>,
}

/// Information about expression evaluation result
pub struct EvaluateExprInfo {
    /// The value as a String.
    pub result: String,
    /// The variables type.
    pub type_: String,
    /// Indicates whether there are children available for a given variable.
    pub has_children: bool,
}

impl InspectVariableInfo {
    fn try_from_dict<'v>(value_dict: DictRef<'v>) -> crate::Result<Self> {
        let key_segments = value_dict
            .iter()
            .map(|(key, value)| (PathSegment::Key(key.to_str()), value))
            .collect::<Vec<_>>();

        Ok(Self {
            sub_values: key_segments
                .into_iter()
                .map(|(path_segment, value)| Variable::from_value(path_segment, value))
                .collect::<Vec<_>>(),
        })
    }

    fn try_from_struct_like<'v>(v: Value<'v>, heap: &'v Heap) -> crate::Result<Self> {
        Ok(Self {
            sub_values: v
                .dir_attr()
                .into_iter()
                .map(|child_name| {
                    let child_value = v.get_attr_error(&child_name, heap)?;
                    let segment = PathSegment::Attr(child_name);
                    Ok(Variable::from_value(segment, child_value))
                })
                .collect::<crate::Result<Vec<_>>>()?,
        })
    }

    fn try_from_array_like<'v>(v: Value<'v>, heap: &'v Heap) -> crate::Result<Self> {
        let len = v.length()?;
        Ok(Self {
            sub_values: (0..len)
                .map(|i| {
                    let index = heap.alloc(i);
                    v.at(index, heap)
                        .map(|v| Variable::from_value(PathSegment::Index(i), v))
                })
                .collect::<crate::Result<Vec<_>>>()?,
        })
    }

    /// Trying to create InspectVariableInfo from a given starlark value
    pub fn try_from_value<'v>(v: Value<'v>, heap: &'v Heap) -> crate::Result<Self> {
        match v.get_type() {
            "dict" => Self::try_from_dict(
                DictRef::from_value(v).ok_or(anyhow::Error::msg("not a dictionary"))?,
            ),
            "struct" => Self::try_from_struct_like(v, heap),
            "list" | "tuple" => Self::try_from_array_like(v, heap),
            "bool" | "int" | "float" | "string" => Ok(Default::default()),
            "function" | "never" | "NoneType" => Ok(Default::default()),
            // this branch will catch Ty::basic(name)
            _ => Self::try_from_struct_like(v, heap),
        }
    }
}

impl EvaluateExprInfo {
    /// Creating EvaluateExprInfo from a given starlark value
    pub fn from_value(v: &Value) -> Self {
        Self {
            result: Variable::value_as_str(v),
            type_: v.get_type().to_owned(),
            has_children: Variable::has_children(v),
        }
    }
}

/// The DapAdapter accepts DAP requests and updates the hooks in the running evaluator.
pub trait DapAdapter: Debug + Send + 'static {
    /// Sets multiple breakpoints for a file (and clears existing ones).
    ///
    /// See <https://microsoft.github.io/debug-adapter-protocol/specification#Requests_SetBreakpoints>
    fn set_breakpoints(
        &self,
        source: &str,
        breakpoints: &ResolvedBreakpoints,
    ) -> anyhow::Result<()>;

    /// Gets the top stack frame, may be None if entered from native.
    fn top_frame(&self) -> anyhow::Result<Option<StackFrame>>;

    /// Gets a stacktrace from the current execution state.
    ///
    /// See <https://microsoft.github.io/debug-adapter-protocol/specification#Requests_StackTrace>
    fn stack_trace(&self, args: StackTraceArguments) -> anyhow::Result<StackTraceResponseBody>;

    /// Gets the variables scope for a frame.
    ///
    /// See <https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Scopes>
    fn scopes(&self) -> anyhow::Result<ScopesInfo>;

    /// Gets variables for the current scope
    ///
    /// See <https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Variables>
    fn variables(&self) -> anyhow::Result<VariablesInfo>;

    /// Gets all child variables for the given access path
    ///
    /// See <https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Variables>
    fn inspect_variable(&self, path: VariablePath) -> anyhow::Result<InspectVariableInfo>;

    /// Resumes execution.
    ///
    /// See <https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Continue>
    fn continue_(&self) -> anyhow::Result<()>;

    /// Continues execution until some condition.
    ///
    /// See <https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Next>
    /// <https://microsoft.github.io/debug-adapter-protocol/specification#Requests_StepIn>
    /// <https://microsoft.github.io/debug-adapter-protocol/specification#Requests_StepOut>
    fn step(&self, kind: StepKind) -> anyhow::Result<()>;
    /// Evaluates in expression in the context of the top-most frame.
    ///
    /// See <https://microsoft.github.io/debug-adapter-protocol/specification#Requests_Evaluate>
    fn evaluate(&self, expr: &str) -> anyhow::Result<EvaluateExprInfo>;
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub(crate) struct Breakpoint {
    span: FileSpan,
    condition: Option<String>,
}

/// Breakpoints resolved to their spans.
#[derive(Debug)]
pub struct ResolvedBreakpoints(Vec<Option<Breakpoint>>);

impl ResolvedBreakpoints {
    /// Converts resolved breakpoints to a SetBreakpointsResponseBody. The breakpoints should've been resolved from the corresponding SetBreakpointsRequest.
    pub fn to_response(&self) -> SetBreakpointsResponseBody {
        implementation::resolved_breakpoints_to_dap(self)
    }
}

/// Resolves the breakpoints to their FileSpan if possible.
pub fn resolve_breakpoints(
    args: &SetBreakpointsArguments,
    ast: &AstModule,
) -> anyhow::Result<ResolvedBreakpoints> {
    implementation::resolve_breakpoints(args, ast)
}

/// This is sort of the evaluation side of the DapAdapter. It's expected that these are on different threads
/// (the starlark evaluation is single-threaded, so certainly the DapAdapter itself doesn't do interesting
/// things there).
pub trait DapAdapterEvalHook: Debug + Send + 'static {
    /// Hooks the evaluator for this DapAdapter.
    fn add_dap_hooks(self: Box<Self>, eval: &mut Evaluator<'_, '_, '_>);
}

/// The DAP capabilities that the adapter supports.
pub fn dap_capabilities() -> Capabilities {
    Capabilities {
        supports_configuration_done_request: Some(true),
        supports_evaluate_for_hovers: Some(true),
        supports_set_variable: Some(true),
        supports_step_in_targets_request: Some(true),
        supports_conditional_breakpoints: Some(true),
        ..Capabilities::default()
    }
}

/// Creates a DapAdapter and corresponding DapAdapterEvalHook.
pub fn prepare_dap_adapter(
    client: Box<dyn DapAdapterClient>,
) -> (impl DapAdapter, impl DapAdapterEvalHook) {
    implementation::prepare_dap_adapter(client)
}
