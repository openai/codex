use super::prelude::*;

#[tokio::test]
async fn nowrap() -> Result<()> {
	let mut child = TokioCommandWrap::with_new("yes", |command| {
		command.stdout(Stdio::null());
	})
	.spawn()?;
	assert!(child.try_wait()?.is_none(), "pre sigterm");

	child.signal(Signal::SIGTERM as _)?;
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
	assert!(child.try_wait()?.is_none(), "pre sigterm");

	child.signal(Signal::SIGTERM as _)?;
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
	assert!(child.try_wait()?.is_none(), "pre sigterm");

	child.signal(Signal::SIGTERM as _)?;
	sleep(DIE_TIME).await;
	assert!(child.try_wait()?.is_some(), "try_wait() one");

	sleep(DIE_TIME).await;
	assert!(child.try_wait()?.is_some(), "try_wait() two");

	Ok(())
}
