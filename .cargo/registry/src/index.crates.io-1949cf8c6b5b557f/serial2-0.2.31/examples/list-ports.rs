fn main() {
	match serial2::SerialPort::available_ports() {
		Err(e) => {
			eprintln!("Failed to enumerate serial ports: {e}");
			std::process::exit(1);
		},
		Ok(ports) => {
			eprintln!("Found {} ports", ports.len());
			for port in ports {
				println!("{}", port.display())
			}
		},
	}
}
