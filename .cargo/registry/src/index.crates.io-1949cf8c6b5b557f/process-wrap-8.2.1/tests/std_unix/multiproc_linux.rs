#![cfg(target_os = "linux")]

use super::prelude::*;

#[test]
fn process_group_kill_leader() -> Result<()> {
	let mut leader = StdCommandWrap::with_new("tests/multiproc_helper.rs", |command| {
		command
			.arg("1")
			.arg("10")
			.stdout(Stdio::piped())
			.stderr(Stdio::null());
	})
	.wrap(ProcessGroup::leader())
	.spawn()?;
	assert!(leader.try_wait()?.is_none(), "leader: pre kill");

	sleep(DIE_TIME);

	let stdout = leader
		.stdout()
		.take()
		.expect("Option.unwrap(): get leader stdout");
	let mut lines = BufReader::new(stdout).lines();
	let Some(Ok(line)) = lines.next() else {
		panic!("expected line with child pid");
	};
	let Some((parent, child)) = line.split_once(':') else {
		panic!("expected line with parent and child pids");
	};

	let parent = parent.parse::<i32>().unwrap();
	let child = child.parse::<i32>().unwrap();

	assert_eq!(parent, leader.id() as _);
	assert!(pid_alive(parent), "parent process should be alive");
	assert!(pid_alive(child), "child process should be alive");

	leader.kill().unwrap();
	sleep(DIE_TIME);
	assert!(!pid_alive(parent), "parent process should be dead");
	assert!(!pid_alive(child), "child process should be dead");

	Ok(())
}

#[test]
fn process_group_kill_group() -> Result<()> {
	let mut leader = StdCommandWrap::with_new("tests/multiproc_helper.rs", |command| {
		command
			.arg("1")
			.arg("10")
			.stdout(Stdio::piped())
			.stderr(Stdio::null());
	})
	.wrap(ProcessGroup::leader())
	.spawn()?;
	assert!(leader.try_wait()?.is_none(), "leader: pre kill");

	sleep(DIE_TIME);

	let stdout = leader
		.stdout()
		.take()
		.expect("Option.unwrap(): get leader stdout");
	let mut lines = BufReader::new(stdout).lines();
	let Some(Ok(line)) = lines.next() else {
		panic!("expected line with child pid");
	};
	let Some((parent, child)) = line.split_once(':') else {
		panic!("expected line with parent and child pids");
	};

	let parent = parent.parse::<i32>().unwrap();
	let child = child.parse::<i32>().unwrap();

	assert_eq!(parent, leader.id() as _);
	assert!(pid_alive(parent), "parent process should be alive");
	assert!(pid_alive(child), "child process should be alive");

	nix::sys::signal::killpg(nix::unistd::Pid::from_raw(parent), Signal::SIGKILL).unwrap();
	sleep(DIE_TIME);
	assert!(!pid_alive(child), "child process should be dead");
	leader.wait().unwrap();
	assert!(!pid_alive(parent), "parent process should be dead");

	Ok(())
}

#[test]
fn process_session_kill_leader() -> Result<()> {
	let mut leader = StdCommandWrap::with_new("tests/multiproc_helper.rs", |command| {
		command
			.arg("1")
			.arg("10")
			.stdout(Stdio::piped())
			.stderr(Stdio::null());
	})
	.wrap(ProcessSession)
	.spawn()?;
	assert!(leader.try_wait()?.is_none(), "leader: pre kill");

	sleep(DIE_TIME);

	let stdout = leader
		.stdout()
		.take()
		.expect("Option.unwrap(): get leader stdout");
	let mut lines = BufReader::new(stdout).lines();
	let Some(Ok(line)) = lines.next() else {
		panic!("expected line with child pid");
	};
	let Some((parent, child)) = line.split_once(':') else {
		panic!("expected line with parent and child pids");
	};

	let parent = parent.parse::<i32>().unwrap();
	let child = child.parse::<i32>().unwrap();

	assert_eq!(parent, leader.id() as _);
	assert!(pid_alive(parent), "parent process should be alive");
	assert!(pid_alive(child), "child process should be alive");

	leader.kill().unwrap();
	sleep(DIE_TIME);
	assert!(!pid_alive(parent), "parent process should be dead");
	assert!(!pid_alive(child), "child process should be dead");

	Ok(())
}

#[test]
fn process_session_kill_group() -> Result<()> {
	let mut leader = StdCommandWrap::with_new("tests/multiproc_helper.rs", |command| {
		command
			.arg("1")
			.arg("10")
			.stdout(Stdio::piped())
			.stderr(Stdio::null());
	})
	.wrap(ProcessSession)
	.spawn()?;
	assert!(leader.try_wait()?.is_none(), "leader: pre kill");

	sleep(DIE_TIME);

	let stdout = leader
		.stdout()
		.take()
		.expect("Option.unwrap(): get leader stdout");
	let mut lines = BufReader::new(stdout).lines();
	let Some(Ok(line)) = lines.next() else {
		panic!("expected line with child pid");
	};
	let Some((parent, child)) = line.split_once(':') else {
		panic!("expected line with parent and child pids");
	};

	let parent = parent.parse::<i32>().unwrap();
	let child = child.parse::<i32>().unwrap();

	assert_eq!(parent, leader.id() as _);
	assert!(pid_alive(parent), "parent process should be alive");
	assert!(pid_alive(child), "child process should be alive");

	nix::sys::signal::killpg(nix::unistd::Pid::from_raw(parent), Signal::SIGKILL).unwrap();
	sleep(DIE_TIME);
	assert!(!pid_alive(child), "child process should be dead");
	leader.wait().unwrap();
	assert!(!pid_alive(parent), "parent process should be dead");

	Ok(())
}
