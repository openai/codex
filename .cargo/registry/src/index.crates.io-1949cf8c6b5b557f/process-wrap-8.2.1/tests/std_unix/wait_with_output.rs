use super::prelude::*;

#[test]
fn nowrap() -> Result<()> {
	let child = StdCommandWrap::with_new("echo", |command| {
		command.arg("hello").stdout(Stdio::piped());
	})
	.spawn()?;

	let output = (child.wait_with_output())?;
	assert!(output.status.success());
	assert_eq!(output.stdout, b"hello\n".to_vec());
	assert_eq!(output.stderr, Vec::new());
	Ok(())
}

#[test]
fn process_group() -> Result<()> {
	let child = StdCommandWrap::with_new("echo", |command| {
		command.arg("hello").stdout(Stdio::piped());
	})
	.wrap(ProcessGroup::leader())
	.spawn()?;

	let output = (child.wait_with_output())?;
	assert!(output.status.success());
	assert_eq!(output.stdout, b"hello\n".to_vec());
	assert_eq!(output.stderr, Vec::new());
	Ok(())
}

#[test]
fn process_session() -> Result<()> {
	let child = StdCommandWrap::with_new("echo", |command| {
		command.arg("hello").stdout(Stdio::piped());
	})
	.wrap(ProcessSession)
	.spawn()?;

	let output = (child.wait_with_output())?;
	assert!(output.status.success());
	assert_eq!(output.stdout, b"hello\n".to_vec());
	assert_eq!(output.stderr, Vec::new());
	Ok(())
}
