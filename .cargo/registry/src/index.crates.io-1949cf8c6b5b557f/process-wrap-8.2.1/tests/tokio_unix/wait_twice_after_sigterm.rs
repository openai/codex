use super::prelude::*;

#[tokio::test]
async fn nowrap() -> Result<()> {
	let mut child = TokioCommandWrap::with_new("yes", |command| {
		command.stdout(Stdio::null());
	})
	.spawn()?;
	assert!(child.try_wait()?.is_none(), "pre sigterm");

	child.signal(Signal::SIGTERM as _)?;

	let status = Box::into_pin(child.wait()).await?;
	assert_eq!(status.signal(), Some(Signal::SIGTERM as i32), "wait() one");

	let status = Box::into_pin(child.wait()).await?;
	assert_eq!(status.signal(), Some(Signal::SIGTERM as i32), "wait() two");

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

	let status = Box::into_pin(child.wait()).await?;
	assert_eq!(status.signal(), Some(Signal::SIGTERM as i32), "wait() one");

	let status = Box::into_pin(child.wait()).await?;
	assert_eq!(status.signal(), Some(Signal::SIGTERM as i32), "wait() two");

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

	let status = Box::into_pin(child.wait()).await?;
	assert_eq!(status.signal(), Some(Signal::SIGTERM as i32), "wait() one");

	let status = Box::into_pin(child.wait()).await?;
	assert_eq!(status.signal(), Some(Signal::SIGTERM as i32), "wait() two");

	Ok(())
}
