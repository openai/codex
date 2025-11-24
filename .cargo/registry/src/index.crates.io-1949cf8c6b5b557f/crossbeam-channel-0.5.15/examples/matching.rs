//! Using `select!` to send and receive on the same channel at the same time.
//!
//! This example is based on the following program in Go.
//!
//! Source:
//!   - https://web.archive.org/web/20171209034309/https://www.nada.kth.se/~snilsson/concurrency
//!   - http://www.nada.kth.se/~snilsson/concurrency/src/matching.go
//!
//! Copyright & License:
//!   - Stefan Nilsson
//!   - Creative Commons Attribution 3.0 Unported License
//!   - https://creativecommons.org/licenses/by/3.0/
//!
//! ```go
//! func main() {
//!     people := []string{"Anna", "Bob", "Cody", "Dave", "Eva"}
//!     match := make(chan string, 1) // Make room for one unmatched send.
//!     wg := new(sync.WaitGroup)
//!     for _, name := range people {
//!         wg.Add(1)
//!         go Seek(name, match, wg)
//!     }
//!     wg.Wait()
//!     select {
//!     case name := <-match:
//!         fmt.Printf("No one received %s’s message.\n", name)
//!     default:
//!         // There was no pending send operation.
//!     }
//! }
//!
//! // Seek either sends or receives, whichever possible, a name on the match
//! // channel and notifies the wait group when done.
//! func Seek(name string, match chan string, wg *sync.WaitGroup) {
//!     select {
//!     case peer := <-match:
//!         fmt.Printf("%s received a message from %s.\n", name, peer)
//!     case match <- name:
//!         // Wait for someone to receive my message.
//!     }
//!     wg.Done()
//! }
//! ```

use crossbeam_channel::{bounded, select};
use crossbeam_utils::thread;

fn main() {
    let people = vec!["Anna", "Bob", "Cody", "Dave", "Eva"];
    let (s, r) = bounded(1); // Make room for one unmatched send.

    // Either send my name into the channel or receive someone else's, whatever happens first.
    let seek = |name, s, r| {
        select! {
            recv(r) -> peer => println!("{} received a message from {}.", name, peer.unwrap()),
            send(s, name) -> _ => {}, // Wait for someone to receive my message.
        }
    };

    thread::scope(|scope| {
        for name in people {
            let (s, r) = (s.clone(), r.clone());
            scope.spawn(move |_| seek(name, s, r));
        }
    })
    .unwrap();

    // Check if there is a pending send operation.
    if let Ok(name) = r.try_recv() {
        println!("No one received {}’s message.", name);
    }
}
