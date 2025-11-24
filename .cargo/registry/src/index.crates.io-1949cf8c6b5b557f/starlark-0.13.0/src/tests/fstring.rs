/*
 * Copyright 2018 The Starlark in Rust Authors.
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

mod pass {
    use crate::assert::Assert;
    use crate::syntax::Dialect;

    fn assert() -> Assert<'static> {
        let mut a = Assert::new();
        a.dialect(&Dialect::AllOptionsInternal);
        a
    }

    #[test]
    fn basic() {
        assert().is_true(
            r#"
x = "a"
f"{x}b" == "ab"
"#,
        );

        assert().is_true(
            r#"
x = "a"
f"{ x }b" == "ab"
"#,
        );

        assert().is_true(
            r#"
x = "a"
f"{x}b{x}" == "aba"
"#,
        );
    }

    #[test]
    fn escape() {
        assert().is_true(
            r#"
x = "a"
f"{{}}{x}b{{x}}" == "{}ab{x}"
"#,
        );
    }

    #[test]
    fn function_parameter() {
        assert().is_true(
            r#"
def f(x):
  return f"q{x}"

f("1") == "q1"
"#,
        );
    }

    #[test]
    fn multiple() {
        assert().is_true(
            r#"
x = "x"
y = "y"
f"{x}{y}" == "xy"
"#,
        );

        assert().is_true(
            r#"
x = "x"
y = "y"
f"{x}{y}{x}" == "xyx"
"#,
        );
    }

    #[test]
    fn tuple() {
        assert().is_true(
            r#"
x = ("x",)
f"{x}" == '("x",)'
"#,
        );
    }

    #[test]
    fn conv() {
        assert().is_true(r#"x = 'a'; f"{x}" == 'a'"#);
        assert().is_true(r#"x = 'a'; f"{x!s}" == 'a'"#);
        assert().is_true(r#"x = 'a'; f"{x!r}" == '"a"'"#);
    }
}

mod fail {
    use starlark_syntax::golden_test_template::golden_test_template;

    use crate::assert::Assert;
    use crate::syntax::Dialect;

    fn fstring_golden_test_with_dialect(test_name: &str, text: &str, dialect: &Dialect) {
        let mut a = Assert::new();
        a.dialect(dialect);

        let err = a.fails(text, &[]);

        golden_test_template(
            &format!("src/tests/fstring/golden/{test_name}.err.golden.md"),
            &format!("{}", err),
        );
    }

    fn fstring_golden_test(test_name: &str, text: &str) {
        fstring_golden_test_with_dialect(test_name, text, &Dialect::AllOptionsInternal);
    }

    #[test]
    fn undeclared_variable() {
        fstring_golden_test("undeclared_variable", "f'foo {bar}'");
    }

    #[test]
    fn invalid_identifier() {
        fstring_golden_test("invalid_identifier", "f'foo {bar baz}'");
    }

    #[test]
    fn invalid_identifier_expression() {
        fstring_golden_test("invalid_identifier_expression", "f'foo {bar[123]}'");
    }

    #[test]
    fn invalid_identifier_triple_quotes() {
        fstring_golden_test("invalid_identifier_triple_quotes", "f'''foo {bar baz}'''");
    }

    #[test]
    fn invalid_identifier_raw() {
        fstring_golden_test("invalid_identifier_raw", "fr'foo {bar baz}'");
    }

    #[test]
    fn invalid_identifier_multiline() {
        fstring_golden_test("invalid_identifier_multiline", "f''''foo \n {bar baz}'''");
    }

    #[test]
    fn invalid_format() {
        fstring_golden_test("invalid_format", "f'foo {bar'");
    }

    #[test]
    fn escape() {
        // NOTE: this is wrong, we put the squiggly lines in the wrong place.
        fstring_golden_test("escape", "f'foo \\n {bar baz}'");
    }

    #[test]
    fn not_enabled() {
        fstring_golden_test_with_dialect("not_enabled", "f'{foo}'", &Dialect::Standard);
    }
}
