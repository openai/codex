use super::prelude::*;

#[test]
fn nowrap() -> Result<()> {
	let mut child = StdCommandWrap::with_new("powershell.exe", |command| {
		command.arg("/C").arg("echo hello").stdout(Stdio::null());
	})
	.spawn()?;
	sleep(DIE_TIME);
	let status = child.try_wait()?;
	assert!(status.is_some());
	assert!(status.unwrap().success());

	Ok(())
}

#[test]
fn job_object() -> Result<()> {
	let mut child = StdCommandWrap::with_new("powershell.exe", |command| {
		command.arg("/C").arg("echo hello").stdout(Stdio::null());
	})
	.wrap(JobObject)
	.spawn()?;
	sleep(DIE_TIME);
	let status = child.try_wait()?;
	assert!(status.is_some());
	assert!(status.unwrap().success());

	Ok(())
}
