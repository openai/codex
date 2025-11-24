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

//! Linter.

use std::collections::HashSet;

pub use lint_message::LintMessage;
pub use types::EvalMessage;
pub use types::EvalSeverity;
pub use types::Lint;
pub use unused_loads::remove::remove_unused_loads;

use crate::analysis::types::LintT;
use crate::syntax::AstModule;

mod dubious;
pub mod find_call_name;
mod flow;
mod incompatible;
mod lint_message;
mod names;
mod performance;
mod types;
mod underscore;
mod unused_loads;

/// Run the linter.
pub trait AstModuleLint {
    /// Run a static linter over the module. If the complete set of global variables are known
    /// they can be passed as the `globals` argument, resulting in name-resolution lint errors.
    /// The precise checks run by the linter are not considered stable between versions.
    fn lint(&self, globals: Option<&HashSet<String>>) -> Vec<Lint>;
}

impl AstModuleLint for AstModule {
    fn lint(&self, globals: Option<&HashSet<String>>) -> Vec<Lint> {
        let mut res = Vec::new();
        res.extend(flow::lint(self).into_iter().map(LintT::erase));
        res.extend(incompatible::lint(self).into_iter().map(LintT::erase));
        res.extend(dubious::lint(self).into_iter().map(LintT::erase));
        res.extend(names::lint(self, globals).into_iter().map(LintT::erase));
        res.extend(underscore::lint(self).into_iter().map(LintT::erase));
        res.extend(performance::lint(self).into_iter().map(LintT::erase));
        res.retain(|issue| !self.is_suppressed(&issue.short_name, issue.location.span));
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codemap::Pos;
    use crate::syntax::Dialect;

    fn module(x: &str) -> AstModule {
        AstModule::parse("X", x.to_owned(), &Dialect::AllOptionsInternal).unwrap()
    }

    #[test]
    fn test_lint_suppressions_keyword_matching() {
        let m = module(
            r#"
def good1() -> str: #starlark-lint-disable missing-return
    pass
def bad1() -> str: # invalid suppression starlark-lint-disable missing-return
    pass
def bad2() -> str: #starlark-lint-disable-also-invalid missing-return
    pass
def good2() -> str:
    pass       # starlark-lint-disable  ,,missing-return, misplaced-load , missing-return ,,
def bad3() -> str:
    pass       # # starlark-lint-disable missing-return # invalid prefix
"#,
        );
        let res = m.lint(None);
        assert_eq!(res.len(), 3);
        assert!(res[0].problem.contains("bad1"));
        assert!(res[1].problem.contains("bad2"));
        assert!(res[2].problem.contains("bad3"));
    }

    #[test]
    fn test_lint_suppressions_fn_with_many_issues() {
        let m = module(
            r#"
def bad1(items):
    a = all(items)
    b = all({"a": a for a in []})
    c = any(list({}))

# suppressing issues fn-wide doesnt work
# starlark-lint-disable unused-assign, eager-and-inefficient-bool-check
def bad2(items):
    d = all(items)
    e = all({"e": e for e in []})
    f = any(list({}))

def good1(items):
    g = all(items)  # starlark-lint-disable unused-assign
    # starlark-lint-disable unused-assign
    # starlark-lint-disable eager-and-inefficient-bool-check
    h = all({"h": h for h in []})
    # starlark-lint-disable inefficient-bool-check
    i = any(list({})) # starlark-lint-disable unused-assign
"#,
        );
        let res = m.lint(None);
        assert_eq!(res.len(), 10);
        assert!(res[0].problem.contains("Unused assignment of `a`"));
        assert!(res[1].problem.contains("`b`"));
        assert!(res[2].problem.contains("`c`"));
        assert!(res[3].problem.contains("`d`"));
        assert!(res[4].problem.contains("`e`"));
        assert!(res[5].problem.contains("`f`"));
        assert!(res[6].original.contains("all({\"a\": a for a in []})"));
        assert!(
            res[7]
                .problem
                .contains("`any(list({}))` allocates a new list")
        );
        assert!(res[8].original.contains("all({\"e\": e for e in []})"));
        assert!(
            res[9]
                .problem
                .contains("`any(list({}))` allocates a new list")
        );
    }

    #[test]
    fn test_lint_suppressions_preceding_whitespace() {
        let m = module(
            r#"
def bad():
    a = 1

def good():
    # starlark-lint-disable unused-assign
    # extra comment
    b = 1
"#,
        );
        let res = m.lint(None);
        assert_eq!(res.len(), 1);
        assert!(res[0].problem.contains("Unused assignment of `a`"));
    }

    #[test]
    fn test_lint_suppressions_with_space_separator() {
        let m = module(
            r#"
def good():
    #    starlark-lint-disable unused-assign FIXME
    b = 1
"#,
        );
        let res = m.lint(None);
        assert!(res.is_empty());
    }

    #[test]
    fn test_lint_suppressions_multiline_span() {
        let m = module(
            r#"
def bad() -> str:
    pass
def good() -> str:
    pass       # starlark-lint-disable missing-return
"#,
        );
        let res = m.lint(None);
        assert_eq!(res.len(), 1);
        assert!(res[0].problem.contains("bad"));
    }

    #[test]
    fn test_lint_suppressions_small_span() {
        let m = module(
            r#"
load("@cell//t:rust_library.bzl", "rust_library") # starlark-lint-disable unused-load

def bad() -> str:
    pass
"#,
        );
        let res = m.lint(None);
        assert_eq!(res.len(), 1);
        assert!(res[0].problem.contains("bad"));
    }

    #[test]
    fn test_lint_suppressions_data() {
        let m = module(
            r#"
{no3: 1, no4: 2, yes: 3, no3: 3}

# starlark-lint-disable duplicate-key
{no3: 1, no4: 2, yes: 3, no3: 3}

{no3: 1, no4: 2, yes: 3, no3: 3} # starlark-lint-disable duplicate-key

{   no3: 1,
    no4: 2,
    yes: 3,
    # inline data suppression of one key doesnt work
    # starlark-lint-disable duplicate-key
    no3: 3
}

{   no3: 1,     # starlark-lint-disable duplicate-key
    no4: 2,
    yes: 3,
    # each offender has to be disabled
    # starlark-lint-disable duplicate-key
    no3: 3
}

# starlark-lint-disable duplicate-key
{   no3: 1,
    no4: 2,
    yes: 3,
    no3: 3
}
"#,
        );
        let res = m.lint(None);
        assert_eq!(res.len(), 2);
        assert_eq!(res[0].location.span.begin(), Pos::new(2));
        assert_eq!(res[1].location.span.begin(), Pos::new(183));
    }

    #[test]
    fn test_lint_suppressions_line_before() {
        let m = module(
            r#"
# starlark-lint-disable unused-load
load("@cell//buck/lib:rust_library.bzl", "rust_library")
load("@cell//buck/lib:rust_binary.bzl", "rust_binary")

def bad1() -> str:
    pass

# starlark-lint-disable missing-return
def good1() -> str:
    pass

# starlark-lint-disable missing-return
# must not be on the last line of a block of comments
def good2() -> str:
    pass

# suppressions accumulate in a block of comments,
# starlark-lint-disable missing-return, unreachable
# and you can put other comments between
# starlark-lint-disable unused-load
def good3() -> str:
    pass
"#,
        );
        let res = m.lint(None);
        assert_eq!(res.len(), 2);
        assert!(res[0].problem.contains("bad1"));
        assert!(res[1].problem.contains("rust_binary"));
    }

    #[test]
    fn test_lint_suppressions_line_before_windows_newlines() {
        let src = module(
            "\
            # starlark-lint-disable unused-load\r\n\
            load('@cell//buck/lib:rust_library.bzl', 'rust_library')",
        );
        let res = src.lint(None);
        assert!(res.is_empty());
    }

    #[test]
    fn test_lint_suppressions_inside_fn() {
        let m = module(
            r#"
def bad1() -> str:
    pass

def good1() -> str:
    # starlark-lint-disable missing-return
    pass

def good2() -> str:
    pass # starlark-lint-disable missing-return
"#,
        );
        let res = m.lint(None);
        assert_eq!(res.len(), 1);
        assert!(res[0].problem.contains("bad1"));
    }
}
