use super::prelude::*;

#[test]
fn nowrap() -> Result<()> {
	let mut child = StdCommandWrap::with_new("powershell.exe", |command| {
		command.arg("/C").arg("echo hello").stdout(Stdio::piped());
	})
	.spawn()?;

	let mut output = String::new();
	if let Some(mut out) = child.stdout().take() {
		out.read_to_string(&mut output)?;
	}

	assert_eq!(output.as_str(), "hello\r\n");
	Ok(())
}

#[test]
fn job_object() -> Result<()> {
	let mut child = StdCommandWrap::with_new("powershell.exe", |command| {
		command.arg("/C").arg("echo hello").stdout(Stdio::piped());
	})
	.wrap(JobObject)
	.spawn()?;

	let mut output = String::new();
	if let Some(mut out) = child.stdout().take() {
		out.read_to_string(&mut output)?;
	}

	assert_eq!(output.as_str(), "hello\r\n");
	Ok(())
}
