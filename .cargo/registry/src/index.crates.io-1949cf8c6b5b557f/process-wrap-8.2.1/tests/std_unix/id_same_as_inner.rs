use super::prelude::*;

#[test]
fn nowrap() -> Result<()> {
	let child = StdCommandWrap::with_new("echo", |command| {
		command.stdout(Stdio::null());
	})
	.spawn()?;

	assert_eq!(child.id(), child.inner().id());

	Ok(())
}

#[test]
fn process_group() -> Result<()> {
	let child = StdCommandWrap::with_new("echo", |command| {
		command.stdout(Stdio::null());
	})
	.wrap(ProcessGroup::leader())
	.spawn()?;

	assert_eq!(child.id(), child.inner().id());

	Ok(())
}

#[test]
fn process_session() -> Result<()> {
	let child = StdCommandWrap::with_new("echo", |command| {
		command.stdout(Stdio::null());
	})
	.wrap(ProcessSession)
	.spawn()?;

	assert_eq!(child.id(), child.inner().id());

	Ok(())
}
