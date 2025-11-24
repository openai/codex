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

use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::mpsc::channel;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::Mutex;

use debugserver_types::*;
use dupe::Dupe;
use starlark_syntax::error::StarlarkResultExt;
use starlark_syntax::slice_vec_ext::SliceExt;

use super::EvaluateExprInfo;
use super::InspectVariableInfo;
use super::PathSegment;
use super::VariablePath;
use crate::codemap::FileSpan;
use crate::codemap::FileSpanRef;
use crate::codemap::Span;
use crate::debug::adapter::Breakpoint;
use crate::debug::adapter::ResolvedBreakpoints;
use crate::debug::DapAdapter;
use crate::debug::DapAdapterClient;
use crate::debug::DapAdapterEvalHook;
use crate::debug::ScopesInfo;
use crate::debug::StepKind;
use crate::debug::Variable;
use crate::debug::VariablesInfo;
use crate::eval::BeforeStmtFuncDyn;
use crate::eval::Evaluator;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::values::Value;

pub(crate) fn prepare_dap_adapter(
    client: Box<dyn DapAdapterClient>,
) -> (impl DapAdapter, impl DapAdapterEvalHook) {
    let (sender, receiver) = std::sync::mpsc::channel::<ToEvalMessage>();
    let state = Arc::new(SharedAdapterState {
        client,
        breakpoints: Arc::new(Mutex::new(BreakpointConfig::new())),
        disable_breakpoints: Arc::new(0usize.into()),
    });

    (
        DapAdapterImpl {
            state: state.clone(),
            sender,
        },
        DapAdapterEvalHookImpl::new(state, receiver),
    )
}

type ToEvalMessage = Box<dyn Fn(FileSpanRef, &mut Evaluator) -> Next + Send>;

/// The DapAdapter allows
#[derive(Debug)]
struct DapAdapterImpl {
    state: Arc<SharedAdapterState>,
    sender: Sender<ToEvalMessage>,
}

struct DapAdapterEvalHookImpl {
    state: Arc<SharedAdapterState>,
    receiver: Receiver<ToEvalMessage>,
    step: Option<(StepKind, usize)>,
}

fn evaluate_expr<'v>(
    state: &SharedAdapterState,
    eval: &mut Evaluator<'v, '_, '_>,
    expr: String,
) -> anyhow::Result<Value<'v>> {
    // We don't want to trigger breakpoints during an evaluate,
    // not least because we currently don't allow reenterant evaluate
    state.disable_breakpoints.fetch_add(1, Ordering::SeqCst);
    // Don't use `?`, we need to reset disable_breakpoints.
    let ast = AstModule::parse("interactive", expr, &Dialect::AllOptionsInternal);
    // This technically loses structured access to the diagnostic information. However, it's
    // completely unused, so there's not much point in converting all of this code to using
    // `starlark::Error`, only for buck2 to then go and blindly turn it into a `anyhow::Error`
    // anyway.
    let res = ast
        .and_then(|ast| eval.eval_statements(ast))
        .into_anyhow_result();
    state.disable_breakpoints.fetch_sub(1, Ordering::SeqCst);
    res
}

impl<'a, 'e: 'a> BeforeStmtFuncDyn<'a, 'e> for DapAdapterEvalHookImpl {
    fn call<'v>(
        &mut self,
        span_loc: FileSpanRef,
        eval: &mut Evaluator<'v, 'a, 'e>,
    ) -> crate::Result<()> {
        let stop = if self.state.disable_breakpoints.load(Ordering::SeqCst) > 0 {
            false
        } else {
            let breaks = self.state.breakpoints.lock().unwrap();
            let breakpoint = breaks.at(span_loc);
            match breakpoint {
                Some(Breakpoint {
                    condition: Some(condition),
                    ..
                }) => match evaluate_expr(&self.state, eval, condition.to_owned()) {
                    Ok(v) => v.to_bool(),
                    Err(_) => {
                        // If failed to evaluate the condition, stop.
                        // TODO(nga): print the error.
                        true
                    }
                },
                Some(..) => true,
                None => false,
            }
        };

        let step_stop = match self.step {
            None => false,
            Some((StepKind::Into, _)) => true,
            // These aren't quite right because we only get called before statements and so we could
            // return from the current function and be in an expression that then calls another function
            // without hitting a new statement in the outer function.
            Some((StepKind::Over, stack_size)) => eval.call_stack_count() <= stack_size,
            Some((StepKind::Out, stack_size)) => eval.call_stack_count() < stack_size,
        };

        if stop || step_stop {
            self.step = None;
            self.state.client.event_stopped()?;
            loop {
                let msg = self.receiver.recv();
                match msg.map(|msg| msg(span_loc, eval)) {
                    Ok(Next::Continue) => break,
                    Ok(Next::Step(kind)) => {
                        self.step = Some((kind, eval.call_stack_count()));
                        break;
                    }
                    Ok(Next::RemainPaused) => continue,
                    Err(..) => {
                        // DapAdapter has been dropped so we'll continue.
                        break;
                    }
                }
            }
        }
        Ok(())
    }
}

impl Debug for DapAdapterEvalHookImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DapAdapterEvaluationWrapper").finish()
    }
}

impl DapAdapterEvalHookImpl {
    fn new(state: Arc<SharedAdapterState>, receiver: Receiver<ToEvalMessage>) -> Self {
        Self {
            state,
            receiver,
            step: None,
        }
    }
}

impl DapAdapterEvalHook for DapAdapterEvalHookImpl {
    fn add_dap_hooks(self: Box<Self>, eval: &mut Evaluator<'_, '_, '_>) {
        eval.before_stmt_for_dap((self as Box<dyn BeforeStmtFuncDyn>).into());
    }
}

#[derive(Debug)]
struct BreakpointConfig {
    // maps a source filename to the breakpoint spans for the file
    breakpoints: HashMap<String, HashMap<Span, Breakpoint>>,
}

impl BreakpointConfig {
    fn new() -> Self {
        Self {
            breakpoints: HashMap::new(),
        }
    }

    fn at(&self, span_loc: FileSpanRef) -> Option<&Breakpoint> {
        self.breakpoints
            .get(span_loc.filename())
            .and_then(|file_breaks| file_breaks.get(&span_loc.span))
    }

    fn set_breakpoints(
        &mut self,
        source: &str,
        breakpoints: &ResolvedBreakpoints,
    ) -> anyhow::Result<()> {
        if breakpoints.0.is_empty() {
            self.breakpoints.remove(source);
        } else {
            self.breakpoints.insert(
                source.to_owned(),
                breakpoints
                    .0
                    .iter()
                    .filter_map(|x| x.clone())
                    .map(|x| (x.span.span, x))
                    .collect(),
            );
        }
        Ok(())
    }
}

#[derive(Debug)]
struct SharedAdapterState {
    client: Box<dyn DapAdapterClient>,
    // These breakpoints must all match statements as per before_stmt.
    // Those values for which we abort the execution.
    breakpoints: Arc<Mutex<BreakpointConfig>>,
    // Set while we are doing evaluate calls (>= 1 means disable)
    disable_breakpoints: Arc<AtomicUsize>,
}

#[derive(Debug, Clone, Copy, Dupe)]
enum Next {
    Continue,
    RemainPaused,
    Step(StepKind),
}

fn convert_frame(id: usize, name: String, location: Option<FileSpan>) -> StackFrame {
    let mut s = StackFrame {
        id: id as i64,
        name,
        column: 0,
        line: 0,
        end_column: None,
        end_line: None,
        module_id: None,
        presentation_hint: None,
        source: None,
    };
    if let Some(loc) = location {
        let span = loc.resolve_span();
        s.line = span.begin.line as i64 + 1;
        s.column = span.begin.column as i64 + 1;
        s.end_line = Some(span.end.line as i64 + 1);
        s.end_column = Some(span.end.column as i64 + 1);
        s.source = Some(Source {
            path: Some(loc.filename().to_owned()),
            ..Source::default()
        })
    }
    s
}

impl DapAdapter for DapAdapterImpl {
    fn set_breakpoints(
        &self,
        source: &str,
        breakpoints: &ResolvedBreakpoints,
    ) -> anyhow::Result<()> {
        self.state
            .breakpoints
            .lock()
            .unwrap()
            .set_breakpoints(source, breakpoints)
    }

    fn top_frame(&self) -> anyhow::Result<Option<StackFrame>> {
        self.with_ctx(Box::new(|span, eval| {
            let frame = eval.call_stack_top_frame();
            let name = frame.map_or("".to_owned(), |v| v.name);
            Ok(Some(convert_frame(0, name, Some(span.to_file_span()))))
        }))
    }

    fn stack_trace(&self, _: StackTraceArguments) -> anyhow::Result<StackTraceResponseBody> {
        // Our model of a Frame and the debugger model are a bit different.
        // We record the location of the call, but DAP wants the location we are at.
        // We also have them in the wrong order
        self.with_ctx(Box::new(|span, eval| {
            let frames = eval.call_stack().into_frames();
            let mut next = Some(span.to_file_span());
            let mut res = Vec::with_capacity(frames.len() + 1);
            for (i, x) in frames.iter().rev().enumerate() {
                res.push(convert_frame(i, x.name.clone(), next));
                next = x.location.dupe();
            }
            res.push(convert_frame(frames.len(), "Root".to_owned(), next));
            Ok(StackTraceResponseBody {
                total_frames: Some(res.len() as i64),
                stack_frames: res,
            })
        }))
    }

    fn scopes(&self) -> anyhow::Result<ScopesInfo> {
        self.with_ctx(Box::new(|_, eval| {
            let vars = eval.local_variables();
            Ok(ScopesInfo {
                num_locals: vars.len(),
            })
        }))
    }

    fn variables(&self) -> anyhow::Result<VariablesInfo> {
        self.with_ctx(Box::new(|_, eval| {
            let vars = eval.local_variables();
            Ok(VariablesInfo {
                locals: vars
                    .into_iter()
                    .map(|(name, value)| Variable::from_value(PathSegment::Attr(name), value))
                    .collect(),
            })
        }))
    }

    fn inspect_variable(&self, path: VariablePath) -> anyhow::Result<InspectVariableInfo> {
        let state = self.state.dupe();
        self.with_ctx(Box::new(move |_span, eval| {
            let access_path = &path.access_path;
            let mut value = match &path.scope {
                super::Scope::Local(name) => {
                    let mut vars = eval.local_variables();
                    // since vars is owned within this closure scope we can just remove value from the map
                    // obtaining owned variable as the rest of the map will be dropped anyway
                    vars.shift_remove(name).ok_or_else(|| {
                        anyhow::Error::msg(format!("Local variable {} not found", name))
                    })
                }
                super::Scope::Expr(expr) => evaluate_expr(&state, eval, expr.to_owned()),
            }?;

            for p in access_path.iter() {
                value = p.get(&value, eval.heap()).into_anyhow_result()?;
            }
            InspectVariableInfo::try_from_value(value, eval.heap()).into_anyhow_result()
        }))
    }

    fn continue_(&self) -> anyhow::Result<()> {
        self.inject_next(Next::Continue);
        Ok(())
    }

    fn step(&self, kind: StepKind) -> anyhow::Result<()> {
        self.inject_next(Next::Step(kind));
        Ok(())
    }

    fn evaluate(&self, expr: &str) -> anyhow::Result<EvaluateExprInfo> {
        let state = self.state.dupe();
        let expression = expr.to_owned();
        self.with_ctx(Box::new(move |_, eval| {
            match evaluate_expr(&state, eval, expression.clone()) {
                Err(e) => Err(e),
                Ok(v) => Ok(EvaluateExprInfo::from_value(&v)),
            }
        }))
    }
}

impl DapAdapterImpl {
    fn inject<T: 'static + Send>(
        &self,
        f: Box<dyn Fn(FileSpanRef, &mut Evaluator) -> (Next, T) + Send>,
    ) -> T {
        let (sender, receiver) = channel();
        self.sender
            .send(Box::new(move |span, eval| {
                let (next, res) = f(span, eval);
                sender.send(res).unwrap();
                next
            }))
            .unwrap();
        receiver.recv().unwrap()
    }

    fn inject_next(&self, next: Next) {
        self.inject(Box::new(move |_, _| (next, ())))
    }

    fn with_ctx<T: 'static + Send>(
        &self,
        f: Box<dyn Fn(FileSpanRef, &mut Evaluator) -> T + Send>,
    ) -> T {
        self.inject(Box::new(move |span, eval| {
            (Next::RemainPaused, f(span, eval))
        }))
    }
}

pub(crate) fn breakpoint(verified: bool) -> debugserver_types::Breakpoint {
    debugserver_types::Breakpoint {
        column: None,
        end_column: None,
        end_line: None,
        id: None,
        line: None,
        message: None,
        source: None,
        verified,
    }
}

pub(crate) fn resolve_breakpoints(
    args: &SetBreakpointsArguments,
    ast: &AstModule,
) -> anyhow::Result<ResolvedBreakpoints> {
    let poss: HashMap<usize, FileSpan> = ast
        .stmt_locations()
        .iter()
        .map(|span| (span.resolve_span().begin.line, span.dupe()))
        .collect();
    Ok(ResolvedBreakpoints(args.breakpoints.as_ref().map_or(
        Vec::new(),
        |v| {
            v.map(|x| {
                poss.get(&(x.line as usize - 1)).map(|span| Breakpoint {
                    span: span.clone(),
                    condition: x.condition.clone(),
                })
            })
        },
    )))
}

pub(crate) fn resolved_breakpoints_to_dap(
    breakpoints: &ResolvedBreakpoints,
) -> SetBreakpointsResponseBody {
    SetBreakpointsResponseBody {
        breakpoints: breakpoints.0.map(|x| breakpoint(x.is_some())),
    }
}
