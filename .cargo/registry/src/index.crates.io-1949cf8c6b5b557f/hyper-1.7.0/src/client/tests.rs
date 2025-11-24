/*
// FIXME: re-implement tests with `async/await`
#[test]
fn retryable_request() {
    let _ = pretty_env_logger::try_init();

    let mut rt = Runtime::new().expect("new rt");
    let mut connector = MockConnector::new();

    let sock1 = connector.mock("http://mock.local");
    let sock2 = connector.mock("http://mock.local");

    let client = Client::builder()
        .build::<_, crate::Body>(connector);

    client.pool.no_timer();

    {

        let req = Request::builder()
            .uri("http://mock.local/a")
            .body(Default::default())
            .unwrap();
        let res1 = client.request(req);
        let srv1 = poll_fn(|| {
            try_ready!(sock1.read(&mut [0u8; 512]));
            try_ready!(sock1.write(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n"));
            Ok(Async::Ready(()))
        }).map_err(|e: std::io::Error| panic!("srv1 poll_fn error: {}", e));
        rt.block_on(res1.join(srv1)).expect("res1");
    }
    drop(sock1);

    let req = Request::builder()
        .uri("http://mock.local/b")
        .body(Default::default())
        .unwrap();
    let res2 = client.request(req)
        .map(|res| {
            assert_eq!(res.status().as_u16(), 222);
        });
    let srv2 = poll_fn(|| {
        try_ready!(sock2.read(&mut [0u8; 512]));
        try_ready!(sock2.write(b"HTTP/1.1 222 OK\r\nContent-Length: 0\r\n\r\n"));
        Ok(Async::Ready(()))
    }).map_err(|e: std::io::Error| panic!("srv2 poll_fn error: {}", e));

    rt.block_on(res2.join(srv2)).expect("res2");
}

#[test]
fn conn_reset_after_write() {
    let _ = pretty_env_logger::try_init();

    let mut rt = Runtime::new().expect("new rt");
    let mut connector = MockConnector::new();

    let sock1 = connector.mock("http://mock.local");

    let client = Client::builder()
        .build::<_, crate::Body>(connector);

    client.pool.no_timer();

    {
        let req = Request::builder()
            .uri("http://mock.local/a")
            .body(Default::default())
            .unwrap();
        let res1 = client.request(req);
        let srv1 = poll_fn(|| {
            try_ready!(sock1.read(&mut [0u8; 512]));
            try_ready!(sock1.write(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n"));
            Ok(Async::Ready(()))
        }).map_err(|e: std::io::Error| panic!("srv1 poll_fn error: {}", e));
        rt.block_on(res1.join(srv1)).expect("res1");
    }

    let req = Request::builder()
        .uri("http://mock.local/a")
        .body(Default::default())
        .unwrap();
    let res2 = client.request(req);
    let mut sock1 = Some(sock1);
    let srv2 = poll_fn(|| {
        // We purposefully keep the socket open until the client
        // has written the second request, and THEN disconnect.
        //
        // Not because we expect servers to be jerks, but to trigger
        // state where we write on an assumedly good connection, and
        // only reset the close AFTER we wrote bytes.
        try_ready!(sock1.as_mut().unwrap().read(&mut [0u8; 512]));
        sock1.take();
        Ok(Async::Ready(()))
    }).map_err(|e: std::io::Error| panic!("srv2 poll_fn error: {}", e));
    let err = rt.block_on(res2.join(srv2)).expect_err("res2");
    assert!(err.is_incomplete_message(), "{:?}", err);
}

#[test]
fn checkout_win_allows_connect_future_to_be_pooled() {
    let _ = pretty_env_logger::try_init();

    let mut rt = Runtime::new().expect("new rt");
    let mut connector = MockConnector::new();


    let (tx, rx) = oneshot::channel::<()>();
    let sock1 = connector.mock("http://mock.local");
    let sock2 = connector.mock_fut("http://mock.local", rx);

    let client = Client::builder()
        .build::<_, crate::Body>(connector);

    client.pool.no_timer();

    let uri = "http://mock.local/a".parse::<crate::Uri>().expect("uri parse");

    // First request just sets us up to have a connection able to be put
    // back in the pool. *However*, it doesn't insert immediately. The
    // body has 1 pending byte, and we will only drain in request 2, once
    // the connect future has been started.
    let mut body = {
        let res1 = client.get(uri.clone())
            .map(|res| res.into_body().concat2());
        let srv1 = poll_fn(|| {
            try_ready!(sock1.read(&mut [0u8; 512]));
            // Chunked is used so as to force 2 body reads.
            try_ready!(sock1.write(b"\
                HTTP/1.1 200 OK\r\n\
                transfer-encoding: chunked\r\n\
                \r\n\
                1\r\nx\r\n\
                0\r\n\r\n\
            "));
            Ok(Async::Ready(()))
        }).map_err(|e: std::io::Error| panic!("srv1 poll_fn error: {}", e));

        rt.block_on(res1.join(srv1)).expect("res1").0
    };


    // The second request triggers the only mocked connect future, but then
    // the drained body allows the first socket to go back to the pool,
    // "winning" the checkout race.
    {
        let res2 = client.get(uri.clone());
        let drain = poll_fn(move || {
            body.poll()
        });
        let srv2 = poll_fn(|| {
            try_ready!(sock1.read(&mut [0u8; 512]));
            try_ready!(sock1.write(b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\nx"));
            Ok(Async::Ready(()))
        }).map_err(|e: std::io::Error| panic!("srv2 poll_fn error: {}", e));

        rt.block_on(res2.join(drain).join(srv2)).expect("res2");
    }

    // "Release" the mocked connect future, and let the runtime spin once so
    // it's all setup...
    {
        let mut tx = Some(tx);
        let client = &client;
        let key = client.pool.h1_key("http://mock.local");
        let mut tick_cnt = 0;
        let fut = poll_fn(move || {
            tx.take();

            if client.pool.idle_count(&key) == 0 {
                tick_cnt += 1;
                assert!(tick_cnt < 10, "ticked too many times waiting for idle");
                trace!("no idle yet; tick count: {}", tick_cnt);
                ::futures::task::current().notify();
                Ok(Async::NotReady)
            } else {
                Ok::<_, ()>(Async::Ready(()))
            }
        });
        rt.block_on(fut).unwrap();
    }

    // Third request just tests out that the "loser" connection was pooled. If
    // it isn't, this will panic since the MockConnector doesn't have any more
    // mocks to give out.
    {
        let res3 = client.get(uri);
        let srv3 = poll_fn(|| {
            try_ready!(sock2.read(&mut [0u8; 512]));
            try_ready!(sock2.write(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n"));
            Ok(Async::Ready(()))
        }).map_err(|e: std::io::Error| panic!("srv3 poll_fn error: {}", e));

        rt.block_on(res3.join(srv3)).expect("res3");
    }
}

#[cfg(feature = "nightly")]
#[bench]
fn bench_http1_get_0b(b: &mut test::Bencher) {
    let _ = pretty_env_logger::try_init();

    let mut rt = Runtime::new().expect("new rt");
    let mut connector = MockConnector::new();


    let client = Client::builder()
        .build::<_, crate::Body>(connector.clone());

    client.pool.no_timer();

    let uri = Uri::from_static("http://mock.local/a");

    b.iter(move || {
        let sock1 = connector.mock("http://mock.local");
        let res1 = client
            .get(uri.clone())
            .and_then(|res| {
                res.into_body().for_each(|_| Ok(()))
            });
        let srv1 = poll_fn(|| {
            try_ready!(sock1.read(&mut [0u8; 512]));
            try_ready!(sock1.write(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n"));
            Ok(Async::Ready(()))
        }).map_err(|e: std::io::Error| panic!("srv1 poll_fn error: {}", e));
        rt.block_on(res1.join(srv1)).expect("res1");
    });
}

#[cfg(feature = "nightly")]
#[bench]
fn bench_http1_get_10b(b: &mut test::Bencher) {
    let _ = pretty_env_logger::try_init();

    let mut rt = Runtime::new().expect("new rt");
    let mut connector = MockConnector::new();


    let client = Client::builder()
        .build::<_, crate::Body>(connector.clone());

    client.pool.no_timer();

    let uri = Uri::from_static("http://mock.local/a");

    b.iter(move || {
        let sock1 = connector.mock("http://mock.local");
        let res1 = client
            .get(uri.clone())
            .and_then(|res| {
                res.into_body().for_each(|_| Ok(()))
            });
        let srv1 = poll_fn(|| {
            try_ready!(sock1.read(&mut [0u8; 512]));
            try_ready!(sock1.write(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\n0123456789"));
            Ok(Async::Ready(()))
        }).map_err(|e: std::io::Error| panic!("srv1 poll_fn error: {}", e));
        rt.block_on(res1.join(srv1)).expect("res1");
    });
}
*/
