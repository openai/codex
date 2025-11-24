use super::prelude::*;

#[test]
fn nowrap() -> Result<()> {
	let mut child = StdCommandWrap::with_new("yes", |command| {
		command.stdout(Stdio::null());
	})
	.spawn()?;
	assert!(child.try_wait()?.is_none(), "pre sigterm");

	child.signal(Signal::SIGTERM as _)?;
	sleep(DIE_TIME);
	assert!(child.try_wait()?.is_some(), "try_wait() one");

	sleep(DIE_TIME);
	assert!(child.try_wait()?.is_some(), "try_wait() two");

	Ok(())
}

#[test]
fn process_group() -> Result<()> {
	let mut child = StdCommandWrap::with_new("yes", |command| {
		command.stdout(Stdio::null());
	})
	.wrap(ProcessGroup::leader())
	.spawn()?;
	assert!(child.try_wait()?.is_none(), "pre sigterm");

	child.signal(Signal::SIGTERM as _)?;
	sleep(DIE_TIME);
	assert!(child.try_wait()?.is_some(), "try_wait() one");

	sleep(DIE_TIME);
	assert!(child.try_wait()?.is_some(), "try_wait() two");

	Ok(())
}

#[test]
fn process_session() -> Result<()> {
	let mut child = StdCommandWrap::with_new("yes", |command| {
		command.stdout(Stdio::null());
	})
	.wrap(ProcessSession)
	.spawn()?;
	assert!(child.try_wait()?.is_none(), "pre sigterm");

	child.signal(Signal::SIGTERM as _)?;
	sleep(DIE_TIME);
	assert!(child.try_wait()?.is_some(), "try_wait() one");

	sleep(DIE_TIME);
	assert!(child.try_wait()?.is_some(), "try_wait() two");

	Ok(())
}
