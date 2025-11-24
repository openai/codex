#!/usr/bin/env -S cargo +nightly -Zscript

fn main() {
	eprintln!("selfid={}", std::process::id());

	let n: u8 = std::env::args().nth(1).map_or(0, |s| s.parse().unwrap());
	let timeout = std::env::args()
		.nth(2)
		.map(|s| std::time::Duration::from_secs(s.parse().unwrap()));

	let children = (0..n)
		.into_iter()
		.map(|_| {
			std::process::Command::new(std::env::current_exe().unwrap())
				.stdout(std::process::Stdio::null())
				.spawn()
				.unwrap()
		})
		.collect::<Vec<_>>();

	println!(
		"{}:{}",
		std::process::id(),
		children
			.iter()
			.map(|c| c.id().to_string())
			.collect::<Vec<_>>()
			.join(",")
	);

	if let Some(timeout) = timeout {
		std::thread::sleep(timeout);
	} else {
		std::thread::park();
	}

	for mut child in children {
		child.kill().ok();
	}
}
