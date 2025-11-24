use super::prelude::*;

#[tokio::test]
async fn nowrap() -> Result<()> {
	let mut child = TokioCommandWrap::with_new("yes", |command| {
		command.stdout(Stdio::null());
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
async fn process_group() -> Result<()> {
	let mut child = TokioCommandWrap::with_new("yes", |command| {
		command.stdout(Stdio::null());
	})
	.wrap(ProcessGroup::leader())
	.spawn()?;
	assert!(child.try_wait()?.is_none(), "pre kill");

	Box::into_pin(child.kill()).await?;

	let status = Box::into_pin(child.wait()).await?;
	assert!(!status.success());

	sleep(DIE_TIME).await;
	assert!(child.try_wait()?.is_some(), "try_wait() one");

	sleep(DIE_TIME).await;
	assert!(child.try_wait()?.is_some(), "try_wait() two");

	Ok(())
}

#[tokio::test]
async fn process_session() -> Result<()> {
	let mut child = TokioCommandWrap::with_new("yes", |command| {
		command.stdout(Stdio::null());
	})
	.wrap(ProcessSession)
	.spawn()?;
	assert!(child.try_wait()?.is_none(), "pre kill");

	Box::into_pin(child.kill()).await?;
	sleep(DIE_TIME).await;
	assert!(child.try_wait()?.is_some(), "try_wait() one");

	sleep(DIE_TIME).await;
	assert!(child.try_wait()?.is_some(), "try_wait() two");

	Ok(())
}
