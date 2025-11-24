use super::prelude::*;

#[tokio::test]
async fn nowrap() -> Result<()> {
	let child = TokioCommandWrap::with_new("echo", |command| {
		command.stdout(Stdio::null());
	})
	.spawn()?;

	assert_eq!(child.id(), child.inner().id());

	Ok(())
}

#[tokio::test]
async fn process_group() -> Result<()> {
	let child = TokioCommandWrap::with_new("echo", |command| {
		command.stdout(Stdio::null());
	})
	.wrap(ProcessGroup::leader())
	.spawn()?;

	assert_eq!(child.id(), child.inner().id());

	Ok(())
}

#[tokio::test]
async fn process_session() -> Result<()> {
	let child = TokioCommandWrap::with_new("echo", |command| {
		command.stdout(Stdio::null());
	})
	.wrap(ProcessSession)
	.spawn()?;

	assert_eq!(child.id(), child.inner().id());

	Ok(())
}
