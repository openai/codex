use super::prelude::*;

#[test]
fn nowrap() -> Result<()> {
	let mut child = StdCommandWrap::with_new("powershell.exe", |command| {
		command.arg("/C").arg("pause").stdout(Stdio::null());
	})
	.spawn()?;
	assert!(child.try_wait()?.is_none(), "pre kill");

	(child.kill())?;
	sleep(DIE_TIME);
	assert!(child.try_wait()?.is_some(), "try_wait() one");

	sleep(DIE_TIME);
	assert!(child.try_wait()?.is_some(), "try_wait() two");

	Ok(())
}

#[test]
fn job_object() -> Result<()> {
	let mut child = StdCommandWrap::with_new("powershell.exe", |command| {
		command.arg("/C").arg("pause").stdout(Stdio::null());
	})
	.wrap(JobObject)
	.spawn()?;
	assert!(child.try_wait()?.is_none(), "pre kill");

	(child.kill())?;
	sleep(DIE_TIME);
	assert!(child.try_wait()?.is_some(), "try_wait() one");

	sleep(DIE_TIME);
	assert!(child.try_wait()?.is_some(), "try_wait() two");

	Ok(())
}
