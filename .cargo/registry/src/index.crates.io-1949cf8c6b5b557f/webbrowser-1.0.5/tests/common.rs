use actix_files as fs;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use crossbeam_channel as cbc;
use rand::RngCore;
use std::{io::Write, path::PathBuf, sync::Arc};
use urlencoding::decode;
use webbrowser::{open_browser, Browser};

#[derive(Clone)]
struct AppState {
    tx: Arc<cbc::Sender<String>>,
}

async fn log_handler(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    if data.tx.send(req.uri().to_string()).is_err() {
        panic!("channel send failed");
    }
    let uri = req.uri();
    if uri.path() == URI_PNG_1PX {
        HttpResponse::Ok()
            .content_type("image/png")
            .body(PNG_1PX.to_vec())
    } else {
        HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(format!("<html><body><p>URI: {}</p><script type=\"text/javascript>window.close();</script></body></html>", req.uri()))
    }
}

async fn delayed_response(req: HttpRequest) -> impl Responder {
    let qs = req.query_string();
    let ms: u64 = qs
        .replace("ms=", "")
        .parse()
        .expect("failed to parse millis");
    tokio::time::sleep(tokio::time::Duration::from_millis(ms)).await;
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(format!(
            "<html><body><p>Delayed by {qs}ms</p></body></html>"
        ))
}

pub async fn check_request_received_using<F>(uri: String, host: &str, op: F)
where
    F: FnOnce(&str, u16),
{
    // initialize env logger
    let _ = env_logger::try_init();

    // start the server on a random port
    let bind_addr = format!("{host}:0");
    let (tx, rx) = cbc::bounded(2);
    let data = AppState {
        tx: Arc::new(tx.clone()),
    };
    let http_server = HttpServer::new(move || {
        let wasm_pkg_dir = "tests/test-wasm-app/pkg";
        let _ = std::fs::create_dir_all(std::path::Path::new(wasm_pkg_dir));
        App::new()
            .service(fs::Files::new("/static/wasm", wasm_pkg_dir))
            .service(web::scope("/utils").route("/delay", web::get().to(delayed_response)))
            .app_data(web::Data::new(data.clone()))
            .default_service(web::to(log_handler))
    })
    .bind(&bind_addr)
    .unwrap_or_else(|_| panic!("Can not bind to {}", &bind_addr));

    let port = http_server
        .addrs()
        .first()
        .expect("Failed to find bound address")
        .port();

    let server = http_server.run();
    let server_handle = server.handle();
    tokio::spawn(server);

    // invoke the op
    op(&format!("http://{}:{}{}", host, port, &uri), port);

    // wait for the url to be hit
    let timeout = option_env!("TEST_REQ_TIMEOUT")
        .map(|s| s.parse().expect("failed to parse TEST_REQ_TIMEOUT"))
        .unwrap_or(90);
    match rx.recv_timeout(std::time::Duration::from_secs(timeout)) {
        Ok(msg) => assert_eq!(decode(&msg).unwrap(), uri),
        Err(_) => panic!("failed to receive uri data"),
    }

    // stop the server
    server_handle.stop(true).await;
}

#[allow(dead_code)]
pub async fn check_request_received(browser: Browser, uri: String) {
    check_request_received_using(uri, "127.0.0.1", |url, _port| {
        open_browser(browser, url).expect("failed to open browser");
    })
    .await;
}

#[allow(dead_code)]
pub async fn check_local_file<F>(browser: Browser, html_dir: Option<PathBuf>, url_op: F)
where
    F: FnOnce(&PathBuf) -> String,
{
    let cwd = std::env::current_dir().expect("unable to determine current dir");
    let tmpdir = cwd.join("target").join("tmp");
    let html_dir = html_dir.unwrap_or(tmpdir);
    let id = rand::thread_rng().next_u32();
    let pb = html_dir.join(format!("test.{id}.html"));
    let img_uri = format!("{URI_PNG_1PX}?r={id}");
    check_request_received_using(img_uri, "127.0.0.1", |uri, _port| {
        let url = url_op(&pb);
        let mut html_file = std::fs::File::create(&pb).expect("failed to create html file");
        html_file
            .write_fmt(format_args!(
                "<p>html file: {}</p><p>url: {}</p>img: <img src=\"{}\"/>",
                &pb.as_os_str().to_string_lossy(),
                url,
                uri
            ))
            .expect("failed to write html file");
        drop(html_file);
        open_browser(browser, &url).expect("failed to open browser");
    })
    .await;
    let _ = std::fs::remove_file(&pb);
}

#[allow(dead_code)]
pub async fn check_browser(browser: Browser, platform: &str) {
    check_request_received(browser, format!("/{platform}")).await;
    check_request_received(browser, format!("/{platform}/😀😀😀")).await;
}

const URI_PNG_1PX: &str = "/img/1px.png";

// generated from https://shoonia.github.io/1x1/#ff4563ff
const PNG_1PX: [u8; 82] = [
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 2, 0,
    0, 0, 144, 119, 83, 222, 0, 0, 0, 1, 115, 82, 71, 66, 0, 174, 206, 28, 233, 0, 0, 0, 12, 73,
    68, 65, 84, 24, 87, 99, 248, 239, 154, 12, 0, 3, 238, 1, 168, 16, 134, 253, 64, 0, 0, 0, 0, 73,
    69, 78, 68, 174, 66, 96, 130,
];
