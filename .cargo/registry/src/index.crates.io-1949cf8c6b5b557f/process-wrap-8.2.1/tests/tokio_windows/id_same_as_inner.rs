use super::prelude::*;

#[tokio::test]
async fn nowrap() -> Result<()> {
	let child = TokioCommandWrap::with_new("powershell.exe", |command| {
		command.arg("/C").arg("echo hello").stdout(Stdio::null());
	})
	.spawn()?;

	assert_eq!(child.id(), child.inner().id());

	Ok(())
}

#[tokio::test]
async fn job_object() -> Result<()> {
	let child = TokioCommandWrap::with_new("powershell.exe", |command| {
		command.arg("/C").arg("echo hello").stdout(Stdio::null());
	})
	.wrap(JobObject)
	.spawn()?;

	assert_eq!(child.id(), child.inner().id());

	Ok(())
}
