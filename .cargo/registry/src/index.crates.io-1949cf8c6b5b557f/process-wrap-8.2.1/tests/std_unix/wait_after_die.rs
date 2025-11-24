use super::prelude::*;

#[test]
fn nowrap() -> Result<()> {
	let mut child = StdCommandWrap::with_new("echo", |command| {
		command.stdout(Stdio::null());
	})
	.spawn()?;
	sleep(DIE_TIME);

	let status = (child.wait())?;
	assert!(status.success());

	Ok(())
}

#[test]
fn process_group() -> Result<()> {
	let mut child = StdCommandWrap::with_new("echo", |command| {
		command.stdout(Stdio::null());
	})
	.wrap(ProcessGroup::leader())
	.spawn()?;
	sleep(DIE_TIME);

	let status = (child.wait())?;
	assert!(status.success());

	Ok(())
}

#[test]
fn process_session() -> Result<()> {
	let mut child = StdCommandWrap::with_new("echo", |command| {
		command.stdout(Stdio::null());
	})
	.wrap(ProcessSession)
	.spawn()?;
	sleep(DIE_TIME);

	let status = (child.wait())?;
	assert!(status.success());

	Ok(())
}
