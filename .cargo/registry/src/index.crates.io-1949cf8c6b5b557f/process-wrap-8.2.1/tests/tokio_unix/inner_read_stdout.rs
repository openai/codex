use super::prelude::*;

#[tokio::test]
async fn nowrap() -> Result<()> {
	let mut child = TokioCommandWrap::with_new("echo", |command| {
		command.arg("hello").stdout(Stdio::piped());
	})
	.spawn()?;

	let mut output = String::new();
	if let Some(mut out) = child.stdout().take() {
		out.read_to_string(&mut output).await?;
	}

	assert_eq!(output.as_str(), "hello\n");
	Ok(())
}

#[tokio::test]
async fn process_group() -> Result<()> {
	let mut child = TokioCommandWrap::with_new("echo", |command| {
		command.arg("hello").stdout(Stdio::piped());
	})
	.wrap(ProcessGroup::leader())
	.spawn()?;

	let mut output = String::new();
	if let Some(mut out) = child.stdout().take() {
		out.read_to_string(&mut output).await?;
	}

	assert_eq!(output.as_str(), "hello\n");
	Ok(())
}

#[tokio::test]
async fn process_session() -> Result<()> {
	let mut child = TokioCommandWrap::with_new("echo", |command| {
		command.arg("hello").stdout(Stdio::piped());
	})
	.wrap(ProcessSession)
	.spawn()?;

	let mut output = String::new();
	if let Some(mut out) = child.stdout().take() {
		out.read_to_string(&mut output).await?;
	}

	assert_eq!(output.as_str(), "hello\n");
	Ok(())
}
