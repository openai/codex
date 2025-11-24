use std::{env, fs};
use tokio::io::BufReader;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::compat::TokioAsyncReadCompatExt as _;
use weezl::{decode, encode, BitOrder};

async fn pair() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind("localhost:0")
        .await
        .expect("No loop tcp for testing");
    let addr = listener.local_addr().expect("No address for listener");

    let connect = TcpStream::connect(addr);
    let accept = listener.accept();

    let (a, (b, _)) = tokio::try_join!(connect, accept).expect("Can connect");
    (a, b)
}

async fn assert_send_through(data: &[u8], send: &mut TcpStream, recv: &mut TcpStream) {
    let mut send = send.compat();
    let mut recv = BufReader::new(recv).compat();

    let mut encoder = encode::Encoder::new(BitOrder::Lsb, 8);
    let encode = encoder.into_async(&mut send).encode_all(data);

    let mut recv_buffer = vec![];
    let mut decoder = decode::Decoder::new(BitOrder::Lsb, 8);
    let decode = decoder.into_async(&mut recv_buffer).decode_all(&mut recv);

    let (encode, decode) = tokio::join!(encode, decode);
    encode.status.expect("Could send/encoded data");
    decode.status.expect("Could recv/decode data");

    assert_eq!(recv_buffer, data);
}

#[test]
fn with_streams() {
    let file = env::args().next().unwrap();
    let data = fs::read(file).unwrap();

    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let _enter = rt.enter();

    let (mut send, mut recv) = rt.block_on(pair());
    rt.block_on(assert_send_through(&data, &mut send, &mut recv));
}
