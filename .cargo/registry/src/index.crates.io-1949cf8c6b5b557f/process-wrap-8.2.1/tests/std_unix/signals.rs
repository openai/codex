use super::prelude::*;

#[test]
fn nowrap() -> Result<()> {
	let mut child = StdCommandWrap::with_new("yes", |command| {
		command.stdout(Stdio::null());
	})
	.spawn()?;

	child.signal(Signal::SIGCONT as _)?;
	sleep(DIE_TIME);
	assert!(child.try_wait()?.is_none(), "not exited with sigcont");

	child.signal(Signal::SIGTERM as _)?;
	sleep(DIE_TIME);
	assert!(child.try_wait()?.is_some(), "exited with sigterm");

	Ok(())
}

#[test]
fn process_group() -> Result<()> {
	let mut child = StdCommandWrap::with_new("yes", |command| {
		command.stdout(Stdio::null());
	})
	.wrap(ProcessGroup::leader())
	.spawn()?;

	child.signal(Signal::SIGCONT as _)?;
	sleep(DIE_TIME);
	assert!(child.try_wait()?.is_none(), "not exited with sigcont");

	child.signal(Signal::SIGTERM as _)?;
	sleep(DIE_TIME);
	assert!(child.try_wait()?.is_some(), "exited with sigterm");

	Ok(())
}

#[test]
fn process_session() -> Result<()> {
	let mut child = StdCommandWrap::with_new("yes", |command| {
		command.stdout(Stdio::null());
	})
	.wrap(ProcessSession)
	.spawn()?;

	child.signal(Signal::SIGCONT as _)?;
	sleep(DIE_TIME);
	assert!(child.try_wait()?.is_none(), "not exited with sigcont");

	child.signal(Signal::SIGTERM as _)?;
	sleep(DIE_TIME);
	assert!(child.try_wait()?.is_some(), "exited with sigterm");

	Ok(())
}
