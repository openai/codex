use std::sync::Once;

static V8_INIT: Once = Once::new();

fn init_v8() {
    V8_INIT.call_once(|| {
        let platform = v8::new_default_platform(0, false).make_shared();
        v8::V8::initialize_platform(platform);
        v8::V8::initialize();
    });
}

pub fn evaluate_integer_expression(source: &str) -> Result<i64, String> {
    init_v8();

    let isolate = &mut v8::Isolate::new(v8::CreateParams::default());
    v8::scope!(let handle_scope, isolate);
    let context = v8::Context::new(handle_scope, Default::default());
    let scope = &mut v8::ContextScope::new(handle_scope, context);
    let code = v8::String::new(scope, source)
        .ok_or_else(|| "failed to allocate JavaScript source".to_string())?;
    let script = v8::Script::compile(scope, code, None)
        .ok_or_else(|| "failed to compile JavaScript source".to_string())?;
    let result = script
        .run(scope)
        .ok_or_else(|| "failed to execute JavaScript source".to_string())?;

    result
        .integer_value(scope)
        .ok_or_else(|| "JavaScript expression did not evaluate to an integer".to_string())
}

pub fn smoke_value() -> Result<i64, String> {
    evaluate_integer_expression("1 + 2")
}

#[cfg(test)]
mod tests {
    use super::smoke_value;

    #[test]
    fn evaluates_smoke_expression() {
        assert_eq!(smoke_value().unwrap(), 3);
    }
}
