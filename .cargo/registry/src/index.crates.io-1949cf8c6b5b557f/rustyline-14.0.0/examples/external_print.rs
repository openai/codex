use std::thread;
use std::time::Duration;

use rand::{thread_rng, Rng};

use rustyline::{DefaultEditor, ExternalPrinter, Result};

fn main() -> Result<()> {
    let mut rl = DefaultEditor::new()?;
    let mut printer = rl.create_external_printer()?;
    thread::spawn(move || {
        let mut rng = thread_rng();
        let mut i = 0usize;
        loop {
            printer
                .print(format!("External message #{i}"))
                .expect("External print failure");
            let wait_ms = rng.gen_range(1000..10000);
            thread::sleep(Duration::from_millis(wait_ms));
            i += 1;
        }
    });

    loop {
        let line = rl.readline("> ")?;
        rl.add_history_entry(line.as_str())?;
        println!("Line: {line}");
    }
}
