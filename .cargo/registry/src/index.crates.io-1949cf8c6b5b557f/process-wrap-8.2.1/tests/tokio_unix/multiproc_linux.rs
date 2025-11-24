#![cfg(target_os = "linux")]

use super::prelude::*;

#[tokio::test]
async fn process_group_kill_leader() -> Result<()> {
	let mut leader = TokioCommandWrap::with_new("tests/multiproc_helper.rs", |command| {
		command
			.arg("1")
			.arg("10")
			.stdout(Stdio::piped())
			.stderr(Stdio::null());
	})
	.wrap(KillOnDrop)
	.wrap(ProcessGroup::leader())
	.spawn()?;
	assert!(leader.try_wait()?.is_none(), "leader: pre kill");

	sleep(DIE_TIME).await;

	let stdout = leader
		.stdout()
		.take()
		.expect("Option.unwrap(): get leader stdout");
	let mut lines = BufReader::new(stdout).lines();
	let Some(line) = lines.next_line().await? else {
		panic!("expected line with child pid");
	};
	let Some((parent, child)) = line.split_once(':') else {
		panic!("expected line with parent and child pids");
	};

	let parent = parent.parse::<i32>().unwrap();
	let child = child.parse::<i32>().unwrap();

	assert_eq!(parent, leader.id().unwrap() as _);
	assert!(pid_alive(parent), "parent process should be alive");
	assert!(pid_alive(child), "child process should be alive");

	Box::into_pin(leader.kill()).await.unwrap();
	sleep(DIE_TIME).await;
	assert!(!pid_alive(parent), "parent process should be dead");
	assert!(!pid_alive(child), "child process should be dead");

	Ok(())
}

#[tokio::test]
async fn process_group_kill_group() -> Result<()> {
	let mut leader = TokioCommandWrap::with_new("tests/multiproc_helper.rs", |command| {
		command
			.arg("1")
			.arg("10")
			.stdout(Stdio::piped())
			.stderr(Stdio::null());
	})
	.wrap(KillOnDrop)
	.wrap(ProcessGroup::leader())
	.spawn()?;
	assert!(leader.try_wait()?.is_none(), "leader: pre kill");

	sleep(DIE_TIME).await;

	let stdout = leader
		.stdout()
		.take()
		.expect("Option.unwrap(): get leader stdout");
	let mut lines = BufReader::new(stdout).lines();
	let Some(line) = lines.next_line().await? else {
		panic!("expected line with child pid");
	};
	let Some((parent, child)) = line.split_once(':') else {
		panic!("expected line with parent and child pids");
	};

	let parent = parent.parse::<i32>().unwrap();
	let child = child.parse::<i32>().unwrap();

	assert_eq!(parent, leader.id().unwrap() as _);
	assert!(pid_alive(parent), "parent process should be alive");
	assert!(pid_alive(child), "child process should be alive");

	nix::sys::signal::killpg(nix::unistd::Pid::from_raw(parent), Signal::SIGKILL).unwrap();
	sleep(DIE_TIME).await;
	assert!(!pid_alive(child), "child process should be dead");
	Box::into_pin(leader.wait()).await.unwrap();
	assert!(!pid_alive(parent), "parent process should be dead");

	Ok(())
}

#[tokio::test]
async fn process_session_kill_leader() -> Result<()> {
	let mut leader = TokioCommandWrap::with_new("tests/multiproc_helper.rs", |command| {
		command
			.arg("1")
			.arg("10")
			.stdout(Stdio::piped())
			.stderr(Stdio::null());
	})
	.wrap(KillOnDrop)
	.wrap(ProcessSession)
	.spawn()?;
	assert!(leader.try_wait()?.is_none(), "leader: pre kill");

	sleep(DIE_TIME).await;

	let stdout = leader
		.stdout()
		.take()
		.expect("Option.unwrap(): get leader stdout");
	let mut lines = BufReader::new(stdout).lines();
	let Some(line) = lines.next_line().await? else {
		panic!("expected line with child pid");
	};
	let Some((parent, child)) = line.split_once(':') else {
		panic!("expected line with parent and child pids");
	};

	let parent = parent.parse::<i32>().unwrap();
	let child = child.parse::<i32>().unwrap();

	assert_eq!(parent, leader.id().unwrap() as _);
	assert!(pid_alive(parent), "parent process should be alive");
	assert!(pid_alive(child), "child process should be alive");

	Box::into_pin(leader.kill()).await.unwrap();
	sleep(DIE_TIME).await;
	assert!(!pid_alive(parent), "parent process should be dead");
	assert!(!pid_alive(child), "child process should be dead");

	Ok(())
}

#[tokio::test]
async fn process_session_kill_group() -> Result<()> {
	let mut leader = TokioCommandWrap::with_new("tests/multiproc_helper.rs", |command| {
		command
			.arg("1")
			.arg("10")
			.stdout(Stdio::piped())
			.stderr(Stdio::null());
	})
	.wrap(KillOnDrop)
	.wrap(ProcessSession)
	.spawn()?;
	assert!(leader.try_wait()?.is_none(), "leader: pre kill");

	sleep(DIE_TIME).await;

	let stdout = leader
		.stdout()
		.take()
		.expect("Option.unwrap(): get leader stdout");
	let mut lines = BufReader::new(stdout).lines();
	let Some(line) = lines.next_line().await? else {
		panic!("expected line with child pid");
	};
	let Some((parent, child)) = line.split_once(':') else {
		panic!("expected line with parent and child pids");
	};

	let parent = parent.parse::<i32>().unwrap();
	let child = child.parse::<i32>().unwrap();

	assert_eq!(parent, leader.id().unwrap() as _);
	assert!(pid_alive(parent), "parent process should be alive");
	assert!(pid_alive(child), "child process should be alive");

	nix::sys::signal::killpg(nix::unistd::Pid::from_raw(parent), Signal::SIGKILL).unwrap();
	sleep(DIE_TIME).await;
	assert!(!pid_alive(child), "child process should be dead");
	Box::into_pin(leader.wait()).await.unwrap();
	assert!(!pid_alive(parent), "parent process should be dead");

	Ok(())
}
