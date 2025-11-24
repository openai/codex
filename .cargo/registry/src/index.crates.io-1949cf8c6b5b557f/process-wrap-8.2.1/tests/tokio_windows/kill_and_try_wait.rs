use super::prelude::*;

#[tokio::test]
async fn nowrap() -> Result<()> {
	let mut child = TokioCommandWrap::with_new("powershell.exe", |command| {
		command.arg("/C").arg("pause").stdout(Stdio::null());
	})
	.spawn()?;
	assert!(child.try_wait()?.is_none(), "pre kill");

	Box::into_pin(child.kill()).await?;
	sleep(DIE_TIME).await;
	assert!(child.try_wait()?.is_some(), "try_wait() one");

	sleep(DIE_TIME).await;
	assert!(child.try_wait()?.is_some(), "try_wait() two");

	Ok(())
}

#[tokio::test]
async fn job_object() -> Result<()> {
	let mut child = TokioCommandWrap::with_new("powershell.exe", |command| {
		command.arg("/C").arg("pause").stdout(Stdio::null());
	})
	.wrap(JobObject)
	.spawn()?;
	assert!(child.try_wait()?.is_none(), "pre kill");

	Box::into_pin(child.kill()).await?;
	sleep(DIE_TIME).await;
	assert!(child.try_wait()?.is_some(), "try_wait() one");

	sleep(DIE_TIME).await;
	assert!(child.try_wait()?.is_some(), "try_wait() two");

	Ok(())
}
