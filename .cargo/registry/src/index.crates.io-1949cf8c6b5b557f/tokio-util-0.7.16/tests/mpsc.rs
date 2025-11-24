use futures::sink::SinkExt;
use std::future::poll_fn;
use tokio::sync::mpsc::channel;
use tokio_test::task::spawn;
use tokio_test::{
    assert_ok, assert_pending, assert_ready, assert_ready_eq, assert_ready_err, assert_ready_ok,
};
use tokio_util::sync::PollSender;

#[tokio::test]
async fn simple() {
    let (send, mut recv) = channel(3);
    let mut send = PollSender::new(send);

    for i in 1..=3i32 {
        let mut reserve = spawn(poll_fn(|cx| send.poll_reserve(cx)));
        assert_ready_ok!(reserve.poll());
        send.send_item(i).unwrap();
    }

    let mut reserve = spawn(poll_fn(|cx| send.poll_reserve(cx)));
    assert_pending!(reserve.poll());

    assert_eq!(recv.recv().await.unwrap(), 1);
    assert!(reserve.is_woken());
    assert_ready_ok!(reserve.poll());

    drop(recv);

    send.send_item(42).unwrap();
}

#[tokio::test]
async fn simple_ref() {
    let v = [1, 2, 3i32];

    let (send, mut recv) = channel(3);
    let mut send = PollSender::new(send);

    for vi in v.iter() {
        let mut reserve = spawn(poll_fn(|cx| send.poll_reserve(cx)));
        assert_ready_ok!(reserve.poll());
        send.send_item(vi).unwrap();
    }

    let mut reserve = spawn(poll_fn(|cx| send.poll_reserve(cx)));
    assert_pending!(reserve.poll());

    assert_eq!(*recv.recv().await.unwrap(), 1);
    assert!(reserve.is_woken());
    assert_ready_ok!(reserve.poll());
    drop(recv);
    send.send_item(&42).unwrap();
}

#[tokio::test]
async fn repeated_poll_reserve() {
    let (send, mut recv) = channel::<i32>(1);
    let mut send = PollSender::new(send);

    let mut reserve = spawn(poll_fn(|cx| send.poll_reserve(cx)));
    assert_ready_ok!(reserve.poll());
    assert_ready_ok!(reserve.poll());
    send.send_item(1).unwrap();

    assert_eq!(recv.recv().await.unwrap(), 1);
}

#[tokio::test]
async fn abort_send() {
    let (send, mut recv) = channel(3);
    let mut send = PollSender::new(send);
    let send2 = send.get_ref().cloned().unwrap();

    for i in 1..=3i32 {
        let mut reserve = spawn(poll_fn(|cx| send.poll_reserve(cx)));
        assert_ready_ok!(reserve.poll());
        send.send_item(i).unwrap();
    }

    let mut reserve = spawn(poll_fn(|cx| send.poll_reserve(cx)));
    assert_pending!(reserve.poll());
    assert_eq!(recv.recv().await.unwrap(), 1);
    assert!(reserve.is_woken());
    assert_ready_ok!(reserve.poll());

    let mut send2_send = spawn(send2.send(5));
    assert_pending!(send2_send.poll());
    assert!(send.abort_send());
    assert!(send2_send.is_woken());
    assert_ready_ok!(send2_send.poll());

    assert_eq!(recv.recv().await.unwrap(), 2);
    assert_eq!(recv.recv().await.unwrap(), 3);
    assert_eq!(recv.recv().await.unwrap(), 5);
}

#[tokio::test]
async fn close_sender_last() {
    let (send, mut recv) = channel::<i32>(3);
    let mut send = PollSender::new(send);

    let mut recv_task = spawn(recv.recv());
    assert_pending!(recv_task.poll());

    send.close();

    assert!(recv_task.is_woken());
    assert!(assert_ready!(recv_task.poll()).is_none());
}

#[tokio::test]
async fn close_sender_not_last() {
    let (send, mut recv) = channel::<i32>(3);
    let mut send = PollSender::new(send);
    let send2 = send.get_ref().cloned().unwrap();

    let mut recv_task = spawn(recv.recv());
    assert_pending!(recv_task.poll());

    send.close();

    assert!(!recv_task.is_woken());
    assert_pending!(recv_task.poll());

    drop(send2);

    assert!(recv_task.is_woken());
    assert!(assert_ready!(recv_task.poll()).is_none());
}

#[tokio::test]
async fn close_sender_before_reserve() {
    let (send, mut recv) = channel::<i32>(3);
    let mut send = PollSender::new(send);

    let mut recv_task = spawn(recv.recv());
    assert_pending!(recv_task.poll());

    send.close();

    assert!(recv_task.is_woken());
    assert!(assert_ready!(recv_task.poll()).is_none());

    let mut reserve = spawn(poll_fn(|cx| send.poll_reserve(cx)));
    assert_ready_err!(reserve.poll());
}

#[tokio::test]
async fn close_sender_after_pending_reserve() {
    let (send, mut recv) = channel::<i32>(1);
    let mut send = PollSender::new(send);

    let mut recv_task = spawn(recv.recv());
    assert_pending!(recv_task.poll());

    let mut reserve = spawn(poll_fn(|cx| send.poll_reserve(cx)));
    assert_ready_ok!(reserve.poll());
    send.send_item(1).unwrap();

    assert!(recv_task.is_woken());

    let mut reserve = spawn(poll_fn(|cx| send.poll_reserve(cx)));
    assert_pending!(reserve.poll());
    drop(reserve);

    send.close();

    assert!(send.is_closed());
    let mut reserve = spawn(poll_fn(|cx| send.poll_reserve(cx)));
    assert_ready_err!(reserve.poll());
}

#[tokio::test]
async fn close_sender_after_successful_reserve() {
    let (send, mut recv) = channel::<i32>(3);
    let mut send = PollSender::new(send);

    let mut recv_task = spawn(recv.recv());
    assert_pending!(recv_task.poll());

    let mut reserve = spawn(poll_fn(|cx| send.poll_reserve(cx)));
    assert_ready_ok!(reserve.poll());
    drop(reserve);

    send.close();
    assert!(send.is_closed());
    assert!(!recv_task.is_woken());
    assert_pending!(recv_task.poll());

    let mut reserve = spawn(poll_fn(|cx| send.poll_reserve(cx)));
    assert_ready_ok!(reserve.poll());
}

#[tokio::test]
async fn abort_send_after_pending_reserve() {
    let (send, mut recv) = channel::<i32>(1);
    let mut send = PollSender::new(send);

    let mut recv_task = spawn(recv.recv());
    assert_pending!(recv_task.poll());

    let mut reserve = spawn(poll_fn(|cx| send.poll_reserve(cx)));
    assert_ready_ok!(reserve.poll());
    send.send_item(1).unwrap();

    assert_eq!(send.get_ref().unwrap().capacity(), 0);
    assert!(!send.abort_send());

    let mut reserve = spawn(poll_fn(|cx| send.poll_reserve(cx)));
    assert_pending!(reserve.poll());

    assert!(send.abort_send());
    assert_eq!(send.get_ref().unwrap().capacity(), 0);
}

#[tokio::test]
async fn abort_send_after_successful_reserve() {
    let (send, mut recv) = channel::<i32>(1);
    let mut send = PollSender::new(send);

    let mut recv_task = spawn(recv.recv());
    assert_pending!(recv_task.poll());

    assert_eq!(send.get_ref().unwrap().capacity(), 1);
    let mut reserve = spawn(poll_fn(|cx| send.poll_reserve(cx)));
    assert_ready_ok!(reserve.poll());
    assert_eq!(send.get_ref().unwrap().capacity(), 0);

    assert!(send.abort_send());
    assert_eq!(send.get_ref().unwrap().capacity(), 1);
}

#[tokio::test]
async fn closed_when_receiver_drops() {
    let (send, _) = channel::<i32>(1);
    let mut send = PollSender::new(send);

    let mut reserve = spawn(poll_fn(|cx| send.poll_reserve(cx)));
    assert_ready_err!(reserve.poll());
}

#[should_panic]
#[test]
fn start_send_panics_when_idle() {
    let (send, _) = channel::<i32>(3);
    let mut send = PollSender::new(send);

    send.send_item(1).unwrap();
}

#[should_panic]
#[test]
fn start_send_panics_when_acquiring() {
    let (send, _) = channel::<i32>(1);
    let mut send = PollSender::new(send);

    let mut reserve = spawn(poll_fn(|cx| send.poll_reserve(cx)));
    assert_ready_ok!(reserve.poll());
    send.send_item(1).unwrap();

    let mut reserve = spawn(poll_fn(|cx| send.poll_reserve(cx)));
    assert_pending!(reserve.poll());
    send.send_item(2).unwrap();
}

#[test]
fn sink_send_then_flush() {
    let (send, mut recv) = channel(1);
    let mut send = PollSender::new(send);

    let mut recv_task = spawn(recv.recv());
    assert_pending!(recv_task.poll());

    let mut ready = spawn(poll_fn(|cx| send.poll_ready_unpin(cx)));
    assert_ready_ok!(ready.poll());
    assert_ok!(send.start_send_unpin(()));

    let mut ready = spawn(poll_fn(|cx| send.poll_ready_unpin(cx)));
    assert_pending!(ready.poll());

    let mut flush = spawn(poll_fn(|cx| send.poll_flush_unpin(cx)));
    assert_ready_ok!(flush.poll());

    // Flushing does not mean that the sender becomes ready.
    let mut ready = spawn(poll_fn(|cx| send.poll_ready_unpin(cx)));
    assert_pending!(ready.poll());

    assert_ready_eq!(recv_task.poll(), Some(()));
    assert!(ready.is_woken());
    assert_ready_ok!(ready.poll());
}

#[test]
fn sink_send_then_close() {
    let (send, mut recv) = channel(1);
    let mut send = PollSender::new(send);

    let mut recv_task = spawn(recv.recv());
    assert_pending!(recv_task.poll());

    let mut ready = spawn(poll_fn(|cx| send.poll_ready_unpin(cx)));
    assert_ready_ok!(ready.poll());
    assert_ok!(send.start_send_unpin(1));

    let mut ready = spawn(poll_fn(|cx| send.poll_ready_unpin(cx)));
    assert_pending!(ready.poll());

    assert!(recv_task.is_woken());
    assert_ready_eq!(recv_task.poll(), Some(1));

    assert!(ready.is_woken());
    assert_ready_ok!(ready.poll());

    drop(recv_task);
    let mut recv_task = spawn(recv.recv());
    assert_pending!(recv_task.poll());
    assert_ok!(send.start_send_unpin(2));

    let mut close = spawn(poll_fn(|cx| send.poll_close_unpin(cx)));
    assert_ready_ok!(close.poll());

    assert!(recv_task.is_woken());
    assert_ready_eq!(recv_task.poll(), Some(2));

    drop(recv_task);
    let mut recv_task = spawn(recv.recv());
    assert_ready_eq!(recv_task.poll(), None);
}

#[test]
fn sink_send_ref() {
    let data = "data".to_owned();
    let (send, mut recv) = channel(1);
    let mut send = PollSender::new(send);

    let mut recv_task = spawn(recv.recv());
    assert_pending!(recv_task.poll());

    let mut ready = spawn(poll_fn(|cx| send.poll_ready_unpin(cx)));
    assert_ready_ok!(ready.poll());

    assert_ok!(send.start_send_unpin(data.as_str()));

    let mut flush = spawn(poll_fn(|cx| send.poll_flush_unpin(cx)));
    assert_ready_ok!(flush.poll());

    assert_ready_eq!(recv_task.poll(), Some("data"));
}
