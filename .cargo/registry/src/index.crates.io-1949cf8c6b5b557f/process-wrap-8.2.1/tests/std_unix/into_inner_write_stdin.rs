use super::prelude::*;

#[test]
fn nowrap() -> Result<()> {
	let mut child = StdCommandWrap::with_new("cat", |command| {
		command.stdin(Stdio::piped()).stdout(Stdio::piped());
	})
	.spawn()?
	.into_inner();

	if let Some(mut din) = child.stdin.take() {
		din.write_all(b"hello")?;
	}

	let mut output = String::new();
	if let Some(mut out) = child.stdout.take() {
		out.read_to_string(&mut output)?;
	}

	assert_eq!(output.as_str(), "hello");
	Ok(())
}

#[test]
fn process_group() -> Result<()> {
	let mut child = StdCommandWrap::with_new("cat", |command| {
		command.stdin(Stdio::piped()).stdout(Stdio::piped());
	})
	.wrap(ProcessGroup::leader())
	.spawn()?
	.into_inner();

	if let Some(mut din) = child.stdin.take() {
		din.write_all(b"hello")?;
	}

	let mut output = String::new();
	if let Some(mut out) = child.stdout.take() {
		out.read_to_string(&mut output)?;
	}

	assert_eq!(output.as_str(), "hello");
	Ok(())
}

#[test]
fn process_session() -> Result<()> {
	let mut child = StdCommandWrap::with_new("cat", |command| {
		command.stdin(Stdio::piped()).stdout(Stdio::piped());
	})
	.wrap(ProcessSession)
	.spawn()?
	.into_inner();

	if let Some(mut din) = child.stdin.take() {
		din.write_all(b"hello")?;
	}

	let mut output = String::new();
	if let Some(mut out) = child.stdout.take() {
		out.read_to_string(&mut output)?;
	}

	assert_eq!(output.as_str(), "hello");
	Ok(())
}
