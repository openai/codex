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

#[cfg(test)]
mod t {
    use std::collections::HashMap;
    use std::hint;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;
    use std::thread;
    use std::thread::ScopedJoinHandle;
    use std::time::Duration;
    use std::time::Instant;

    use debugserver_types::*;
    use dupe::Dupe;

    use crate::assert::test_functions;
    use crate::debug::adapter::implementation::prepare_dap_adapter;
    use crate::debug::adapter::implementation::resolve_breakpoints;
    use crate::debug::DapAdapter;
    use crate::debug::DapAdapterClient;
    use crate::debug::DapAdapterEvalHook;
    use crate::debug::StepKind;
    use crate::debug::VariablePath;
    use crate::environment::GlobalsBuilder;
    use crate::environment::Module;
    use crate::eval::Evaluator;
    use crate::eval::ReturnFileLoader;
    use crate::syntax::AstModule;
    use crate::syntax::Dialect;
    use crate::values::OwnedFrozenValue;
    use crate::wasm::is_wasm;

    #[derive(Debug)]
    struct Client {
        controller: BreakpointController,
    }

    impl Client {
        pub fn new(controller: BreakpointController) -> Self {
            Self { controller }
        }
    }

    impl DapAdapterClient for Client {
        fn event_stopped(&self) -> crate::Result<()> {
            println!("stopped!");
            self.controller.eval_stopped()
        }
    }

    #[derive(Debug, Clone, Dupe)]
    struct BreakpointController {
        /// The number of breakpoint hits or 999999 if cancelled.
        breakpoints_hit: Arc<AtomicUsize>,
    }

    impl BreakpointController {
        fn new() -> Self {
            Self {
                breakpoints_hit: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn get_client(&self) -> Box<dyn DapAdapterClient> {
            Box::new(Client::new(self.dupe()))
        }

        fn eval_stopped(&self) -> crate::Result<()> {
            loop {
                let breakpoints_hit = self.breakpoints_hit.load(Ordering::SeqCst);
                if breakpoints_hit == 999999 {
                    eprintln!("eval_stopped: cancelled");
                    return Err(anyhow::anyhow!("cancelled").into());
                }
                if self.breakpoints_hit.compare_exchange(
                    breakpoints_hit,
                    breakpoints_hit + 1,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ) == Ok(breakpoints_hit)
                {
                    return Ok(());
                }
            }
        }

        fn wait_for_eval_stopped(&self, breakpoint_count: usize, timeout: Duration) {
            let now = Instant::now();
            loop {
                let breakpoints_hit = self.breakpoints_hit.load(Ordering::SeqCst);
                assert_ne!(breakpoints_hit, 999999, "cancelled");
                assert!(breakpoints_hit <= breakpoint_count);
                if breakpoints_hit == breakpoint_count {
                    break;
                }
                if now.elapsed() > timeout {
                    panic!("didn't hit expected breakpoint");
                }
                hint::spin_loop();
            }
        }
    }

    struct BreakpointControllerDropGuard {
        controller: BreakpointController,
    }

    impl Drop for BreakpointControllerDropGuard {
        fn drop(&mut self) {
            eprintln!("dropping controller");
            self.controller
                .breakpoints_hit
                .store(999999, Ordering::SeqCst);
        }
    }

    fn breakpoint(line: i64, condition: Option<&str>) -> SourceBreakpoint {
        SourceBreakpoint {
            column: None,
            condition: condition.map(|v| v.to_owned()),
            hit_condition: None,
            line,
            log_message: None,
        }
    }

    fn breakpoints_args(path: &str, lines: &[(i64, Option<&str>)]) -> SetBreakpointsArguments {
        SetBreakpointsArguments {
            breakpoints: Some(
                lines
                    .iter()
                    .map(|(line, condition)| breakpoint(*line, condition.as_deref()))
                    .collect(),
            ),
            lines: None,
            source: Source {
                adapter_data: None,
                checksums: None,
                name: None,
                origin: None,
                path: Some(path.to_owned()),
                presentation_hint: None,
                source_reference: None,
                sources: None,
            },
            source_modified: None,
        }
    }

    fn eval_with_hook(
        ast: AstModule,
        hook: Box<dyn DapAdapterEvalHook>,
    ) -> crate::Result<OwnedFrozenValue> {
        let modules = HashMap::new();
        let loader = ReturnFileLoader { modules: &modules };
        let globals = GlobalsBuilder::extended().with(test_functions).build();
        let env = Module::new();
        let res = {
            let mut eval = Evaluator::new(&env);
            hook.add_dap_hooks(&mut eval);
            eval.set_loader(&loader);
            eval.eval_module(ast, &globals)?
        };

        env.set("_", res);
        Ok(env
            .freeze()
            .expect("error freezing module")
            .get("_")
            .unwrap())
    }

    fn join_timeout<T>(waiting: ScopedJoinHandle<T>, timeout: Duration) -> T {
        let start = Instant::now();
        while !waiting.is_finished() {
            if start.elapsed() > timeout {
                panic!();
            }
        }
        waiting.join().unwrap()
    }

    static TIMEOUT: Duration = Duration::from_secs(10);

    fn dap_test_template<'env, F, R>(f: F) -> crate::Result<R>
    where
        F: for<'scope> FnOnce(
            &'scope thread::Scope<'scope, 'env>,
            BreakpointController,
            Box<dyn DapAdapter>,
            Box<dyn DapAdapterEvalHook>,
        ) -> crate::Result<R>,
    {
        let controller = BreakpointController::new();

        let _guard = BreakpointControllerDropGuard {
            controller: controller.dupe(),
        };

        let (adapter, eval_hook) = prepare_dap_adapter(controller.get_client());
        thread::scope(|s| f(s, controller, Box::new(adapter), Box::new(eval_hook)))
    }

    #[test]
    fn test_breakpoint() -> crate::Result<()> {
        if is_wasm() {
            // `thread::scope` doesn't work in wasm.
            return Ok(());
        }

        let file_contents = "
x = [1, 2, 3]
print(x)
        ";
        dap_test_template(|s, controller, adapter, eval_hook| {
            let ast = AstModule::parse(
                "test.bzl",
                file_contents.to_owned(),
                &Dialect::AllOptionsInternal,
            )?;
            let breakpoints =
                resolve_breakpoints(&breakpoints_args("test.bzl", &[(3, None)]), &ast)?;
            adapter.set_breakpoints("test.bzl", &breakpoints)?;
            let eval_result =
                s.spawn(move || -> crate::Result<_> { eval_with_hook(ast, eval_hook) });
            controller.wait_for_eval_stopped(1, TIMEOUT);
            // TODO(cjhopman): we currently hit breakpoints on top-level statements twice (once for the gc bytecode, once for the actual statement).
            adapter.continue_()?;
            controller.wait_for_eval_stopped(2, TIMEOUT);

            adapter.continue_()?;

            join_timeout(eval_result, TIMEOUT)?;
            Ok(())
        })
    }

    #[test]
    fn test_breakpoint_with_failing_condition() -> crate::Result<()> {
        if is_wasm() {
            return Ok(());
        }

        let file_contents = "
x = [1, 2, 3]
print(x)
        ";
        dap_test_template(|s, _, adapter, eval_hook| {
            let ast = AstModule::parse(
                "test.bzl",
                file_contents.to_owned(),
                &Dialect::AllOptionsInternal,
            )?;
            let breakpoints =
                resolve_breakpoints(&breakpoints_args("test.bzl", &[(3, Some("5 in x"))]), &ast)?;
            adapter.set_breakpoints("test.bzl", &breakpoints)?;
            let eval_result =
                s.spawn(move || -> crate::Result<_> { eval_with_hook(ast, eval_hook) });
            join_timeout(eval_result, TIMEOUT)?;
            Ok(())
        })
    }

    #[test]
    fn test_breakpoint_with_passing_condition() -> crate::Result<()> {
        if is_wasm() {
            return Ok(());
        }

        let file_contents = "
x = [1, 2, 3]
print(x)
        ";
        dap_test_template(|s, controller, adapter, eval_hook| {
            let ast = AstModule::parse(
                "test.bzl",
                file_contents.to_owned(),
                &Dialect::AllOptionsInternal,
            )?;
            let breakpoints =
                resolve_breakpoints(&breakpoints_args("test.bzl", &[(3, Some("2 in x"))]), &ast)?;
            adapter.set_breakpoints("test.bzl", &breakpoints)?;
            let eval_result =
                s.spawn(move || -> crate::Result<_> { eval_with_hook(ast, eval_hook) });
            controller.wait_for_eval_stopped(1, TIMEOUT);
            adapter.continue_()?;
            // TODO(cjhopman): we currently hit breakpoints on top-level statements twice (once for the gc bytecode, once for the actual statement).
            controller.wait_for_eval_stopped(2, TIMEOUT);
            adapter.continue_()?;

            join_timeout(eval_result, TIMEOUT)?;
            Ok(())
        })
    }

    #[test]
    fn test_step_over() -> crate::Result<()> {
        if is_wasm() {
            return Ok(());
        }

        let file_contents = "
def adjust(y):
    y[0] += 1
    y[1] += 1 # line 4
    y[2] += 1
x = [1, 2, 3]
adjust(x) # line 7
adjust(x)
print(x)
        ";
        dap_test_template(|s, controller, adapter, eval_hook| {
            let ast = AstModule::parse(
                "test.bzl",
                file_contents.to_owned(),
                &Dialect::AllOptionsInternal,
            )?;
            let breakpoints =
                resolve_breakpoints(&breakpoints_args("test.bzl", &[(7, None)]), &ast)?;
            adapter.set_breakpoints("test.bzl", &breakpoints)?;
            let eval_result =
                s.spawn(move || -> crate::Result<_> { eval_with_hook(ast, eval_hook) });
            controller.wait_for_eval_stopped(1, TIMEOUT);
            // TODO(cjhopman): we currently hit breakpoints on top-level statements twice (once for the gc bytecode, once for the actual statement).
            adapter.continue_()?;
            controller.wait_for_eval_stopped(2, TIMEOUT);

            assert_eq!("1", adapter.evaluate("x[0]")?.result);
            assert_eq!("2", adapter.evaluate("x[1]")?.result);
            assert_eq!("3", adapter.evaluate("x[2]")?.result);
            adapter.step(StepKind::Over)?;
            controller.wait_for_eval_stopped(3, TIMEOUT);
            assert_eq!("2", adapter.evaluate("x[0]")?.result);
            assert_eq!("3", adapter.evaluate("x[1]")?.result);
            assert_eq!("4", adapter.evaluate("x[2]")?.result);

            // TODO(cjhopman): we currently hit breakpoints on top-level statements twice (once for the gc bytecode, once for the actual statement).
            adapter.step(StepKind::Over)?;
            controller.wait_for_eval_stopped(4, TIMEOUT);
            adapter.step(StepKind::Over)?;
            controller.wait_for_eval_stopped(5, TIMEOUT);
            assert_eq!("3", adapter.evaluate("x[0]")?.result);
            assert_eq!("4", adapter.evaluate("x[1]")?.result);
            assert_eq!("5", adapter.evaluate("x[2]")?.result);
            adapter.continue_()?;
            join_timeout(eval_result, TIMEOUT)?;
            Ok(())
        })
    }

    #[test]
    fn test_step_into() -> crate::Result<()> {
        if is_wasm() {
            return Ok(());
        }

        let file_contents = "
def adjust(y):
    y[0] += 1
    y[1] += 1 # line 4
    y[2] += 1
x = [1, 2, 3]
adjust(x) # line 7
adjust(x)
print(x)
        ";
        dap_test_template(|s, controller, adapter, eval_hook| {
            let ast = AstModule::parse(
                "test.bzl",
                file_contents.to_owned(),
                &Dialect::AllOptionsInternal,
            )?;
            let breakpoints =
                resolve_breakpoints(&breakpoints_args("test.bzl", &[(7, None)]), &ast)?;
            adapter.set_breakpoints("test.bzl", &breakpoints)?;
            let eval_result =
                s.spawn(move || -> crate::Result<_> { eval_with_hook(ast, eval_hook) });
            controller.wait_for_eval_stopped(1, TIMEOUT);
            // TODO(cjhopman): we currently hit breakpoints on top-level statements twice (once for the gc bytecode, once for the actual statement).
            adapter.continue_()?;
            controller.wait_for_eval_stopped(2, TIMEOUT);

            assert_eq!("1", adapter.evaluate("x[0]")?.result);
            assert_eq!("2", adapter.evaluate("x[1]")?.result);
            assert_eq!("3", adapter.evaluate("x[2]")?.result);

            // into adjust
            adapter.step(StepKind::Into)?;
            controller.wait_for_eval_stopped(3, TIMEOUT);
            assert_eq!("1", adapter.evaluate("y[0]")?.result);
            assert_eq!("2", adapter.evaluate("y[1]")?.result);
            assert_eq!("3", adapter.evaluate("y[2]")?.result);

            // into should go to next line
            adapter.step(StepKind::Into)?;
            controller.wait_for_eval_stopped(4, TIMEOUT);
            assert_eq!("2", adapter.evaluate("y[0]")?.result);
            assert_eq!("2", adapter.evaluate("y[1]")?.result);
            assert_eq!("3", adapter.evaluate("y[2]")?.result);

            // two more intos should get us out of the function call
            adapter.step(StepKind::Into)?;
            controller.wait_for_eval_stopped(5, TIMEOUT);
            adapter.step(StepKind::Into)?;
            controller.wait_for_eval_stopped(6, TIMEOUT);
            assert_eq!("2", adapter.evaluate("x[0]")?.result);
            assert_eq!("3", adapter.evaluate("x[1]")?.result);
            assert_eq!("4", adapter.evaluate("x[2]")?.result);

            // and once more back into the function
            adapter.step(StepKind::Into)?;
            controller.wait_for_eval_stopped(7, TIMEOUT);

            // TODO(cjhopman): unfortunately, gc being marked as statements causes us to need to step_into again.
            adapter.step(StepKind::Into)?;
            controller.wait_for_eval_stopped(8, TIMEOUT);

            assert_eq!("2", adapter.evaluate("y[0]")?.result);
            assert_eq!("3", adapter.evaluate("y[1]")?.result);
            assert_eq!("4", adapter.evaluate("y[2]")?.result);

            adapter.continue_()?;
            join_timeout(eval_result, TIMEOUT)?;
            Ok(())
        })
    }

    #[test]
    fn test_step_out() -> crate::Result<()> {
        if is_wasm() {
            return Ok(());
        }

        let file_contents = "
def adjust(y):
    y[0] += 1
    y[1] += 1 # line 4
    y[2] += 1
x = [1, 2, 3]
adjust(x) # line 7
adjust(x)
print(x)
        ";
        dap_test_template(|s, controller, adapter, eval_hook| {
            let ast = AstModule::parse(
                "test.bzl",
                file_contents.to_owned(),
                &Dialect::AllOptionsInternal,
            )?;
            let breakpoints =
                resolve_breakpoints(&breakpoints_args("test.bzl", &[(4, None)]), &ast)?;
            adapter.set_breakpoints("test.bzl", &breakpoints)?;
            let eval_result =
                s.spawn(move || -> crate::Result<_> { eval_with_hook(ast, eval_hook) });
            // should break on the first time hitting line 4
            controller.wait_for_eval_stopped(1, TIMEOUT);
            assert_eq!("2", adapter.evaluate("y[0]")?.result);
            assert_eq!("2", adapter.evaluate("y[1]")?.result);
            assert_eq!("3", adapter.evaluate("y[2]")?.result);

            // step out should take us to line 8
            adapter.step(StepKind::Out)?;
            controller.wait_for_eval_stopped(2, TIMEOUT);
            assert_eq!("2", adapter.evaluate("x[0]")?.result);
            assert_eq!("3", adapter.evaluate("x[1]")?.result);
            assert_eq!("4", adapter.evaluate("x[2]")?.result);

            // step out should actually hit the breakpoint at 4 first (before getting out)
            adapter.step(StepKind::Out)?;
            controller.wait_for_eval_stopped(3, TIMEOUT);
            assert_eq!("3", adapter.evaluate("y[0]")?.result);
            assert_eq!("3", adapter.evaluate("y[1]")?.result);
            assert_eq!("4", adapter.evaluate("y[2]")?.result);

            // step out should get out to the print
            adapter.step(StepKind::Out)?;
            controller.wait_for_eval_stopped(4, TIMEOUT);
            assert_eq!("3", adapter.evaluate("x[0]")?.result);
            assert_eq!("4", adapter.evaluate("x[1]")?.result);
            assert_eq!("5", adapter.evaluate("x[2]")?.result);

            // one more out should be equivalent to continue
            adapter.step(StepKind::Out)?;
            join_timeout(eval_result, TIMEOUT)?;
            Ok(())
        })
    }

    #[test]
    fn test_local_variables() -> crate::Result<()> {
        if is_wasm() {
            return Ok(());
        }

        let file_contents = "
def do():
    a = struct(
        f1 = \"1\",
        f2 = 123,
    )
    arr = [1, 2, 3, 4, 6, \"234\", 123.32]
    t = (1, 2)
    d = dict(a = 1, b = \"2\")
    empty_dict = {}
    empty_list = []
    empty_tuple = ()
    return d # line 13
print(do())
        ";
        let result = dap_test_template(|s, controller, adapter, eval_hook| {
            let ast = AstModule::parse(
                "test.bzl",
                file_contents.to_owned(),
                &Dialect::AllOptionsInternal,
            )?;
            let breakpoints =
                resolve_breakpoints(&breakpoints_args("test.bzl", &[(13, None)]), &ast)?;
            adapter.set_breakpoints("test.bzl", &breakpoints)?;
            let eval_result =
                s.spawn(move || -> crate::Result<_> { eval_with_hook(ast, eval_hook) });
            controller.wait_for_eval_stopped(1, TIMEOUT);
            let result = adapter.variables();
            adapter.continue_()?;
            join_timeout(eval_result, TIMEOUT)?;
            result.map_err(crate::Error::from)
        })?;
        // It's easier to handle errors outside of thread::scope block as the test is quite flaky
        // and hangs in case error propagates
        assert_eq!(
            vec![
                ("a".to_owned(), String::from("<type:struct, size=2>"), true),
                ("arr".to_owned(), String::from("<list, size=7>"), true),
                ("t".to_owned(), String::from("<tuple, size=2>"), true),
                ("d".to_owned(), String::from("<dict, size=2>"), true),
                ("empty_dict".to_owned(), String::from("{}"), false),
                ("empty_list".to_owned(), String::from("[]"), false),
                ("empty_tuple".to_owned(), String::from("()"), false),
            ],
            result
                .locals
                .into_iter()
                .map(|v| (v.name.to_string(), v.value, v.has_children))
                .collect::<Vec<_>>()
        );

        Ok(())
    }

    #[test]
    fn test_inspect_variables() -> crate::Result<()> {
        if is_wasm() {
            return Ok(());
        }

        let file_contents = "
def do():
    a = struct(
        f1 = \"1\",
        f2 = 123,
    )
    arr = [1, 2, 3, 4, 6, \"234\", 123.32]
    t = (1, 2)
    d = dict(a = 1, b = \"2\")
    empty_dict = {}
    empty_list = []
    empty_tuple = ()
    return d # line 13
print(do())
        ";
        let result = dap_test_template(|s, controller, adapter, eval_hook| {
            let mut result = Vec::new();
            let ast = AstModule::parse(
                "test.bzl",
                file_contents.to_owned(),
                &Dialect::AllOptionsInternal,
            )?;
            let breakpoints =
                resolve_breakpoints(&breakpoints_args("test.bzl", &[(13, None)]), &ast)?;
            adapter.set_breakpoints("test.bzl", &breakpoints)?;
            let eval_result =
                s.spawn(move || -> crate::Result<_> { eval_with_hook(ast, eval_hook) });
            controller.wait_for_eval_stopped(1, TIMEOUT);
            result.extend([
                adapter.inspect_variable(VariablePath::new_local("a")),
                adapter.inspect_variable(VariablePath::new_local("arr")),
                adapter.inspect_variable(VariablePath::new_local("t")),
                adapter.inspect_variable(VariablePath::new_local("d")),
            ]);
            adapter.continue_()?;
            join_timeout(eval_result, TIMEOUT)?;
            crate::Result::Ok(result)
        })?
        .into_iter()
        .collect::<anyhow::Result<Vec<_>>>()?;

        // It's easier to handle errors outside of thread::scope block as the test is quite flaky
        // and hangs in case error propagates

        assert_variable("f1", "1", false, &result[0].sub_values[0]);
        assert_variable("f2", "123", false, &result[0].sub_values[1]);
        assert_variable("0", "1", false, &result[1].sub_values[0]);
        assert_variable("5", "234", false, &result[1].sub_values[5]);
        assert_variable("0", "1", false, &result[2].sub_values[0]);
        assert_variable("1", "2", false, &result[2].sub_values[1]);
        assert_variable("\"a\"", "1", false, &result[3].sub_values[0]);
        assert_variable("\"b\"", "2", false, &result[3].sub_values[1]);
        Ok(())
    }

    #[test]
    fn test_evaluate_expression() -> crate::Result<()> {
        if is_wasm() {
            return Ok(());
        }

        let file_contents = "
def do():
    s = struct(
        inner = struct(
            inner = struct(
                value = \"more_inner\"
            ),
            value = \"inner\",
            arr = [dict(a = 1, b = \"2\"), 1337]
        )
    )
    return s # line 12
print(do())
        ";
        let result = dap_test_template(|s, controller, adapter, eval_hook| {
            let mut result = Vec::new();
            let ast = AstModule::parse(
                "test.bzl",
                file_contents.to_owned(),
                &Dialect::AllOptionsInternal,
            )?;
            let breakpoints =
                resolve_breakpoints(&breakpoints_args("test.bzl", &[(12, None)]), &ast)?;
            adapter.set_breakpoints("test.bzl", &breakpoints)?;
            let eval_result =
                s.spawn(move || -> crate::Result<_> { eval_with_hook(ast, eval_hook) });
            controller.wait_for_eval_stopped(1, TIMEOUT);
            result.extend([
                adapter.evaluate("s.inner.value"),
                adapter.evaluate("s.inner.inner.value"),
                adapter.evaluate("s.inner.arr[0]"),
                adapter.evaluate("s.inner.arr[0][\"a\"]"),
                adapter.evaluate("s.inner.arr[1]"),
            ]);
            adapter.continue_()?;
            join_timeout(eval_result, TIMEOUT)?;
            crate::Result::Ok(result)
        })?
        .into_iter()
        .collect::<anyhow::Result<Vec<_>>>()?;

        // It's easier to handle errors outside of thread::scope block as the test is quite flaky
        // and hangs in case error propagates
        assert_eq!(
            vec![
                ("inner", false),
                ("more_inner", false),
                ("<dict, size=2>", true),
                ("1", false),
                ("1337", false),
            ],
            result
                .iter()
                .map(|v| (v.result.as_str(), v.has_children))
                .collect::<Vec<_>>()
        );

        Ok(())
    }

    fn assert_variable(
        name: &str,
        value: &str,
        has_children: bool,
        var: &crate::debug::adapter::Variable,
    ) {
        assert_eq!(
            (name.to_owned(), value, has_children),
            (var.name.to_string(), var.value.as_str(), var.has_children)
        );
    }

    #[test]
    pub fn test_truncate_string() {
        assert_eq!(
            "Hello",
            crate::debug::adapter::Variable::truncate_string("Hello".to_owned(), 10)
        );
        assert_eq!(
            "Hello",
            crate::debug::adapter::Variable::truncate_string("Hello".to_owned(), 5)
        );
        assert_eq!(
            "Hello, ...(truncated)",
            // A string that should be truncated at a character boundary
            crate::debug::adapter::Variable::truncate_string("Hello, 世界".to_owned(), 7)
        );
        assert_eq!(
            "Hello, ...(truncated)",
            // A string that would be truncated within a multi-byte character
            crate::debug::adapter::Variable::truncate_string("Hello, 世界".to_owned(), 8)
        );
        assert_eq!(
            "Hello, ...(truncated)",
            // A string that should be truncated just before a multi-byte character
            crate::debug::adapter::Variable::truncate_string("Hello, 世界".to_owned(), 9)
        );
        assert_eq!(
            "Hello, 世...(truncated)",
            crate::debug::adapter::Variable::truncate_string("Hello, 世界".to_owned(), 10)
        );
    }
}
