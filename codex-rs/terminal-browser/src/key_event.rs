use anyhow::Result;
use serde_json::Map;
use serde_json::Value;
use serde_json::json;

use crate::cdp::CdpClient;
use crate::input::BrowserKeyInput;
use crate::scripts;

const CONTROL_MODIFIER: u8 = 2;
const META_MODIFIER: u8 = 4;

struct KeyEvent<'a> {
    key: &'a str,
    code: &'a str,
    text: &'a str,
    modifiers: u8,
    windows_virtual_key_code: u32,
    commands: &'static [&'static str],
}

pub(crate) async fn dispatch_tool_key(client: &CdpClient, key: &str) -> Result<()> {
    let event = KeyEvent {
        key,
        code: scripts::key_code(key),
        text: scripts::key_text(key),
        modifiers: 0,
        windows_virtual_key_code: windows_virtual_key_code(key, scripts::key_code(key)),
        commands: &[],
    };
    dispatch(client, event).await
}

pub(crate) async fn dispatch_human_key(client: &CdpClient, input: &BrowserKeyInput) -> Result<()> {
    let text = input
        .text
        .as_deref()
        .unwrap_or_else(|| scripts::control_key_text(&input.key));
    let event = KeyEvent {
        key: &input.key,
        code: &input.code,
        text,
        modifiers: input.modifiers.cdp_mask(),
        windows_virtual_key_code: windows_virtual_key_code(&input.key, &input.code),
        commands: &[],
    };
    dispatch(client, event).await
}

pub(crate) async fn dispatch_select_all(client: &CdpClient) -> Result<()> {
    let modifiers = if cfg!(target_os = "macos") {
        META_MODIFIER
    } else {
        CONTROL_MODIFIER
    };
    dispatch(
        client,
        KeyEvent {
            key: "a",
            code: "KeyA",
            text: "",
            modifiers,
            windows_virtual_key_code: 65,
            commands: &["selectAll"],
        },
    )
    .await
}

pub(crate) async fn dispatch_backspace(client: &CdpClient) -> Result<()> {
    dispatch(
        client,
        KeyEvent {
            key: "Backspace",
            code: "Backspace",
            text: "",
            modifiers: 0,
            windows_virtual_key_code: 8,
            commands: &[],
        },
    )
    .await
}

async fn dispatch(client: &CdpClient, event: KeyEvent<'_>) -> Result<()> {
    let event_type = if event.text.is_empty() {
        "rawKeyDown"
    } else {
        "keyDown"
    };
    let mut key_down = Map::from_iter([
        ("type".to_string(), json!(event_type)),
        ("key".to_string(), json!(event.key)),
        ("code".to_string(), json!(event.code)),
        ("text".to_string(), json!(event.text)),
        ("unmodifiedText".to_string(), json!(event.text)),
        ("modifiers".to_string(), json!(event.modifiers)),
        (
            "windowsVirtualKeyCode".to_string(),
            json!(event.windows_virtual_key_code),
        ),
    ]);
    if !event.commands.is_empty() {
        key_down.insert("commands".to_string(), json!(event.commands));
    }
    client
        .call("Input.dispatchKeyEvent", Value::Object(key_down))
        .await?;
    client
        .call(
            "Input.dispatchKeyEvent",
            json!({
                "type": "keyUp",
                "key": event.key,
                "code": event.code,
                "modifiers": event.modifiers,
                "windowsVirtualKeyCode": event.windows_virtual_key_code,
            }),
        )
        .await?;
    Ok(())
}

fn windows_virtual_key_code(key: &str, code: &str) -> u32 {
    match key {
        "Backspace" => 8,
        "Tab" => 9,
        "Enter" => 13,
        "Shift" => 16,
        "Control" => 17,
        "Alt" => 18,
        "Escape" => 27,
        "Space" => 32,
        "PageUp" => 33,
        "PageDown" => 34,
        "End" => 35,
        "Home" => 36,
        "ArrowLeft" => 37,
        "ArrowUp" => 38,
        "ArrowRight" => 39,
        "ArrowDown" => 40,
        "Insert" => 45,
        "Delete" => 46,
        "Meta" => 91,
        _ => code_virtual_key_code(code).unwrap_or_else(|| character_virtual_key_code(key)),
    }
}

fn code_virtual_key_code(code: &str) -> Option<u32> {
    if let Some(letter) = code.strip_prefix("Key")
        && letter.len() == 1
    {
        return letter.chars().next().map(u32::from);
    }
    if let Some(digit) = code.strip_prefix("Digit")
        && digit.len() == 1
    {
        return digit.chars().next().map(u32::from);
    }
    match code {
        "Semicolon" => Some(186),
        "Equal" => Some(187),
        "Comma" => Some(188),
        "Minus" => Some(189),
        "Period" => Some(190),
        "Slash" => Some(191),
        "Backquote" => Some(192),
        "BracketLeft" => Some(219),
        "Backslash" => Some(220),
        "BracketRight" => Some(221),
        "Quote" => Some(222),
        _ => None,
    }
}

fn character_virtual_key_code(key: &str) -> u32 {
    let Some(character) = key.chars().next().filter(|_| key.chars().count() == 1) else {
        return 0;
    };
    if character.is_ascii_alphabetic() {
        return u32::from(character.to_ascii_uppercase());
    }
    match character {
        ';' | ':' => 186,
        '=' | '+' => 187,
        ',' | '<' => 188,
        '-' | '_' => 189,
        '.' | '>' => 190,
        '/' | '?' => 191,
        '`' | '~' => 192,
        '[' | '{' => 219,
        '\\' | '|' => 220,
        ']' | '}' => 221,
        '\'' | '"' => 222,
        _ => u32::from(character),
    }
}
