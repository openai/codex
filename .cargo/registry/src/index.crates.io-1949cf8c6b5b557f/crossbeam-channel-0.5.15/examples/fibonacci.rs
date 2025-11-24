//! An asynchronous fibonacci sequence generator.

use std::thread;

use crossbeam_channel::{bounded, Sender};

// Sends the Fibonacci sequence into the channel until it becomes disconnected.
fn fibonacci(sender: Sender<u64>) {
    let (mut x, mut y) = (0, 1);
    while sender.send(x).is_ok() {
        let tmp = x;
        x = y;
        y += tmp;
    }
}

fn main() {
    let (s, r) = bounded(0);
    thread::spawn(|| fibonacci(s));

    // Print the first 20 Fibonacci numbers.
    for num in r.iter().take(20) {
        println!("{}", num);
    }
}
