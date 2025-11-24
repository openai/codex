use super::prelude::*;

#[test]
fn nowrap() -> Result<()> {
	let child = StdCommandWrap::with_new("powershell.exe", |command| {
		command.arg("/C").arg("echo hello").stdout(Stdio::piped());
	})
	.spawn()?;

	let output = (child.wait_with_output())?;
	assert!(output.status.success());
	assert_eq!(output.stdout, b"hello\r\n".to_vec());
	assert_eq!(output.stderr, Vec::new());
	Ok(())
}

#[test]
fn job_object() -> Result<()> {
	let child = StdCommandWrap::with_new("powershell.exe", |command| {
		command.arg("/C").arg("echo hello").stdout(Stdio::piped());
	})
	.wrap(JobObject)
	.spawn()?;

	let output = (child.wait_with_output())?;
	assert!(output.status.success());
	assert_eq!(output.stdout, b"hello\r\n".to_vec());
	assert_eq!(output.stderr, Vec::new());
	Ok(())
}
