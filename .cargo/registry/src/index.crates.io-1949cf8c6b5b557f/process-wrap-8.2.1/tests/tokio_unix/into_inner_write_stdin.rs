use super::prelude::*;

#[tokio::test]
async fn nowrap() -> Result<()> {
	let mut child = TokioCommandWrap::with_new("cat", |command| {
		command.stdin(Stdio::piped()).stdout(Stdio::piped());
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

	assert_eq!(output.as_str(), "hello");
	Ok(())
}

#[tokio::test]
async fn process_group() -> Result<()> {
	let mut child = TokioCommandWrap::with_new("cat", |command| {
		command.stdin(Stdio::piped()).stdout(Stdio::piped());
	})
	.wrap(ProcessGroup::leader())
	.spawn()?
	.into_inner();

	if let Some(mut din) = child.stdin().take() {
		din.write_all(b"hello").await?;
	}

	let mut output = String::new();
	if let Some(mut out) = child.stdout().take() {
		out.read_to_string(&mut output).await?;
	}

	assert_eq!(output.as_str(), "hello");
	Ok(())
}

#[tokio::test]
async fn process_session() -> Result<()> {
	let mut child = TokioCommandWrap::with_new("cat", |command| {
		command.stdin(Stdio::piped()).stdout(Stdio::piped());
	})
	.wrap(ProcessSession)
	.spawn()?
	.into_inner();

	if let Some(mut din) = child.stdin().take() {
		din.write_all(b"hello").await?;
	}

	let mut output = String::new();
	if let Some(mut out) = child.stdout().take() {
		out.read_to_string(&mut output).await?;
	}

	assert_eq!(output.as_str(), "hello");
	Ok(())
}
