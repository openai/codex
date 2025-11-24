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

use std::collections::HashSet;
use std::mem;
use std::mem::MaybeUninit;
use std::path::Path;

use dupe::Dupe;
use starlark_syntax::eval_exception::EvalException;
use starlark_syntax::frame::Frame;
use starlark_syntax::internal_error;
use thiserror::Error;

use crate::any::AnyLifetime;
use crate::cast;
use crate::codemap::FileSpan;
use crate::codemap::FileSpanRef;
use crate::codemap::ResolvedFileSpan;
use crate::collections::alloca::Alloca;
use crate::collections::string_pool::StringPool;
use crate::const_frozen_string;
use crate::environment::slots::ModuleSlotId;
use crate::environment::FrozenModuleData;
use crate::environment::Module;
use crate::eval::bc::addr::BcPtrAddr;
use crate::eval::bc::bytecode::Bc;
use crate::eval::bc::frame::BcFramePtr;
use crate::eval::bc::opcode::BcOpcode;
use crate::eval::bc::writer::BcStatementLocations;
use crate::eval::compiler::def::CopySlotFromParent;
use crate::eval::compiler::def::Def;
use crate::eval::compiler::def::DefInfo;
use crate::eval::compiler::def::FrozenDef;
use crate::eval::runtime::before_stmt::BeforeStmt;
use crate::eval::runtime::before_stmt::BeforeStmtFunc;
use crate::eval::runtime::cheap_call_stack::CheapCallStack;
use crate::eval::runtime::frame_span::FrameSpan;
use crate::eval::runtime::inlined_frame::InlinedFrames;
use crate::eval::runtime::profile::bc::BcProfile;
use crate::eval::runtime::profile::data::ProfileData;
use crate::eval::runtime::profile::data::ProfileDataImpl;
use crate::eval::runtime::profile::heap::HeapProfile;
use crate::eval::runtime::profile::heap::HeapProfileFormat;
use crate::eval::runtime::profile::heap::RetainedHeapProfileMode;
use crate::eval::runtime::profile::mode::ProfileMode;
use crate::eval::runtime::profile::or_instrumentation::ProfileOrInstrumentationMode;
use crate::eval::runtime::profile::stmt::StmtProfile;
use crate::eval::runtime::profile::time_flame::TimeFlameProfile;
use crate::eval::runtime::profile::typecheck::TypecheckProfile;
use crate::eval::runtime::rust_loc::rust_loc;
use crate::eval::runtime::slots::LocalCapturedSlotId;
use crate::eval::runtime::slots::LocalSlotId;
use crate::eval::soft_error::HardErrorSoftErrorHandler;
use crate::eval::CallStack;
use crate::eval::FileLoader;
use crate::eval::SoftErrorHandler;
use crate::stdlib::breakpoint::BreakpointConsole;
use crate::stdlib::breakpoint::RealBreakpointConsole;
use crate::stdlib::extra::PrintHandler;
use crate::stdlib::extra::StderrPrintHandler;
use crate::values::function::NativeFunction;
use crate::values::layout::value_captured::value_captured_get;
use crate::values::layout::value_captured::FrozenValueCaptured;
use crate::values::layout::value_captured::ValueCaptured;
use crate::values::FrozenHeap;
use crate::values::FrozenRef;
use crate::values::Heap;
use crate::values::Trace;
use crate::values::Tracer;
use crate::values::Value;
use crate::values::ValueLike;

#[derive(Error, Debug)]
enum EvaluatorError {
    #[error("Profiling was not enabled")]
    ProfilingNotEnabled,
    #[error("Profile data already collected")]
    ProfileDataAlreadyCollected,
    #[error("Retained memory profiling can be only obtained from `FrozenModule`")]
    RetainedMemoryProfilingCannotBeObtainedFromEvaluator,
    #[error("Profile or instrumentation already enabled")]
    ProfileOrInstrumentationAlreadyEnabled,
    #[error("Top frame is not def (internal error)")]
    TopFrameNotDef,
    #[error("Coverage not enabled")]
    CoverageNotEnabled,
    #[error("Local variable `{0}` referenced before assignment")]
    LocalVariableReferencedBeforeAssignment(String),
    #[error("Max callstack size is already set")]
    CallstackSizeAlreadySet,
    #[error("Max callstack size cannot be zero")]
    ZeroCallstackSize,
}

/// Number of bytes to allocate between GC's.
pub(crate) const GC_THRESHOLD: usize = 100000;

/// Default value for max starlark stack size
pub(crate) const DEFAULT_STACK_SIZE: usize = 50;

/// Holds everything about an ongoing evaluation (local variables, globals, module resolution etc).
pub struct Evaluator<'v, 'a, 'e> {
    // The module that is being used for this evaluation
    pub(crate) module_env: &'v Module,
    /// Current function (`def` or `lambda`) frame: locals and bytecode stack.
    pub(crate) current_frame: BcFramePtr<'v>,
    // How we deal with a `load` function.
    pub(crate) loader: Option<&'a dyn FileLoader>,
    // `DefInfo` of currently executed module.
    // `DefInfo` of currently execution function can be obtained from call stack.
    pub(crate) module_def_info: FrozenRef<'static, DefInfo>,
    // Should we enable heap profiling or not
    pub(crate) heap_profile: HeapProfile,
    // Should we enable flame profiling or not
    pub(crate) time_flame_profile: TimeFlameProfile<'v>,
    // Is GC disabled for some reason
    pub(crate) disable_gc: bool,
    // If true, the interpreter prints to stderr on GC.
    // This is used for debugging.
    pub(crate) verbose_gc: bool,
    // Size of the heap when we should next perform a GC.
    pub(crate) next_gc_level: usize,
    /// Run static typechecking of the module being evaluated.
    pub(crate) static_typechecking: bool,
    // Profiling or instrumentation enabled.
    pub(crate) profile_or_instrumentation_mode: ProfileOrInstrumentationMode,
    // Used for line profiling
    stmt_profile: StmtProfile,
    // Holds things that require hooking into evaluation.
    eval_instrumentation: EvaluationInstrumentation<'a, 'e>,
    // Total time spent in runtime typechecking.
    // Filled only if runtime typechecking profiling is enabled.
    pub(crate) typecheck_profile: TypecheckProfile,
    // Used for stack-like allocation
    alloca: Alloca,
    // Another stack-like allocation
    pub(crate) string_pool: StringPool,
    /// Field that can be used for any purpose you want (can store types you define).
    /// Typically accessed via native functions you also define.
    pub extra: Option<&'a dyn AnyLifetime<'e>>,
    /// Called to perform console IO each time `breakpoint` function is called.
    pub(crate) breakpoint_handler:
        Option<Box<dyn Fn() -> anyhow::Result<Box<dyn BreakpointConsole>>>>,
    /// Use in implementation of `print` function.
    pub(crate) print_handler: &'a (dyn PrintHandler + 'a),
    /// Deprecation handler.
    pub(crate) soft_error_handler: &'a (dyn SoftErrorHandler + 'a),
    /// Max size of starlark stack
    pub(crate) max_callstack_size: Option<usize>,
    // The Starlark-level call-stack of functions.
    // Must go last because it's quite a big structure
    pub(crate) call_stack: CheapCallStack<'v>,
}

/// Just holds things that require using EvaluationCallbacksEnabled so that we can cache whether that needs to be enabled or not.
struct EvaluationInstrumentation<'a, 'e: 'a> {
    // Bytecode profile.
    bc_profile: BcProfile,
    // Extra functions to run on each statement, usually empty
    before_stmt: BeforeStmt<'a, 'e>,
    heap_or_flame_profile: bool,
    // Whether we need to instrument evaluation or not, should be set if before_stmt or bc_profile are enabled.
    enabled: bool,
}

impl<'a, 'e: 'a> EvaluationInstrumentation<'a, 'e> {
    fn new() -> EvaluationInstrumentation<'a, 'e> {
        Self {
            bc_profile: BcProfile::new(),
            before_stmt: BeforeStmt::default(),
            heap_or_flame_profile: false,
            enabled: false,
        }
    }

    fn enable_heap_or_flame_profile(&mut self) {
        self.heap_or_flame_profile = true;
    }

    fn change<F: FnOnce(&mut EvaluationInstrumentation<'a, 'e>) -> R, R>(&mut self, f: F) -> R {
        let r = f(self);
        self.enabled =
            self.bc_profile.enabled() || self.before_stmt.enabled() || self.heap_or_flame_profile;
        r
    }
}

// Implementing this forces users to be more careful about lifetimes that the Evaluator captures such that we could
// add captures of types that implement Drop without needing changes to client code.
impl Drop for Evaluator<'_, '_, '_> {
    fn drop(&mut self) {}
}

impl<'v, 'a, 'e: 'a> Evaluator<'v, 'a, 'e> {
    /// Crate a new [`Evaluator`] specifying the [`Module`] used for module variables.
    ///
    /// If your program contains `load()` statements, you also need to call
    /// [`set_loader`](Evaluator::set_loader).
    pub fn new(module: &'v Module) -> Self {
        Evaluator {
            call_stack: CheapCallStack::default(),
            module_env: module,
            current_frame: BcFramePtr::null(),
            loader: None,
            extra: None,
            next_gc_level: GC_THRESHOLD,
            disable_gc: false,
            alloca: Alloca::new(),
            profile_or_instrumentation_mode: ProfileOrInstrumentationMode::None,
            heap_profile: HeapProfile::new(),
            stmt_profile: StmtProfile::new(),
            typecheck_profile: TypecheckProfile::default(),
            time_flame_profile: TimeFlameProfile::new(),
            eval_instrumentation: EvaluationInstrumentation::new(),
            module_def_info: DefInfo::empty(), // Will be replaced before it is used
            string_pool: StringPool::default(),
            breakpoint_handler: None,
            print_handler: &StderrPrintHandler,
            soft_error_handler: &HardErrorSoftErrorHandler,
            verbose_gc: false,
            static_typechecking: false,
            max_callstack_size: None,
        }
    }

    /// Disables garbage collection from now onwards. Cannot be re-enabled.
    /// Usually called because you have captured [`Value`]'s unsafely, either in
    /// global variables or the [`extra`](Evaluator::extra) field.
    pub fn disable_gc(&mut self) {
        self.disable_gc = true;
    }

    /// Enable GC logging.
    pub fn verbose_gc(&mut self) {
        self.verbose_gc = true;
    }

    /// Enable static typechecking. For example:
    ///
    /// ```python
    /// def foo() -> int: return "hello"
    /// ```
    ///
    /// would fail when static typechecking is enabled even if `foo` is never called.
    pub fn enable_static_typechecking(&mut self, enable: bool) {
        self.static_typechecking = enable;
    }

    /// Set the [`FileLoader`] used to resolve `load()` statements.
    /// A list of all load statements can be obtained through
    /// [`AstModule::loads`](crate::syntax::AstModule::loads).
    pub fn set_loader(&mut self, loader: &'a dyn FileLoader) {
        self.loader = Some(loader);
    }

    /// Enable profiling, allowing [`Evaluator::write_profile`] to be used.
    /// Profilers add overhead, and while some profilers can be used together,
    /// it's better to run at most one profiler at a time.
    pub fn enable_profile(&mut self, mode: &ProfileMode) -> anyhow::Result<()> {
        if self.profile_or_instrumentation_mode != ProfileOrInstrumentationMode::None {
            return Err(EvaluatorError::ProfileOrInstrumentationAlreadyEnabled.into());
        }

        self.profile_or_instrumentation_mode = ProfileOrInstrumentationMode::Profile(mode.dupe());

        match mode {
            ProfileMode::HeapSummaryAllocated
            | ProfileMode::HeapFlameAllocated
            | ProfileMode::HeapSummaryRetained
            | ProfileMode::HeapFlameRetained => {
                self.heap_profile.enable();

                match mode {
                    ProfileMode::HeapFlameRetained => self
                        .module_env
                        .enable_retained_heap_profile(RetainedHeapProfileMode::Flame),
                    ProfileMode::HeapSummaryRetained => self
                        .module_env
                        .enable_retained_heap_profile(RetainedHeapProfileMode::Summary),
                    _ => {}
                }

                self.eval_instrumentation
                    .change(|v| v.enable_heap_or_flame_profile());

                // Disable GC because otherwise why lose the profile records, as we use the heap
                // to store a complete list of what happened in linear order.
                self.disable_gc = true;
            }
            ProfileMode::Statement | ProfileMode::Coverage => {
                self.stmt_profile.enable();
                self.before_stmt_fn(&|span, eval| eval.stmt_profile.before_stmt(span));
            }
            ProfileMode::TimeFlame => {
                self.time_flame_profile.enable();
                self.eval_instrumentation
                    .change(|v| v.enable_heap_or_flame_profile());
            }
            ProfileMode::Bytecode => {
                self.eval_instrumentation
                    .change(|v| v.bc_profile.enable_1());
            }
            ProfileMode::BytecodePairs => {
                self.eval_instrumentation
                    .change(|v| v.bc_profile.enable_2());
            }
            ProfileMode::Typecheck => {
                self.typecheck_profile.enabled = true;
            }
            ProfileMode::None => {}
        }
        Ok(())
    }

    /// Write a profile to a file.
    /// Only valid if corresponding profiler was enabled.
    pub fn write_profile<P: AsRef<Path>>(&mut self, filename: P) -> crate::Result<()> {
        self.gen_profile()?.write(filename.as_ref())
    }

    /// Generate profile for a given mode.
    /// Only valid if corresponding profiler was enabled.
    pub fn gen_profile(&mut self) -> crate::Result<ProfileData> {
        let mode = match &self.profile_or_instrumentation_mode {
            ProfileOrInstrumentationMode::None => {
                return Err(crate::Error::new_other(EvaluatorError::ProfilingNotEnabled));
            }
            ProfileOrInstrumentationMode::Collected => {
                return Err(crate::Error::new_other(
                    EvaluatorError::ProfileDataAlreadyCollected,
                ));
            }
            ProfileOrInstrumentationMode::Profile(mode) => mode.dupe(),
        };
        self.profile_or_instrumentation_mode = ProfileOrInstrumentationMode::Collected;
        match mode {
            ProfileMode::HeapSummaryAllocated => self
                .heap_profile
                .gen(self.heap(), HeapProfileFormat::Summary),
            ProfileMode::HeapFlameAllocated => self
                .heap_profile
                .gen(self.heap(), HeapProfileFormat::FlameGraph),
            ProfileMode::HeapSummaryRetained | ProfileMode::HeapFlameRetained => {
                Err(crate::Error::new_other(
                    EvaluatorError::RetainedMemoryProfilingCannotBeObtainedFromEvaluator,
                ))
            }
            ProfileMode::Statement => self.stmt_profile.gen(),
            ProfileMode::Coverage => self.stmt_profile.gen_coverage(),
            ProfileMode::Bytecode => self.gen_bc_profile(),
            ProfileMode::BytecodePairs => self.gen_bc_pairs_profile(),
            ProfileMode::TimeFlame => self.time_flame_profile.gen(),
            ProfileMode::Typecheck => self.typecheck_profile.gen(),
            ProfileMode::None => Ok(ProfileData {
                profile: ProfileDataImpl::None,
            }),
        }
    }

    /// Get code coverage.
    ///
    /// Works if statement profile is enabled.
    ///
    /// Note coverage is not precise, because
    /// * some optimizer transformations may create incorrect spans
    /// * some optimizer transformations may remove statements
    pub fn coverage(&self) -> crate::Result<HashSet<ResolvedFileSpan>> {
        match self.profile_or_instrumentation_mode {
            ProfileOrInstrumentationMode::Profile(ProfileMode::Coverage) => {
                self.stmt_profile.coverage()
            }
            _ => Err(crate::Error::new_other(EvaluatorError::CoverageNotEnabled)),
        }
    }

    /// Enable interactive `breakpoint()`. When enabled, `breakpoint()`
    /// reads commands from stdin and write to stdout.
    /// When disabled (default), `breakpoint()` function results in error.
    pub fn enable_terminal_breakpoint_console(&mut self) {
        self.breakpoint_handler = Some(RealBreakpointConsole::factory());
    }

    /// Obtain the current call-stack, suitable for use in diagnostics.
    pub fn call_stack(&self) -> CallStack {
        self.call_stack
            .to_diagnostic_frames(InlinedFrames::default())
    }

    /// Obtain the top frame on the call-stack. May be [`None`] if the
    /// call happened via native functions.
    pub fn call_stack_top_frame(&self) -> Option<Frame> {
        self.call_stack.top_frame()
    }

    /// Current size (in frames) of the stack.
    pub fn call_stack_count(&self) -> usize {
        self.call_stack.count()
    }

    /// Obtain the top location on the call-stack. May be [`None`] if the
    /// call happened via native functions.
    pub fn call_stack_top_location(&self) -> Option<FileSpan> {
        self.call_stack.top_location()
    }

    pub(crate) fn before_stmt_fn(
        &mut self,
        f: &'a dyn for<'v1> Fn(FileSpanRef, &mut Evaluator<'v1, 'a, 'e>),
    ) {
        self.before_stmt(f.into())
    }

    pub(crate) fn before_stmt(&mut self, f: BeforeStmtFunc<'a, 'e>) {
        self.eval_instrumentation
            .change(|v| v.before_stmt.before_stmt.push(f))
    }

    /// This function is used by DAP, and it is not public API.
    // TODO(nga): pull DAP into the crate, and hide this function.
    #[doc(hidden)]
    pub fn before_stmt_for_dap(&mut self, f: BeforeStmtFunc<'a, 'e>) {
        self.before_stmt(f)
    }

    /// Set the handler invoked when `print` function is used.
    pub fn set_print_handler(&mut self, handler: &'a (dyn PrintHandler + 'a)) {
        self.print_handler = handler;
    }

    /// Set deprecation handler. If not set, deprecations are treated as hard errors.
    pub fn set_soft_error_handler(&mut self, handler: &'a (dyn SoftErrorHandler + 'a)) {
        self.soft_error_handler = handler;
    }

    /// Called to add an entry to the call stack, by the function being invoked.
    /// Called for all types of function, including those written in Rust.
    #[inline(always)]
    pub(crate) fn with_call_stack<R>(
        &mut self,
        function: Value<'v>,
        span: Option<FrozenRef<'static, FrameSpan>>,
        within: impl FnOnce(&mut Self) -> crate::Result<R>,
    ) -> crate::Result<R> {
        #[cold]
        #[inline(never)]
        fn add_diagnostics(mut e: crate::Error, me: &Evaluator) -> crate::Error {
            // Make sure we capture the call_stack before popping things off it
            e.set_call_stack(|| me.call_stack.to_diagnostic_frames(InlinedFrames::default()));
            e
        }

        self.call_stack.push(function, span)?;
        // Must always call .pop regardless
        let res = within(self).map_err(|e| add_diagnostics(e, self));
        self.call_stack.pop();
        res
    }

    /// The active heap where [`Value`]s are allocated.
    pub fn heap(&self) -> &'v Heap {
        self.module_env.heap()
    }

    /// Module which was passed to the evaluator.
    pub fn module(&self) -> &'v Module {
        self.module_env
    }

    /// The frozen heap. It's possible to allocate [`FrozenValue`](crate::values::FrozenValue)s here,
    /// but often not a great idea, as they will remain allocated as long
    /// as the results of this execution are required.
    /// Suitable for use with [`add_reference`](FrozenHeap::add_reference)
    /// and [`OwnedFrozenValue::owned_frozen_value`](crate::values::OwnedFrozenValue::owned_frozen_value).
    pub fn frozen_heap(&self) -> &'v FrozenHeap {
        self.module_env.frozen_heap()
    }

    pub(crate) fn get_slot_module(&self, slot: ModuleSlotId) -> crate::Result<Value<'v>> {
        // Make sure the error-path doesn't get inlined into the normal-path execution
        #[cold]
        #[inline(never)]
        fn error<'v>(eval: &Evaluator<'v, '_, '_>, slot: ModuleSlotId) -> crate::Error {
            let name = match eval.top_frame_def_frozen_module(false) {
                Err(e) => Some(format!("<internal error: {e}>")),
                Ok(None) => eval
                    .module_env
                    .mutable_names()
                    .get_slot(slot)
                    .map(|s| s.as_str().to_owned()),
                Ok(Some(e)) => e.get_slot_name(slot).map(|s| s.as_str().to_owned()),
            }
            .unwrap_or_else(|| "<unknown>".to_owned());
            crate::Error::new_other(EvaluatorError::LocalVariableReferencedBeforeAssignment(
                name,
            ))
        }

        match self.top_frame_def_frozen_module(false)? {
            None => self.module_env.slots().get_slot(slot),
            Some(e) => e.get_slot(slot).map(Value::new_frozen),
        }
        .ok_or_else(|| error(self, slot))
    }

    // Make sure the error-path doesn't get inlined into the normal-path execution
    #[cold]
    #[inline(never)]
    pub(crate) fn local_var_referenced_before_assignment(&self, slot: LocalSlotId) -> crate::Error {
        let def_info = match self.top_frame_def_info() {
            Ok(def_info) => def_info,
            Err(e) => return e,
        };
        let names = &def_info.used;
        let name = names[slot.0 as usize].as_str().to_owned();
        crate::Error::new_other(EvaluatorError::LocalVariableReferencedBeforeAssignment(
            name,
        ))
    }

    #[inline(always)]
    pub(crate) fn get_slot_local(
        &self,
        frame: BcFramePtr<'v>,
        slot: LocalSlotId,
    ) -> crate::Result<Value<'v>> {
        // We access locals from explicitly passed frame because it is faster.
        debug_assert!(self.current_frame == frame);

        frame
            .get_slot(slot.to_captured_or_not())
            .ok_or_else(|| self.local_var_referenced_before_assignment(slot))
    }

    pub(crate) fn get_slot_local_captured(
        &self,
        slot: LocalCapturedSlotId,
    ) -> crate::Result<Value<'v>> {
        let value_captured = self.get_slot_local(self.current_frame, LocalSlotId(slot.0))?;
        let value_captured = value_captured_get(value_captured);
        value_captured
            .ok_or_else(|| self.local_var_referenced_before_assignment(LocalSlotId(slot.0)))
    }

    pub(crate) fn clone_slot_capture(
        &self,
        copy: &CopySlotFromParent,
        target_def_info: &DefInfo,
    ) -> Value<'v> {
        match self.current_frame.get_slot(copy.parent) {
            Some(value_captured) => {
                debug_assert!(
                    value_captured.downcast_ref::<ValueCaptured>().is_some()
                        || value_captured
                            .downcast_ref::<FrozenValueCaptured>()
                            .is_some(),
                    "slot {} ({}) is expected to be ValueCaptured, it is {:?} ({}); \
                        def location: {}",
                    copy.parent.0,
                    target_def_info
                        .used
                        .get(copy.child.0 as usize)
                        .copied()
                        .unwrap_or_default()
                        .as_str(),
                    value_captured,
                    value_captured.get_type(),
                    target_def_info.signature_span,
                );
                value_captured
            }
            None => {
                let value_captured = self.heap().alloc_complex(ValueCaptured::new(None));
                self.current_frame.set_slot(copy.parent, value_captured);
                value_captured
            }
        }
    }

    /// Set a variable in the top-level module currently being processed.
    /// This may not be the module the function is being called in.
    ///
    /// Any variables which are set will be available in the [`Module`] after evaluation returns.
    /// If those variables are _also_ existing top-level variables, then the program from that point on
    /// will incorporate those values. If they aren't existing top-level variables, they will be ignored.
    /// These details are subject to change.
    /// As such, use this API with a healthy dose of caution and in limited settings.
    pub fn set_module_variable_at_some_point(
        &mut self,
        name: &str,
        value: Value<'v>,
    ) -> crate::Result<()> {
        value.export_as(name, self)?;
        self.module_env.set(name, value);
        Ok(())
    }

    pub(crate) fn set_slot_module(&mut self, slot: ModuleSlotId, value: Value<'v>) {
        self.module_env.slots().set_slot(slot, value);
    }

    pub(crate) fn set_slot_local_captured(&mut self, slot: LocalCapturedSlotId, value: Value<'v>) {
        let slot = LocalSlotId(slot.0);
        match self.current_frame.get_slot(slot.to_captured_or_not()) {
            Some(value_captured) => {
                let value_captured = value_captured
                    .downcast_ref::<ValueCaptured>()
                    .expect("not a ValueCaptured");
                value_captured.set(value);
            }
            None => {
                let value_captured = self.heap().alloc_complex(ValueCaptured::new(Some(value)));
                self.current_frame
                    .set_slot(slot.to_captured_or_not(), value_captured);
            }
        };
    }

    /// Take a value from the local slot and store it back wrapped in [`ValueCaptured`].
    pub(crate) fn wrap_local_slot_captured(&mut self, slot: LocalSlotId) {
        let value = self
            .current_frame
            .get_slot(slot.to_captured_or_not())
            .expect("slot unset");
        debug_assert!(value.downcast_ref::<ValueCaptured>().is_none());
        let value_captured = self.heap().alloc_complex(ValueCaptured::new(Some(value)));
        self.current_frame
            .set_slot(slot.to_captured_or_not(), value_captured);
    }

    pub(crate) fn check_return_type(&mut self, ret: Value<'v>) -> crate::Result<()> {
        let func = self.call_stack.top_nth_function(0)?;
        if let Some(func) = func.downcast_ref::<Def>() {
            func.check_return_type(ret, self)
        } else if let Some(func) = func.downcast_ref::<FrozenDef>() {
            func.check_return_type(ret, self)
        } else {
            Err(crate::Error::new_other(EvaluatorError::TopFrameNotDef))
        }
    }

    fn func_to_def_info(&self, func: Value<'_>) -> crate::Result<FrozenRef<DefInfo>> {
        if let Some(func) = func.downcast_ref::<Def>() {
            Ok(func.def_info)
        } else if let Some(func) = func.downcast_ref::<FrozenDef>() {
            Ok(func.def_info)
        } else if func.is_none() {
            // For module, it is `None`.
            Ok(self.module_def_info)
        } else {
            Err(crate::Error::new_other(EvaluatorError::TopFrameNotDef))
        }
    }

    pub(crate) fn top_frame_def_info(&self) -> crate::Result<FrozenRef<DefInfo>> {
        let func = self.call_stack.top_nth_function(0)?;
        self.func_to_def_info(func)
    }

    pub(crate) fn top_frame_def_frozen_module(
        &self,
        for_debugger: bool,
    ) -> anyhow::Result<Option<FrozenRef<'static, FrozenModuleData>>> {
        let func = self.top_frame_maybe_for_debugger(for_debugger)?;
        if let Some(func) = func.downcast_ref::<FrozenDef>() {
            Ok(func.module.load_relaxed())
        } else if let Some(func) = func.downcast_ref::<Def>() {
            Ok(func.module.load_relaxed())
        } else {
            Ok(None)
        }
    }

    fn top_frame_maybe_for_debugger(&self, for_debugger: bool) -> anyhow::Result<Value<'v>> {
        let func = self.call_stack.top_nth_function(0)?;
        if for_debugger && func.downcast_ref::<NativeFunction>().is_some() {
            // If top frame is `breakpoint` or `debug_evaluate`, it will be skipped.
            self.call_stack.top_nth_function(1)
        } else {
            Ok(func)
        }
    }

    /// Gets the "top frame" for debugging. If the real top frame is `breakpoint` or `debug_evaluate`
    /// it will be skipped. This should only be used for the starlark debugger.
    pub(crate) fn top_frame_def_info_for_debugger(&self) -> crate::Result<FrozenRef<DefInfo>> {
        let func = self.top_frame_maybe_for_debugger(true)?;
        self.func_to_def_info(func)
    }

    /// Cause a GC to be triggered next time it's possible.
    pub(crate) fn trigger_gc(&mut self) {
        // We will GC next time we can, since the threshold is if 0 or more bytes are allocated
        self.next_gc_level = 0;
    }

    fn trace(&mut self, tracer: &Tracer<'v>) {
        self.module_env.trace(tracer);
        self.current_frame.trace(tracer);
        self.call_stack.trace(tracer);
        self.time_flame_profile.trace(tracer);
    }

    /// Perform a garbage collection.
    /// After this operation all [`Value`]s not reachable from the evaluator will be invalid,
    /// and using them will lead to a segfault.
    /// Do not call during Starlark evaluation.
    pub unsafe fn garbage_collect(&mut self) {
        if self.verbose_gc {
            eprintln!(
                "Starlark: allocated bytes: {}, starting GC...",
                self.heap().allocated_bytes()
            );
        }

        self.stmt_profile
            .before_stmt(rust_loc!().span.file_span_ref());

        self.time_flame_profile
            .record_call_enter(const_frozen_string!("GC").to_value());

        self.heap().garbage_collect(|tracer| self.trace(tracer));

        self.time_flame_profile.record_call_exit();

        if self.verbose_gc {
            eprintln!(
                "Starlark: GC complete. Allocated bytes: {}.",
                self.heap().allocated_bytes()
            );
        }
    }

    /// Note that the `Drop` for the `T` will not be called. That's safe if there is no `Drop`,
    /// or you call it yourself.
    #[inline(always)]
    pub(crate) fn alloca_uninit<T, R, F>(&mut self, len: usize, k: F) -> R
    where
        F: FnOnce(&mut [MaybeUninit<T>], &mut Self) -> R,
    {
        // We want to be able to access the evaluator underneath the alloca.
        // We know that the alloca will be used in a stacked way, so that's fine.
        let alloca = unsafe { cast::ptr_lifetime(&self.alloca) };
        alloca.alloca_uninit(len, |xs| k(xs, self))
    }

    /// Allocate `len` elements, initialize them with `init` function, and invoke
    /// a callback `k` with the pointer to the allocated memory and `self`.
    #[inline(always)]
    pub(crate) fn alloca_init<T, R, F>(&mut self, len: usize, init: impl Fn() -> T, k: F) -> R
    where
        F: FnOnce(&mut [T], &mut Self) -> R,
    {
        let alloca = unsafe { cast::ptr_lifetime(&self.alloca) };
        alloca.alloca_init(len, init, |xs| k(xs, self))
    }

    /// Concat two slices and invoke the callback with the result.
    pub(crate) fn alloca_concat<T: Clone, R, F>(&mut self, x: &[T], y: &[T], k: F) -> R
    where
        F: FnOnce(&[T], &mut Self) -> R,
    {
        let alloca = unsafe { cast::ptr_lifetime(&self.alloca) };
        alloca.alloca_concat(x, y, |xs| k(xs, self))
    }

    pub(crate) fn gen_bc_profile(&mut self) -> crate::Result<ProfileData> {
        self.eval_instrumentation.bc_profile.gen_bc_profile()
    }

    pub(crate) fn gen_bc_pairs_profile(&mut self) -> crate::Result<ProfileData> {
        self.eval_instrumentation.bc_profile.gen_bc_pairs_profile()
    }

    #[cold]
    #[inline(never)]
    fn eval_bc_with_callbacks(
        &mut self,
        def: Value<'v>,
        bc: &Bc,
    ) -> Result<Value<'v>, EvalException> {
        debug_assert!(self.eval_instrumentation.enabled);
        if self.eval_instrumentation.heap_or_flame_profile {
            self.heap_profile.record_call_enter(def, self.heap());
            self.time_flame_profile.record_call_enter(def);
            let res = bc.run(self, &mut EvalCallbacksDisabled);
            self.heap_profile.record_call_exit(self.heap());
            self.time_flame_profile.record_call_exit();
            res
        } else {
            bc.run(
                self,
                &mut EvalCallbacksEnabled {
                    mode: match (
                        self.eval_instrumentation.before_stmt.enabled(),
                        self.eval_instrumentation.bc_profile.enabled(),
                    ) {
                        (true, false) => EvalCallbacksMode::BeforeStmt,
                        (false, true) => EvalCallbacksMode::BcProfile,
                        (true, true) => {
                            return Err(EvalException::new_unknown_span(internal_error!(
                                "both before_stmt and bc_profile are enabled"
                            )));
                        }
                        (false, false) => {
                            return Err(EvalException::new_unknown_span(internal_error!(
                                "neither before_stmt nor bc_profile are enabled"
                            )));
                        }
                    },
                    stmt_locs: &bc.instrs.stmt_locs,
                    bc_start_ptr: bc.instrs.start_ptr(),
                },
            )
        }
    }

    #[inline(always)]
    pub(crate) fn eval_bc(&mut self, def: Value<'v>, bc: &Bc) -> Result<Value<'v>, EvalException> {
        if self.eval_instrumentation.enabled {
            self.eval_bc_with_callbacks(def, bc)
        } else {
            bc.run(self, &mut EvalCallbacksDisabled)
        }
    }

    /// Sets max call stack size.
    /// Stack allocation will happen on entry point of evaluation if not allocated yet.
    pub fn set_max_callstack_size(&mut self, stack_size: usize) -> anyhow::Result<()> {
        if stack_size == 0 {
            return Err(EvaluatorError::ZeroCallstackSize.into());
        }
        if self.max_callstack_size.is_some() {
            return Err(EvaluatorError::CallstackSizeAlreadySet.into());
        }
        self.max_callstack_size = Some(stack_size);
        Ok(())
    }
}

pub(crate) trait EvaluationCallbacks {
    fn before_instr(
        &mut self,
        _eval: &mut Evaluator,
        _ip: BcPtrAddr,
        _opcode: BcOpcode,
    ) -> crate::Result<()>;
}

pub(crate) struct EvalCallbacksDisabled;

impl EvaluationCallbacks for EvalCallbacksDisabled {
    #[inline(always)]
    fn before_instr(
        &mut self,
        _eval: &mut Evaluator,
        _ip: BcPtrAddr,
        _opcode: BcOpcode,
    ) -> crate::Result<()> {
        Ok(())
    }
}

pub(crate) enum EvalCallbacksMode {
    BcProfile,
    BeforeStmt,
}

pub(crate) struct EvalCallbacksEnabled<'a> {
    pub(crate) mode: EvalCallbacksMode,
    pub(crate) stmt_locs: &'a BcStatementLocations,
    pub(crate) bc_start_ptr: BcPtrAddr<'a>,
}

impl<'a> EvalCallbacksEnabled<'a> {
    fn before_stmt(&mut self, eval: &mut Evaluator, ip: BcPtrAddr) -> crate::Result<()> {
        let offset = ip.offset_from(self.bc_start_ptr);
        if let Some(loc) = self.stmt_locs.stmt_at(offset) {
            before_stmt(loc.span, eval)?;
        }
        Ok(())
    }
}

impl<'a> EvaluationCallbacks for EvalCallbacksEnabled<'a> {
    #[inline(always)]
    fn before_instr(
        &mut self,
        eval: &mut Evaluator,
        ip: BcPtrAddr,
        opcode: BcOpcode,
    ) -> crate::Result<()> {
        match self.mode {
            EvalCallbacksMode::BcProfile => {
                eval.eval_instrumentation.bc_profile.before_instr(opcode);
                Ok(())
            }
            EvalCallbacksMode::BeforeStmt => self.before_stmt(eval, ip),
        }
    }
}

// This function should be called before every meaningful statement.
// The purposes are GC, profiling and debugging.
//
// This function is called only if `before_stmt` is set before compilation start.
pub(crate) fn before_stmt(span: FrameSpan, eval: &mut Evaluator) -> crate::Result<()> {
    assert!(
        eval.eval_instrumentation.before_stmt.enabled(),
        "this code should only be called if `before_stmt` is set"
    );
    let mut fs = eval.eval_instrumentation.change(|eval_instrumentation| {
        mem::take(&mut eval_instrumentation.before_stmt.before_stmt)
    });
    let mut result = Ok(());
    for f in &mut fs {
        if result.is_ok() {
            result = f.call(span.span.file_span_ref(), eval);
        }
    }
    let added = eval.eval_instrumentation.change(|eval_instrumentation| {
        mem::replace(&mut eval_instrumentation.before_stmt.before_stmt, fs)
    });
    assert!(
        added.is_empty(),
        "`before_stmt` cannot be modified during evaluation"
    );
    result
}
