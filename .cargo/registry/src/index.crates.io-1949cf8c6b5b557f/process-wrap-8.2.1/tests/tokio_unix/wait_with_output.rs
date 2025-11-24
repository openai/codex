use super::prelude::*;

#[tokio::test]
async fn nowrap() -> Result<()> {
	let child = TokioCommandWrap::with_new("echo", |command| {
		command.arg("hello").stdout(Stdio::piped());
	})
	.spawn()?;

	let output = Box::into_pin(child.wait_with_output()).await?;
	assert!(output.status.success());
	assert_eq!(output.stdout, b"hello\n".to_vec());
	assert_eq!(output.stderr, Vec::new());
	Ok(())
}

#[tokio::test]
async fn process_group() -> Result<()> {
	let child = TokioCommandWrap::with_new("echo", |command| {
		command.arg("hello").stdout(Stdio::piped());
	})
	.wrap(ProcessGroup::leader())
	.spawn()?;

	let output = Box::into_pin(child.wait_with_output()).await?;
	assert!(output.status.success());
	assert_eq!(output.stdout, b"hello\n".to_vec());
	assert_eq!(output.stderr, Vec::new());
	Ok(())
}

#[tokio::test]
async fn process_session() -> Result<()> {
	let child = TokioCommandWrap::with_new("echo", |command| {
		command.arg("hello").stdout(Stdio::piped());
	})
	.wrap(ProcessSession)
	.spawn()?;

	let output = Box::into_pin(child.wait_with_output()).await?;
	assert!(output.status.success());
	assert_eq!(output.stdout, b"hello\n".to_vec());
	assert_eq!(output.stderr, Vec::new());
	Ok(())
}
