use super::format_device_code_prompt;

#[test]
fn device_code_prompt_uses_terminal_relative_styles() {
    let prompt =
        format_device_code_prompt("1.2.3", "https://auth.openai.com/codex/device", "ABCD-EFGH");

    insta::assert_snapshot!(prompt.escape_default().to_string());
}
