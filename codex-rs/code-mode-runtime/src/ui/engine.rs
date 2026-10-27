//! Persistent module state with explicit microtask pumping and no dynamic executable loading.

use std::collections::HashMap;
use std::io;

use codex_plugin_host::protocol::*;

use super::failures;
use super::transport::Transport;

pub(super) struct Engine {
    context: v8::Global<v8::Context>,
    namespace: Option<v8::Global<v8::Object>>,
    pending_promises: HashMap<u64, v8::Global<v8::Promise>>,
    pub(super) transport: Transport,
    entered: bool,
    isolate: v8::OwnedIsolate,
}

impl Engine {
    /// Construct an isolate; user code runs only in watchdog-protected turns.
    pub(super) fn new(transport: Transport) -> io::Result<Self> {
        // V8 heap exhaustion is fatal to the shared helper; the parent survives and can restart it.
        let mut isolate =
            v8::Isolate::new(v8::CreateParams::default().heap_limits(0, 64 * 1024 * 1024));
        isolate.set_microtasks_policy(v8::MicrotasksPolicy::Explicit);
        failures::install(&mut isolate);
        // V8 rejects dynamic imports when no host loader is installed.
        isolate.set_allow_wasm_code_generation_callback(reject_wasm);
        let context = {
            v8::scope!(let scope, &mut isolate);
            let context = v8::Context::new(scope, Default::default());
            context.set_allow_generation_from_strings(false);
            let scope = &mut v8::ContextScope::new(scope, context);
            let global = context.global(scope);
            // Binary buffers are omitted until the sandbox-enabled allocator supports quotas.
            for name in [
                "console",
                "ArrayBuffer",
                "SharedArrayBuffer",
                "Atomics",
                "WebAssembly",
                "DataView",
                "Int8Array",
                "Uint8Array",
                "Uint8ClampedArray",
                "Int16Array",
                "Uint16Array",
                "Int32Array",
                "Uint32Array",
                "Float16Array",
                "Float32Array",
                "Float64Array",
                "BigInt64Array",
                "BigUint64Array",
            ] {
                let key = v8::String::new(scope, name)
                    .ok_or_else(|| io::Error::other("V8 allocation failed"))?;
                if global.delete(scope, key.into()) != Some(true) {
                    return Err(io::Error::other("failed to restrict UI globals"));
                }
            }
            v8::Global::new(scope, context)
        };
        let mut runtime = Self {
            entered: true,
            transport,
            isolate,
            context,
            namespace: None,
            pending_promises: HashMap::new(),
        };
        // SAFETY: no handle scopes remain; the executor enters only the isolate being polled.
        unsafe {
            runtime.isolate.exit();
        }
        runtime.entered = false;
        Ok(runtime)
    }

    pub(super) fn handle(&self) -> v8::IsolateHandle {
        self.isolate.thread_safe_handle()
    }

    pub(super) fn turn(
        &mut self,
        message: HostMessage,
    ) -> Result<Vec<RuntimeMessage>, RuntimeMessage> {
        // SAFETY: all isolates stay on this executor; no scopes survive a turn.
        unsafe {
            self.isolate.enter();
        }
        self.entered = true;
        let result = match message {
            HostMessage::Initialize { source } => self
                .load(&source)
                .map(|()| vec![RuntimeMessage::Initialized]),
            HostMessage::Dispatch {
                id,
                export,
                return_mode,
            } => self.dispatch(id, &export, return_mode),
            HostMessage::Close => Err(ErrorCode::InvalidProgram),
        };
        let script_error = failures::take(&mut self.isolate);
        // SAFETY: load/dispatch have released all scopes before yielding the executor.
        unsafe {
            self.isolate.exit();
        }
        self.entered = false;
        result.map_err(|code| RuntimeMessage::Error { code, script_error })
    }

    pub(super) fn load(&mut self, source: &str) -> Result<(), ErrorCode> {
        if self.namespace.is_some() || source.len() > MAX_SOURCE {
            return Err(ErrorCode::InvalidProgram);
        }
        v8::scope!(let scope, &mut self.isolate);
        let context = v8::Local::new(scope, &self.context);
        let scope = &mut v8::ContextScope::new(scope, context);
        let tc = std::pin::pin!(v8::TryCatch::new(scope));
        let mut tc = tc.init();
        let result = (|| {
            let source = v8::String::new(&tc, source).ok_or(ErrorCode::QuotaExceeded)?;
            let name = v8::String::new(&tc, "plugin.mjs").ok_or(ErrorCode::QuotaExceeded)?;
            let origin = v8::ScriptOrigin::new(
                &tc,
                name.into(),
                /*resource_line_offset*/ 0,
                /*resource_column_offset*/ 0,
                /*resource_is_shared_cross_origin*/ false,
                /*script_id*/ 0,
                /*source_map_url*/ None,
                /*resource_is_opaque*/ false,
                /*is_wasm*/ false,
                /*is_module*/ true,
                /*host_defined_options*/ None,
            );
            let mut source = v8::script_compiler::Source::new(source, Some(&origin));
            let module = v8::script_compiler::compile_module(&tc, &mut source)
                .ok_or(ErrorCode::InvalidProgram)?;
            // Reject imports before V8 invokes its platform-specific resolver callback.
            if module.get_module_requests().length() != 0 {
                let text = v8::String::new(&tc, "module imports are unavailable")
                    .ok_or(ErrorCode::QuotaExceeded)?;
                let error = v8::Exception::type_error(&tc, text);
                let message = v8::Exception::create_message(&tc, error);
                failures::record(&mut tc, Some(message));
                return Err(ErrorCode::InvalidProgram);
            }
            module
                .instantiate_module(&tc, |_, _, _, _| None)
                .ok_or(ErrorCode::InvalidProgram)?;
            let evaluation = module.evaluate(&tc).ok_or(ErrorCode::ExecutionFailed)?;
            let evaluation = v8::Local::<v8::Promise>::try_from(evaluation)
                .map_err(|_| ErrorCode::InvalidProgram)?;
            failures::checkpoint(&mut tc)?;
            if evaluation.state() != v8::PromiseState::Fulfilled {
                return Err(ErrorCode::ExecutionFailed);
            }
            let namespace = v8::Local::<v8::Object>::try_from(module.get_module_namespace())
                .map_err(|_| ErrorCode::InvalidProgram)?;
            self.namespace = Some(v8::Global::new(&tc, namespace));
            Ok(())
        })();
        if result.is_err() {
            let message = tc.message();
            failures::record(&mut tc, message);
        }
        result
    }

    /// One JS turn; unresolved invocations yield back to the IPC dispatcher.
    pub(super) fn dispatch(
        &mut self,
        id: u64,
        export: &str,
        return_mode: ReturnMode,
    ) -> Result<Vec<RuntimeMessage>, ErrorCode> {
        let namespace = self.namespace.as_ref().ok_or(ErrorCode::InvalidProgram)?;
        v8::scope!(let scope, &mut self.isolate);
        let context = v8::Local::new(scope, &self.context);
        let scope = &mut v8::ContextScope::new(scope, context);
        let tc = std::pin::pin!(v8::TryCatch::new(scope));
        let mut tc = tc.init();
        let result = (|| {
            let mut replies = Vec::new();
            if export.is_empty()
                || export.len() > MAX_METHOD
                || self.pending_promises.contains_key(&id)
            {
                return Err(ErrorCode::QuotaExceeded);
            }
            let namespace = v8::Local::new(&tc, namespace);
            let key = v8::String::new(&tc, export).ok_or(ErrorCode::QuotaExceeded)?;
            let function = namespace
                .get(&tc, key.into())
                .ok_or(ErrorCode::InvalidProgram)?;
            let function = v8::Local::<v8::Function>::try_from(function)
                .map_err(|_| ErrorCode::InvalidProgram)?;
            let receiver = v8::undefined(&tc).into();
            let result = function
                .call(&tc, receiver, &[])
                .ok_or(ErrorCode::ExecutionFailed)?;
            if return_mode == ReturnMode::MaybeAsync && result.is_promise() {
                if self.pending_promises.len() >= MAX_PENDING_PROMISES {
                    return Err(ErrorCode::QuotaExceeded);
                }
                let promise = v8::Local::<v8::Promise>::try_from(result)
                    .map_err(|_| ErrorCode::InvalidResult)?;
                self.pending_promises
                    .insert(id, v8::Global::new(&tc, promise));
            } else {
                replies.push(RuntimeMessage::Reply {
                    id,
                    value: result_string(&mut tc, result)?,
                });
            }
            failures::checkpoint(&mut tc)?;
            let completed: Vec<_> = self
                .pending_promises
                .extract_if(|_, promise| {
                    v8::Local::new(&tc, &*promise).state() != v8::PromiseState::Pending
                })
                .collect();
            for (id, promise) in completed {
                let promise = v8::Local::new(&tc, promise);
                if promise.state() != v8::PromiseState::Fulfilled {
                    failures::record_rejection(&mut tc, promise);
                    return Err(ErrorCode::ExecutionFailed);
                }
                let value = promise.result(&tc);
                replies.push(RuntimeMessage::Reply {
                    id,
                    value: result_string(&mut tc, value)?,
                });
            }
            if self.pending_promises.contains_key(&id) {
                replies.push(RuntimeMessage::Pending { id });
            }
            Ok(replies)
        })();
        if result.is_err() {
            let message = tc.message();
            failures::record(&mut tc, message);
        }
        result
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // SAFETY: the executor drops only exited engines. OwnedIsolate's destructor exits it.
        if !self.entered {
            unsafe {
                self.isolate.enter();
            }
        }
    }
}

fn result_string(
    scope: &mut v8::PinScope<'_, '_>,
    result: v8::Local<v8::Value>,
) -> Result<String, ErrorCode> {
    let value = v8::Local::<v8::String>::try_from(result).map_err(|_| ErrorCode::InvalidResult)?;
    if value.length() > MAX_RESULT || value.utf8_length(scope) > MAX_RESULT {
        return Err(ErrorCode::QuotaExceeded);
    }
    Ok(value.to_rust_string_lossy(scope))
}

unsafe extern "C" fn reject_wasm(
    _context: v8::Local<v8::Context>,
    _source: v8::Local<v8::String>,
) -> bool {
    false
}
