//! Bazel-wired proof-of-concept crate reserved for future V8 experiments.

/// Returns the Bazel label for this proof-of-concept crate.
#[must_use]
pub fn bazel_target() -> &'static str {
    "//codex-rs/v8-poc:v8-poc"
}

/// Returns the embedded V8 version when the optional `rusty_v8` feature is enabled.
#[cfg(feature = "rusty_v8")]
#[must_use]
pub fn embedded_v8_version() -> &'static str {
    v8::V8::get_version()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::bazel_target;
    #[cfg(feature = "rusty_v8")]
    use std::sync::Once;

    #[cfg(feature = "rusty_v8")]
    fn initialize_v8() {
        static INIT: Once = Once::new();

        INIT.call_once(|| {
            v8::V8::initialize_platform(v8::new_default_platform(0, false).make_shared());
            v8::V8::initialize();
        });
    }

    #[cfg(feature = "rusty_v8")]
    fn evaluate_expression(expression: &str) -> String {
        initialize_v8();

        let isolate = &mut v8::Isolate::new(Default::default());
        v8::scope!(let scope, isolate);

        let context = v8::Context::new(scope, Default::default());
        let scope = &mut v8::ContextScope::new(scope, context);
        let source = v8::String::new(scope, expression).expect("expression should be valid UTF-8");
        let script = v8::Script::compile(scope, source, None).expect("expression should compile");
        let result = script.run(scope).expect("expression should evaluate");

        result.to_rust_string_lossy(scope)
    }

    #[test]
    fn exposes_expected_bazel_target() {
        assert_eq!(bazel_target(), "//codex-rs/v8-poc:v8-poc");
    }

    #[cfg(feature = "rusty_v8")]
    #[test]
    fn exposes_embedded_v8_version() {
        assert!(!super::embedded_v8_version().is_empty());
    }

    #[cfg(feature = "rusty_v8")]
    #[test]
    fn evaluates_integer_addition() {
        assert_eq!(evaluate_expression("1 + 2"), "3");
    }

    #[cfg(feature = "rusty_v8")]
    #[test]
    fn evaluates_string_concatenation() {
        assert_eq!(evaluate_expression("'hello ' + 'world'"), "hello world");
    }
}
