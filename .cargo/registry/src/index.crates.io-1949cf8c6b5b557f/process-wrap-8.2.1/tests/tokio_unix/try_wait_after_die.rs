use super::prelude::*;

#[tokio::test]
async fn nowrap() -> Result<()> {
	let mut child = TokioCommandWrap::with_new("echo", |command| {
		command.stdout(Stdio::null());
	})
	.spawn()?;
	sleep(DIE_TIME).await;
	let status = child.try_wait()?;
	assert!(status.is_some());
	assert!(status.unwrap().success());

	Ok(())
}

#[tokio::test]
async fn process_group() -> Result<()> {
	let mut child = TokioCommandWrap::with_new("echo", |command| {
		command.stdout(Stdio::null());
	})
	.wrap(ProcessGroup::leader())
	.spawn()?;
	sleep(DIE_TIME).await;
	let status = child.try_wait()?;
	assert!(status.is_some());
	assert!(status.unwrap().success());

	Ok(())
}

#[tokio::test]
async fn process_session() -> Result<()> {
	let mut child = TokioCommandWrap::with_new("echo", |command| {
		command.stdout(Stdio::null());
	})
	.wrap(ProcessSession)
	.spawn()?;
	sleep(DIE_TIME).await;
	let status = child.try_wait()?;
	assert!(status.is_some());
	assert!(status.unwrap().success());

	Ok(())
}
