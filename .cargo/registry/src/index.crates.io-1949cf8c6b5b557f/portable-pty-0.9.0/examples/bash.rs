//! This example demonstrates how to spawn a Bash shell using the `portable_pty` crate.
//! based on pty/examples/whoami.rs

use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use std::io::{Read, Write};
use std::sync::mpsc::channel;
use std::thread;

fn main() {
    let pty_system = NativePtySystem::default();

    // Open the PTY with specified size.
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();

    // Set up the command to launch Bash.
    let cmd = CommandBuilder::new("bash");
    let mut child = pair.slave.spawn_command(cmd).unwrap();

    drop(pair.slave);

    // Set up channels for reading and writing.
    let (tx, rx) = channel::<String>();
    let mut reader = pair.master.try_clone_reader().unwrap();
    let master_writer = pair.master.take_writer().unwrap();

    // Thread to read from the PTY and send data to the main thread.
    thread::spawn(move || {
        let mut buffer = [0u8; 1024];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    let output = String::from_utf8_lossy(&buffer[..n]);
                    println!("{}", output); // Print to stdout for visibility.
                }
                Err(e) => {
                    eprintln!("Error reading from PTY: {}", e);
                    break;
                }
            }
        }
    });

    // Thread to write input into the PTY.
    let tx_writer = thread::spawn(move || {
        handle_input_stream(rx, master_writer);
    });

    println!("You can now type commands for Bash (type 'exit' to quit):");

    // Main thread sends user input to the writer thread.
    loop {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();

        if input.trim() == "exit" {
            break;
        }

        tx.send(input).unwrap();
    }

    drop(tx);
    tx_writer.join().unwrap();

    println!("Waiting for Bash to exit...");
    let status = child.wait().unwrap();
    println!("Bash exited with status: {:?}", status);
}

fn handle_input_stream(rx: std::sync::mpsc::Receiver<String>, mut writer: Box<dyn Write + Send>) {
    for input in rx.iter() {
        if writer.write_all(input.as_bytes()).is_err() {
            eprintln!("Error writing to PTY");
            break;
        }
    }
}
