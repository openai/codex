#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", target_os = "linux"))]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::task::coop::{consume_budget, has_budget_remaining};

const BUDGET: usize = 128;

/// Ensure that UDP sockets have functional budgeting
///
/// # Design
/// Two sockets communicate by spamming packets from one to the other.
///
/// In Linux, this packet will be slammed through the entire network stack and into the receiver's buffer during the
/// send system call because we are using the loopback interface.
/// This happens because the softirq chain invoked on send when using the loopback interface covers virtually the
/// entirety of the lifecycle of a packet within the kernel network stack.
///
/// As a result, neither socket will ever encounter an EWOULDBLOCK, and the only way for these to yield during the loop
/// is through budgeting.
///
/// A second task runs in the background and increments a counter before yielding, allowing us to know how many times sockets yielded.
/// Since we are both sending and receiving, that should happen once per 64 packets, because budgets are of size 128
/// and there are two budget events per packet, a send and a recv.
#[tokio::test]
#[cfg_attr(miri, ignore)] // No `socket` on miri.
async fn coop_budget_udp_send_recv() {
    const N_ITERATIONS: usize = 1024;

    const PACKET: &[u8] = b"Hello, world";
    const PACKET_LEN: usize = 12;

    assert_eq!(
        PACKET_LEN,
        PACKET.len(),
        "Defect in test, programmer can't do math"
    );

    // bind each socket to a dynamic port, forcing IPv4 addressing on the localhost interface
    let tx = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let rx = UdpSocket::bind("127.0.0.1:0").await.unwrap();

    tx.connect(rx.local_addr().unwrap()).await.unwrap();
    rx.connect(tx.local_addr().unwrap()).await.unwrap();

    let tracker = Arc::new(AtomicUsize::default());

    let tracker_clone = Arc::clone(&tracker);

    tokio::task::yield_now().await;

    tokio::spawn(async move {
        loop {
            tracker_clone.fetch_add(1, Ordering::SeqCst);

            tokio::task::yield_now().await;
        }
    });

    for _ in 0..N_ITERATIONS {
        tx.send(PACKET).await.unwrap();

        let mut tmp = [0; PACKET_LEN];

        // ensure that we aren't somehow accumulating other
        assert_eq!(
            PACKET_LEN,
            rx.recv(&mut tmp).await.unwrap(),
            "Defect in test case, received unexpected result from socket"
        );
        assert_eq!(
            PACKET, &tmp,
            "Defect in test case, received unexpected result from socket"
        );
    }

    assert_eq!(N_ITERATIONS / (BUDGET / 2), tracker.load(Ordering::SeqCst));
}

#[tokio::test]
async fn test_has_budget_remaining() {
    // At the beginning, budget should be available.
    assert!(has_budget_remaining());

    // Deplete the budget
    for _ in 0..BUDGET {
        consume_budget().await;
    }

    assert!(!has_budget_remaining());
}
