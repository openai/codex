use super::prelude::*;

#[test]
fn nowrap() -> Result<()> {
	let child = StdCommandWrap::with_new("powershell.exe", |command| {
		command.arg("/C").arg("echo hello").stdout(Stdio::null());
	})
	.spawn()?;

	assert_eq!(child.id(), child.inner().id());

	Ok(())
}

#[test]
fn job_object() -> Result<()> {
	let child = StdCommandWrap::with_new("powershell.exe", |command| {
		command.arg("/C").arg("echo hello").stdout(Stdio::null());
	})
	.wrap(JobObject)
	.spawn()?;

	assert_eq!(child.id(), child.inner().id());

	Ok(())
}
