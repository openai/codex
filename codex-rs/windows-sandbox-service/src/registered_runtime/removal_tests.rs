//! Exercise the actual finalizer authorization barrier without account or package mutations.

use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use pretty_assertions::assert_eq;
use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
use windows_sys::Win32::Foundation::WAIT_TIMEOUT;
use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;
use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;
use windows_sys::Win32::System::Threading::WaitForSingleObject;

#[test]
fn finalizer_script_parses_and_native_bindings_compile() -> Result<()> {
    let executable =
        PathBuf::from(std::env::var_os("SystemRoot").context("SystemRoot is missing")?)
            .join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
    let script = include_str!("removal.ps1");
    let native_start = script
        .find("Add-Type -TypeDefinition @\"")
        .context("find native bindings")?;
    let native_end = native_start
        + script[native_start..]
            .find("\n\"@")
            .context("find binding boundary")?
        + 3;
    let validation = format!(
        "$ErrorActionPreference = 'Stop'; $tokens = $null; $errors = $null;\n\
         $null = [Management.Automation.Language.Parser]::ParseInput([Console]::In.ReadToEnd(), [ref]$tokens, [ref]$errors);\n\
         if ($errors.Count) {{ throw ($errors | Out-String) }}\n{}",
        &script[native_start..native_end],
    );
    let mut child = Command::new(executable)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &validation,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()?;
    child
        .stdin
        .take()
        .context("capture parser input")?
        .write_all(script.as_bytes())?;
    let output = child.wait_with_output()?;
    ensure!(
        output.status.success(),
        "finalizer validation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[test]
fn finalizer_preserves_utf8_plan_and_requires_commit_and_stdin_eof() -> Result<()> {
    let mut system = [0u16; 32768];
    let length = unsafe { GetSystemDirectoryW(system.as_mut_ptr(), system.len() as u32) } as usize;
    ensure!(
        length > 0 && length < system.len(),
        "Windows system directory is unavailable"
    );
    let executable = PathBuf::from(String::from_utf16(&system[..length])?)
        .join(r"WindowsPowerShell\v1.0\powershell.exe");
    // Run the production decoder and protocol in isolation, never the cleanup body.
    let script = include_str!("removal.ps1");
    let input_anchor = "throw 'SYSTEM required' }";
    let input_start = script.find(input_anchor).context("find input setup")? + input_anchor.len();
    let input_end = script
        .find("if ([string]::IsNullOrEmpty")
        .context("find plan validation")?;
    let input_setup = &script[input_start..input_end];
    let start = script
        .find("    # EOF without a successful native-cleanup commit")
        .context("find commit barrier")?;
    let end = script[start..]
        .find("    $finished = $false")
        .context("find cleanup boundary")?
        + start;
    let home = r"C:\Users\Zoë-東京\.codex";
    let script = format!(
        "$ErrorActionPreference = 'Stop';
         [Console]::InputEncoding = [Text.Encoding]::GetEncoding(437);
         {input_setup}
         if ($plan.record.codex_home -cne '{home}') {{ throw 'UTF-8 plan was corrupted' }}
         [Console]::Out.WriteLine('waiting');\n{}\n[Console]::Out.WriteLine('released')",
        &script[start..end]
    );
    let mut plan = serde_json::to_vec(&serde_json::json!({ "record": { "codex_home": home } }))?;
    plan.push(b'\n');
    for (input, expected) in [
        ("", ""),
        ("ABORT\n", ""),
        ("COMMIT\n", "released"),
        ("COMMIT\ntrailing", ""),
    ] {
        let mut child = Command::new(&executable)
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                &script,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()?;
        // Batch the plan and command so decoder read-ahead also exercises the shared reader.
        let mut request = plan.clone();
        request.extend_from_slice(input.as_bytes());
        child
            .stdin
            .as_mut()
            .context("capture barrier input")?
            .write_all(&request)?;
        let mut output = BufReader::new(child.stdout.take().context("capture barrier output")?);
        let mut line = String::new();
        output.read_line(&mut line)?;
        assert_eq!(line.trim_end(), "waiting");
        if input.is_empty() || input.starts_with("COMMIT\n") {
            let before_eof = unsafe {
                WaitForSingleObject(child.as_raw_handle() as _, /*dwmilliseconds*/ 500)
            };
            assert_eq!(before_eof, WAIT_TIMEOUT);
        }
        drop(child.stdin.take());
        let after_eof = unsafe {
            WaitForSingleObject(child.as_raw_handle() as _, /*dwmilliseconds*/ 10_000)
        };
        assert_eq!(after_eof, WAIT_OBJECT_0);
        line.clear();
        output.read_line(&mut line)?;
        assert_eq!(line.trim_end(), expected);
        ensure!(child.wait()?.success(), "barrier subprocess failed");
    }
    Ok(())
}
