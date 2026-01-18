use anyhow::Context;
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use tokio::io::AsyncBufRead;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::process::Child;
use tokio::process::Command;
use tokio::sync::Mutex;

pub struct Transport {
    reader: Mutex<Box<dyn AsyncBufRead + Send + Unpin>>,
    writer: Mutex<Box<dyn AsyncWrite + Send + Unpin>>,
    _child: Option<Child>,
}

const MAX_CONTENT_LENGTH: usize = 100 * 1024 * 1024;

impl Transport {
    pub async fn spawn(
        command: &str,
        args: &[String],
        env: Option<&HashMap<String, String>>,
        cwd: Option<&Path>,
    ) -> Result<Self> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        if let Some(env) = env {
            cmd.envs(env);
        }
        if let Some(cwd) = cwd {
            cmd.current_dir(cwd);
        }

        let mut child = cmd.spawn().context("spawn language server")?;
        let stdin = child
            .stdin
            .take()
            .context("missing language server stdin")?;
        let stdout = child
            .stdout
            .take()
            .context("missing language server stdout")?;
        let stderr = child
            .stderr
            .take()
            .context("missing language server stderr")?;

        let reader: Box<dyn AsyncBufRead + Send + Unpin> = Box::new(BufReader::new(stdout));
        let writer: Box<dyn AsyncWrite + Send + Unpin> = Box::new(stdin);

        tokio::spawn(async move {
            let mut stderr = BufReader::new(stderr);
            let mut line = String::new();
            loop {
                line.clear();
                let bytes = stderr.read_line(&mut line).await.unwrap_or(0);
                if bytes == 0 {
                    break;
                }
                let trimmed = line.trim_end();
                if !trimmed.is_empty() {
                    tracing::debug!("lsp stderr: {trimmed}");
                }
            }
        });

        Ok(Self {
            reader: Mutex::new(reader),
            writer: Mutex::new(writer),
            _child: Some(child),
        })
    }

    pub fn from_io(
        reader: Box<dyn AsyncBufRead + Send + Unpin>,
        writer: Box<dyn AsyncWrite + Send + Unpin>,
    ) -> Self {
        Self {
            reader: Mutex::new(reader),
            writer: Mutex::new(writer),
            _child: None,
        }
    }

    pub async fn read_message(&self) -> Result<String> {
        let mut reader = self.reader.lock().await;
        read_framed_message(&mut *reader).await
    }

    pub async fn write_message(&self, message: &str) -> Result<()> {
        let mut writer = self.writer.lock().await;
        write_framed_message(&mut *writer, message).await
    }
}

pub async fn read_framed_message(reader: &mut (dyn AsyncBufRead + Send + Unpin)) -> Result<String> {
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line).await?;
        if bytes == 0 {
            anyhow::bail!("lsp stream closed");
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some(value) = trimmed
            .strip_prefix("Content-Length:")
            .or_else(|| trimmed.strip_prefix("content-length:"))
        {
            let value = value.trim();
            let parsed = value.parse::<usize>().context("parse Content-Length")?;
            content_length = Some(parsed);
        }
    }

    let content_length = content_length.context("missing Content-Length header")?;
    if content_length > MAX_CONTENT_LENGTH {
        anyhow::bail!("Content-Length {content_length} exceeds limit {MAX_CONTENT_LENGTH}");
    }
    let mut buffer = vec![0u8; content_length];
    reader.read_exact(&mut buffer).await?;
    let message = String::from_utf8(buffer).context("decode lsp message")?;
    Ok(message)
}

pub async fn write_framed_message(
    writer: &mut (dyn AsyncWrite + Send + Unpin),
    message: &str,
) -> Result<()> {
    let bytes = message.as_bytes();
    let length = bytes.len();
    let header = format!("Content-Length: {length}\r\n\r\n");
    writer.write_all(header.as_bytes()).await?;
    writer.write_all(bytes).await?;
    writer.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tokio::io::duplex;

    #[tokio::test]
    async fn framing_roundtrip() {
        let (mut client, server) = duplex(1024);
        let mut reader = BufReader::new(server);

        let message = "{\"jsonrpc\":\"2.0\",\"id\":1}";
        write_framed_message(&mut client, message).await.unwrap();

        let got = read_framed_message(&mut reader).await.unwrap();
        assert_eq!(got, message);
    }
}
