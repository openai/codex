use base64::Engine;
use codex_login::get_auth_file;
use codex_login::try_read_auth_json;
use tempfile::tempdir;

fn fake_jwt_with_plan(plan: &str) -> String {
    #[derive(serde::Serialize)]
    struct Header {
        alg: &'static str,
        typ: &'static str,
    }
    let header = Header {
        alg: "none",
        typ: "JWT",
    };
    let payload = serde_json::json!({
        "email": "user@example.com",
        "https://api.openai.com/auth": {
            "chatgpt_plan_type": plan
        }
    });
    let b64 = |b: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b);
    let header_bytes =
        serde_json::to_vec(&header).unwrap_or_else(|e| panic!("serialize header: {e}"));
    let payload_bytes =
        serde_json::to_vec(&payload).unwrap_or_else(|e| panic!("serialize payload: {e}"));
    format!(
        "{}.{}.{}",
        b64(&header_bytes),
        b64(&payload_bytes),
        b64(b"sig")
    )
}

#[test]
fn reads_old_auth_json_without_account_id() {
    let dir = tempdir().unwrap_or_else(|e| panic!("tempdir failed: {e}"));
    let auth_path = get_auth_file(dir.path());

    // Simulate Python-era auth.json without `account_id` in tokens
    let data = serde_json::json!({
        "OPENAI_API_KEY": "sk-test",
        "tokens": {
            "id_token": fake_jwt_with_plan("pro"),
            "access_token": "at-123",
            "refresh_token": "rt-123"
        },
        "last_refresh": "2025-01-01T00:00:00Z"
    });
    let pretty =
        serde_json::to_vec_pretty(&data).unwrap_or_else(|e| panic!("serialize auth.json: {e}"));
    std::fs::write(&auth_path, pretty).unwrap_or_else(|e| panic!("write auth.json: {e}"));

    let auth = try_read_auth_json(&auth_path)
        .unwrap_or_else(|e| panic!("should parse old auth.json: {e}"));
    let tokens = auth.tokens.unwrap_or_else(|| panic!("tokens exist"));
    assert!(
        tokens.account_id.is_none(),
        "account_id should be None for old files"
    );
    assert_eq!(tokens.access_token, "at-123");
    assert_eq!(tokens.refresh_token, "rt-123");
    assert_eq!(
        tokens.id_token.get_chatgpt_plan_type().as_deref(),
        Some("Pro")
    );
}
