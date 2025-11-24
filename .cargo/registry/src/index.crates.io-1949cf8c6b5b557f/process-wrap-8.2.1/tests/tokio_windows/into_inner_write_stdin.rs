use super::prelude::*;

#[tokio::test]
async fn nowrap() -> Result<()> {
	let mut child = TokioCommandWrap::with_new("findstr", |command| {
		command
			.arg("^")
			.stdin(Stdio::piped())
			.stdout(Stdio::piped());
	})
	.spawn()?
	.into_inner();

	if let Some(mut din) = child.stdin().take() {
		din.write_all(b"hello").await?;
	}

	let mut output = String::new();
	if let Some(mut out) = child.stdout().take() {
		out.read_to_string(&mut output).await?;
	}

	assert_eq!(output.as_str(), "hello\r\n");
	Ok(())
}

#[tokio::test]
async fn job_object() -> Result<()> {
	let mut child = TokioCommandWrap::with_new("findstr", |command| {
		command
			.arg("^")
			.stdin(Stdio::piped())
			.stdout(Stdio::piped());
	})
	.wrap(JobObject)
	.spawn()?
	.into_inner();

	if let Some(mut din) = child.stdin().take() {
		din.write_all(b"hello").await?;
	}

	let mut output = String::new();
	if let Some(mut out) = child.stdout().take() {
		out.read_to_string(&mut output).await?;
	}

	assert_eq!(output.as_str(), "hello\r\n");
	Ok(())
}
