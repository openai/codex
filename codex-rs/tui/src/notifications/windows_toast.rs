use std::io;
use std::process::Command;
use std::process::Stdio;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;

const APP_ID: &str = "Codex";
const POWERSHELL_EXE: &str = "powershell.exe";

#[derive(Debug)]
pub struct WindowsToastBackend {
    encoded_title: String,
}

impl WindowsToastBackend {
    pub fn notify(&mut self, message: &str) -> io::Result<()> {
        let encoded_body = encode_argument(message);
        let encoded_command = build_encoded_command(&self.encoded_title, &encoded_body);
        spawn_powershell(encoded_command)
    }
}

impl Default for WindowsToastBackend {
    fn default() -> Self {
        WindowsToastBackend {
            encoded_title: encode_argument(APP_ID),
        }
    }
}

fn spawn_powershell(encoded_command: String) -> io::Result<()> {
    let mut command = Command::new(POWERSHELL_EXE);
    command
        .arg("-NoProfile")
        .arg("-NoLogo")
        .arg("-EncodedCommand")
        .arg(encoded_command)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "{POWERSHELL_EXE} exited with status {status}"
        )))
    }
}

fn build_encoded_command(encoded_title: &str, encoded_body: &str) -> String {
    let script = build_ps_script(encoded_title, encoded_body);
    encode_script_for_powershell(&script)
}

fn build_ps_script(encoded_title: &str, encoded_body: &str) -> String {
    format!(
        r#"
$encoding = [System.Text.Encoding]::UTF8
$titleText = $encoding.GetString([System.Convert]::FromBase64String("{encoded_title}"))
$bodyText = $encoding.GetString([System.Convert]::FromBase64String("{encoded_body}"))
[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
$doc = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02)
$textNodes = $doc.GetElementsByTagName("text")
$textNodes.Item(0).AppendChild($doc.CreateTextNode($titleText)) | Out-Null
$textNodes.Item(1).AppendChild($doc.CreateTextNode($bodyText)) | Out-Null
$toast = [Windows.UI.Notifications.ToastNotification]::new($doc)
[Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('Codex').Show($toast)
"#,
    )
}

fn encode_script_for_powershell(script: &str) -> String {
    let mut wide: Vec<u8> = Vec::with_capacity((script.len() + 1) * 2);
    for unit in script.encode_utf16() {
        let bytes = unit.to_le_bytes();
        wide.extend_from_slice(&bytes);
    }
    BASE64.encode(wide)
}

fn encode_argument(value: &str) -> String {
    BASE64.encode(escape_for_xml(value))
}

pub fn escape_for_xml(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::encode_script_for_powershell;
    use super::escape_for_xml;
    use pretty_assertions::assert_eq;

    #[test]
    fn escapes_xml_entities() {
        assert_eq!(escape_for_xml("5 > 3"), "5 &gt; 3");
        assert_eq!(escape_for_xml("a & b"), "a &amp; b");
        assert_eq!(escape_for_xml("<tag>"), "&lt;tag&gt;");
        assert_eq!(escape_for_xml("\"quoted\""), "&quot;quoted&quot;");
        assert_eq!(escape_for_xml("single 'quote'"), "single &apos;quote&apos;");
    }

    #[test]
    fn leaves_safe_text_unmodified() {
        assert_eq!(escape_for_xml("codex"), "codex");
        assert_eq!(escape_for_xml("multi word text"), "multi word text");
    }

    #[test]
    fn encodes_utf16le_for_powershell() {
        assert_eq!(encode_script_for_powershell("A"), "QQA=");
    }
}
