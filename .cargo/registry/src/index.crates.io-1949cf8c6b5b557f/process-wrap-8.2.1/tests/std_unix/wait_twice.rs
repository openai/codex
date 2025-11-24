use super::prelude::*;

#[test]
fn nowrap() -> Result<()> {
	let mut child = StdCommandWrap::with_new("echo", |command| {
		command.stdout(Stdio::null());
	})
	.spawn()?;

	let status = (child.wait())?;
	assert!(status.success());

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

	let status = (child.wait())?;
	assert!(status.success());

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

	let status = (child.wait())?;
	assert!(status.success());

	let status = (child.wait())?;
	assert!(status.success());

	Ok(())
}
