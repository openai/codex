use super::prelude::*;

#[test]
fn nowrap() -> Result<()> {
	let mut child = StdCommandWrap::with_new("echo", |command| {
		command.arg("hello").stdout(Stdio::piped());
	})
	.spawn()?;

	let mut output = String::new();
	if let Some(mut out) = child.stdout().take() {
		out.read_to_string(&mut output)?;
	}

	assert_eq!(output.as_str(), "hello\n");
	Ok(())
}

#[test]
fn process_group() -> Result<()> {
	let mut child = StdCommandWrap::with_new("echo", |command| {
		command.arg("hello").stdout(Stdio::piped());
	})
	.wrap(ProcessGroup::leader())
	.spawn()?;

	let mut output = String::new();
	if let Some(mut out) = child.stdout().take() {
		out.read_to_string(&mut output)?;
	}

	assert_eq!(output.as_str(), "hello\n");
	Ok(())
}

#[test]
fn process_session() -> Result<()> {
	let mut child = StdCommandWrap::with_new("echo", |command| {
		command.arg("hello").stdout(Stdio::piped());
	})
	.wrap(ProcessSession)
	.spawn()?;

	let mut output = String::new();
	if let Some(mut out) = child.stdout().take() {
		out.read_to_string(&mut output)?;
	}

	assert_eq!(output.as_str(), "hello\n");
	Ok(())
}
