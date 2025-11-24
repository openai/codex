use super::prelude::*;

#[tokio::test]
async fn nowrap() -> Result<()> {
	let child = TokioCommandWrap::with_new("powershell.exe", |command| {
		command.arg("/C").arg("echo hello").stdout(Stdio::piped());
	})
	.spawn()?;

	let output = Box::into_pin(child.wait_with_output()).await?;
	assert!(output.status.success());
	assert_eq!(output.stdout, b"hello\r\n".to_vec());
	assert_eq!(output.stderr, Vec::new());
	Ok(())
}

#[tokio::test]
async fn job_object() -> Result<()> {
	let child = TokioCommandWrap::with_new("powershell.exe", |command| {
		command.arg("/C").arg("echo hello").stdout(Stdio::piped());
	})
	.wrap(JobObject)
	.spawn()?;

	let output = Box::into_pin(child.wait_with_output()).await?;
	assert!(output.status.success());
	assert_eq!(output.stdout, b"hello\r\n".to_vec());
	assert_eq!(output.stderr, Vec::new());
	Ok(())
}
