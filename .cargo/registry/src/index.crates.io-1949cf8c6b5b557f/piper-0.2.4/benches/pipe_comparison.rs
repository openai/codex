use async_io::Async;
use criterion::{criterion_group, criterion_main, Criterion};
use futures_lite::{future, prelude::*};
use std::net::{TcpListener, TcpStream};

fn pipe_bench(c: &mut Criterion) {
    c.bench_function("pipe", |b| {
        let (mut reader, mut writer) = piper::pipe(16834);
        let data = vec![0; 16834];
        let mut buffer = vec![0; 16834];
        b.iter(move || {
            future::block_on(async {
                writer.write_all(&data).await.unwrap();
                reader.read_exact(&mut buffer).await.unwrap();
            });
        });
    });
}

fn syspipe_bench(c: &mut Criterion) {
    c.bench_function("syspipe_tcp", |b| {
        let (mut tx, mut rx) = future::block_on(self_pipe());
        let data = vec![0; 16834];
        let mut buffer = vec![0; 16834];
        b.iter(|| {
            future::block_on(async {
                tx.write_all(&data).await.unwrap();
                rx.read_exact(&mut buffer).await.unwrap();
            });
        })
    });

    #[cfg(unix)]
    {
        use std::os::unix::net::UnixStream;

        c.bench_function("syspipe_unix", |b| {
            let (mut tx, mut rx) = async_io::Async::<UnixStream>::pair().unwrap();
            let data = vec![0; 16834];
            let mut buffer = vec![0; 16834];
            b.iter(|| {
                future::block_on(async {
                    tx.write_all(&data).await.unwrap();
                    rx.read_exact(&mut buffer).await.unwrap();
                });
            })
        });
    }
}

async fn self_pipe() -> (Async<TcpStream>, Async<TcpStream>) {
    let listener = Async::<TcpListener>::bind(([127, 0, 0, 1], 0)).unwrap();
    let addr = listener.get_ref().local_addr().unwrap();

    let tx = Async::<TcpStream>::connect(addr).await.unwrap();
    let rx = listener.accept().await.unwrap().0;

    (tx, rx)
}

criterion_group!(benches, pipe_bench, syspipe_bench);

criterion_main!(benches);
