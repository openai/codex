use super::prelude::*;

#[tokio::test]
async fn nowrap() -> Result<()> {
	let mut child = TokioCommandWrap::with_new("powershell.exe", |command| {
		command.arg("/C").arg("echo hello").stdout(Stdio::null());
	})
	.spawn()?;
	sleep(DIE_TIME).await;

	let status = Box::into_pin(child.wait()).await?;
	assert!(status.success());

	Ok(())
}

#[tokio::test]
async fn job_object() -> Result<()> {
	let mut child = TokioCommandWrap::with_new("powershell.exe", |command| {
		command.arg("/C").arg("echo hello").stdout(Stdio::null());
	})
	.wrap(JobObject)
	.spawn()?;
	sleep(DIE_TIME).await;

	let status = Box::into_pin(child.wait()).await?;
	assert!(status.success());

	Ok(())
}
