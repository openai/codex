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

use std::sync::Mutex;

use itertools::Itertools;
use once_cell::sync::Lazy;
use starlark_derive::starlark_module;
use thiserror::Error;

use crate as starlark;
use crate::environment::GlobalsBuilder;
use crate::eval::Evaluator;
use crate::read_line::ReadLine;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::values::none::NoneType;

// A breakpoint takes over the console UI, so having two going at once confuses everything.
// Have a global mutex to ensure one at a time.
static BREAKPOINT_MUTEX: Lazy<Mutex<State>> = Lazy::new(|| Mutex::new(State::Allow));

/// `breakpoint` function uses this trait to perform console IO.
pub(crate) trait BreakpointConsole {
    /// Return `None` on EOF.
    fn read_line(&mut self) -> anyhow::Result<Option<String>>;
    fn println(&mut self, line: &str);
}

/// Breakpoint handler implemented with `rustyline`.
pub(crate) struct RealBreakpointConsole {
    read_line: ReadLine,
}

impl BreakpointConsole for RealBreakpointConsole {
    fn read_line(&mut self) -> anyhow::Result<Option<String>> {
        self.read_line.read_line("$> ")
    }

    fn println(&mut self, line: &str) {
        eprintln!("{}", line);
    }
}

impl RealBreakpointConsole {
    pub(crate) fn factory() -> Box<dyn Fn() -> anyhow::Result<Box<dyn BreakpointConsole>>> {
        Box::new(|| {
            Ok(Box::new(RealBreakpointConsole {
                read_line: ReadLine::new("STARLARK_RUST_DEBUGGER_HISTFILE")?,
            }))
        })
    }
}

/// Is debugging allowed or not? After the user hits Ctrl-C they probably
/// just want to stop hard, so don't keep dropping them into breakpoints.
#[derive(PartialEq, Eq)]
enum State {
    Allow, // More breakpoints are fine
    Stop,  // No more breakpoints
}

/// We've run a breakpoint command, what should we do.
enum Next {
    Again,  // Accept another breakpoint command
    Resume, // Continue running
    Fail,   // Stop running
}

fn cmd_help(_eval: &mut Evaluator, rl: &mut dyn BreakpointConsole) -> anyhow::Result<Next> {
    for (name, msg, _) in COMMANDS {
        rl.println(&format!("* :{}, {}", name[0], msg))
    }
    Ok(Next::Again)
}

fn cmd_variables(eval: &mut Evaluator, rl: &mut dyn BreakpointConsole) -> anyhow::Result<Next> {
    fn truncate(mut s: String, n: usize) -> String {
        if s.len() > n {
            s.truncate(n);
            s.push_str("...");
        }
        s
    }

    for (name, value) in eval.local_variables() {
        rl.println(&format!("* {} = {}", name, truncate(value.to_string(), 80)))
    }
    Ok(Next::Again)
}

fn cmd_stack(eval: &mut Evaluator, rl: &mut dyn BreakpointConsole) -> anyhow::Result<Next> {
    for line in eval.call_stack().to_string().lines() {
        rl.println(line)
    }
    Ok(Next::Again)
}

fn cmd_resume(_eval: &mut Evaluator, _rl: &mut dyn BreakpointConsole) -> anyhow::Result<Next> {
    Ok(Next::Resume)
}

fn cmd_fail(_eval: &mut Evaluator, _rl: &mut dyn BreakpointConsole) -> anyhow::Result<Next> {
    Ok(Next::Fail)
}

const COMMANDS: &[(
    &[&str], // Possible names
    &str,    // Help text
    fn(eval: &mut Evaluator, &mut dyn BreakpointConsole) -> anyhow::Result<Next>,
)] = &[
    (&["help", "?"], "Show this help message", cmd_help),
    (&["vars"], "Show all local variables", cmd_variables),
    (&["stack"], "Show the stack trace", cmd_stack),
    (&["resume", "quit", "exit"], "Resume execution", cmd_resume),
    (&["fail"], "Abort with a failure message", cmd_fail),
];

fn pick_command(
    x: &str,
    rl: &mut dyn BreakpointConsole,
) -> Option<fn(eval: &mut Evaluator, &mut dyn BreakpointConsole) -> anyhow::Result<Next>> {
    // If we can find a command that matches perfectly, do that
    // Otherwise return the longest match, but if they are multiple, show a warning
    let mut poss = Vec::new();
    for (names, _, cmd) in COMMANDS {
        for n in *names {
            if *n == x {
                return Some(*cmd);
            }
            if n.starts_with(x) {
                poss.push((n, cmd));
                break;
            }
        }
    }
    match poss.as_slice() {
        [] => rl.println("Unrecognised command, type :help for all commands"),
        [x] => return Some(*x.1),
        xs => rl.println(&format!(
            "Ambiguous command, could have been any of: {}",
            xs.iter().map(|x| x.0).join(" ")
        )),
    }
    None
}

fn breakpoint_loop(
    eval: &mut Evaluator,
    mut rl: Box<dyn BreakpointConsole>,
) -> anyhow::Result<State> {
    loop {
        let readline = rl.read_line()?;
        match readline {
            Some(line) => {
                if let Some(line) = line.strip_prefix(':') {
                    if let Some(cmd) = pick_command(line.trim_end(), &mut *rl) {
                        match cmd(eval, &mut *rl)? {
                            Next::Again => {}
                            Next::Resume => return Ok(State::Allow),
                            Next::Fail => {
                                return Err(anyhow::anyhow!("Selected :fail at breakpoint()"));
                            }
                        }
                    }
                } else {
                    let ast = AstModule::parse("interactive", line, &Dialect::AllOptionsInternal);
                    let res = ast.and_then(|ast| eval.eval_statements(ast));
                    match res {
                        Err(e) => {
                            rl.println(&format!("{:#}", e));
                        }
                        Ok(v) => {
                            if !v.is_none() {
                                rl.println(&v.to_string())
                            }
                        }
                    }
                }
            }
            None => return Ok(State::Stop),
        }
    }
}

#[derive(Error, Debug)]
enum BreakpointError {
    #[error("Breakpoint handler is not enabled for current Evaluator")]
    NoHandler,
}

const BREAKPOINT_HIT_MESSAGE: &str = "BREAKPOINT HIT! :resume to continue, :help for all options";

#[starlark_module]
pub fn global(builder: &mut GlobalsBuilder) {
    /// When a debugger is available, breaks into the debugger.
    fn breakpoint(eval: &mut Evaluator) -> anyhow::Result<NoneType> {
        {
            let mut guard = BREAKPOINT_MUTEX.lock().unwrap();
            if *guard == State::Allow {
                let mut rl = match &mut eval.breakpoint_handler {
                    Some(rl) => rl()?,
                    None => return Err(BreakpointError::NoHandler.into()),
                };
                rl.println(BREAKPOINT_HIT_MESSAGE);
                *guard = breakpoint_loop(eval, rl)?;
            }
        }
        Ok(NoneType)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::env;
    use std::rc::Rc;

    use dupe::Dupe;

    use super::*;
    use crate::assert::Assert;

    // Breakpoint tests should not be executed concurrently
    // to avoid interfering with `BREAKPOINT_MUTEX`.
    static TEST_MUTEX: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    fn reset_global_state() {
        // `breakpoint()` function modifies the global state.
        *BREAKPOINT_MUTEX.lock().unwrap() = State::Allow;
    }

    #[test]
    // Test with: BREAKPOINT=1 cargo test -p starlark breakpoint -- --nocapture
    fn test_breakpoint_real() {
        let _g = TEST_MUTEX.lock();
        reset_global_state();

        if env::var("BREAKPOINT") != Ok("1".to_owned()) {
            return;
        }

        let mut a = Assert::new();
        a.setup_eval(|e| e.enable_terminal_breakpoint_console());
        a.globals_add(global);
        a.pass("x = [1,2,3]; breakpoint(); print(x)");
    }

    #[test]
    fn test_breakpoint_mock() {
        let _g = TEST_MUTEX.lock();
        reset_global_state();

        let printed_lines = Rc::new(RefCell::new(Vec::new()));
        let printed_lines_copy = printed_lines.dupe();

        let mut a = Assert::new();
        a.globals_add(global);
        a.setup_eval(move |eval| {
            let printed_lines = printed_lines.dupe();
            eval.breakpoint_handler = Some(Box::new(move || {
                // `Assert` runs tests several times, take only lines from the last iteration.
                printed_lines.borrow_mut().clear();

                struct Handler {
                    printed_lines: Rc<RefCell<Vec<String>>>,
                    called: bool,
                }

                impl BreakpointConsole for Handler {
                    fn read_line(&mut self) -> anyhow::Result<Option<String>> {
                        let called = self.called;
                        self.called = true;
                        if !called {
                            Ok(Some("x".to_owned()))
                        } else {
                            Ok(None)
                        }
                    }

                    fn println(&mut self, line: &str) {
                        self.printed_lines.borrow_mut().push(line.to_owned());
                    }
                }

                Ok(Box::new(Handler {
                    printed_lines: printed_lines.dupe(),
                    called: false,
                }))
            }));
        });
        a.pass("x = [1,2,3]; breakpoint()");

        assert_eq!(
            vec![BREAKPOINT_HIT_MESSAGE, "[1, 2, 3]"],
            *printed_lines_copy.borrow()
        );
    }

    #[test]
    fn test_breakpoint_disabled() {
        let _g = TEST_MUTEX.lock();
        reset_global_state();

        let mut a = Assert::new();
        a.globals_add(global);
        a.fail(
            "x = [1,2,3]; breakpoint()",
            "Breakpoint handler is not enabled",
        );
    }
}
