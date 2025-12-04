use std::{
    borrow::Cow,
    env, fs,
    net::TcpListener,
    path::{Path, PathBuf},
    str::{self, FromStr as _},
};

use anyhow::{anyhow, Context, Result};
use tiny_http::{Header, Response, Server};

use super::wasm;

macro_rules! optional_resource {
    ($name:tt, $path:tt) => {
        #[cfg(TREE_SITTER_EMBED_WASM_BINDING)]
        fn $name(tree_sitter_dir: Option<&Path>) -> Cow<'static, [u8]> {
            if let Some(tree_sitter_dir) = tree_sitter_dir {
                Cow::Owned(fs::read(tree_sitter_dir.join($path)).unwrap())
            } else {
                Cow::Borrowed(include_bytes!(concat!("../../", $path)))
            }
        }

        #[cfg(not(TREE_SITTER_EMBED_WASM_BINDING))]
        fn $name(tree_sitter_dir: Option<&Path>) -> Cow<'static, [u8]> {
            if let Some(tree_sitter_dir) = tree_sitter_dir {
                Cow::Owned(fs::read(tree_sitter_dir.join($path)).unwrap())
            } else {
                Cow::Borrowed(&[])
            }
        }
    };
}

optional_resource!(get_playground_js, "docs/src/assets/js/playground.js");
optional_resource!(get_lib_js, "lib/binding_web/tree-sitter.js");
optional_resource!(get_lib_wasm, "lib/binding_web/tree-sitter.wasm");

fn get_main_html(tree_sitter_dir: Option<&Path>) -> Cow<'static, [u8]> {
    tree_sitter_dir.map_or(
        Cow::Borrowed(include_bytes!("playground.html")),
        |tree_sitter_dir| {
            Cow::Owned(fs::read(tree_sitter_dir.join("cli/src/playground.html")).unwrap())
        },
    )
}

pub fn serve(grammar_path: &Path, open_in_browser: bool) -> Result<()> {
    let server = get_server()?;
    let (grammar_name, language_wasm) = wasm::load_language_wasm_file(grammar_path)?;
    let url = format!("http://{}", server.server_addr());
    println!("Started playground on: {url}");
    if open_in_browser && webbrowser::open(&url).is_err() {
        eprintln!("Failed to open '{url}' in a web browser");
    }

    let tree_sitter_dir = env::var("TREE_SITTER_BASE_DIR").map(PathBuf::from).ok();
    let main_html = str::from_utf8(&get_main_html(tree_sitter_dir.as_deref()))
        .unwrap()
        .replace("THE_LANGUAGE_NAME", &grammar_name)
        .into_bytes();
    let playground_js = get_playground_js(tree_sitter_dir.as_deref());
    let lib_js = get_lib_js(tree_sitter_dir.as_deref());
    let lib_wasm = get_lib_wasm(tree_sitter_dir.as_deref());

    let html_header = Header::from_str("Content-Type: text/html").unwrap();
    let js_header = Header::from_str("Content-Type: application/javascript").unwrap();
    let wasm_header = Header::from_str("Content-Type: application/wasm").unwrap();

    for request in server.incoming_requests() {
        let res = match request.url() {
            "/" => response(&main_html, &html_header),
            "/tree-sitter-parser.wasm" => response(&language_wasm, &wasm_header),
            "/playground.js" => {
                if playground_js.is_empty() {
                    redirect("https://tree-sitter.github.io/tree-sitter/assets/js/playground.js")
                } else {
                    response(&playground_js, &js_header)
                }
            }
            "/tree-sitter.js" => {
                if lib_js.is_empty() {
                    redirect("https://tree-sitter.github.io/tree-sitter.js")
                } else {
                    response(&lib_js, &js_header)
                }
            }
            "/tree-sitter.wasm" => {
                if lib_wasm.is_empty() {
                    redirect("https://tree-sitter.github.io/tree-sitter.wasm")
                } else {
                    response(&lib_wasm, &wasm_header)
                }
            }
            _ => response(b"Not found", &html_header).with_status_code(404),
        };
        request
            .respond(res)
            .with_context(|| "Failed to write HTTP response")?;
    }

    Ok(())
}

fn redirect(url: &str) -> Response<&[u8]> {
    Response::empty(302)
        .with_data("".as_bytes(), Some(0))
        .with_header(Header::from_bytes("Location", url.as_bytes()).unwrap())
}

fn response<'a>(data: &'a [u8], header: &Header) -> Response<&'a [u8]> {
    Response::empty(200)
        .with_data(data, Some(data.len()))
        .with_header(header.clone())
}

fn get_server() -> Result<Server> {
    let addr = env::var("TREE_SITTER_PLAYGROUND_ADDR").unwrap_or_else(|_| "127.0.0.1".to_owned());
    let port = env::var("TREE_SITTER_PLAYGROUND_PORT")
        .map(|v| {
            v.parse::<u16>()
                .with_context(|| "Invalid port specification")
        })
        .ok();
    let listener = match port {
        Some(port) => {
            bind_to(&addr, port?).with_context(|| "Failed to bind to the specified port")?
        }
        None => get_listener_on_available_port(&addr)
            .with_context(|| "Failed to find a free port to bind to it")?,
    };
    let server =
        Server::from_listener(listener, None).map_err(|_| anyhow!("Failed to start web server"))?;
    Ok(server)
}

fn get_listener_on_available_port(addr: &str) -> Option<TcpListener> {
    (8000..12000).find_map(|port| bind_to(addr, port))
}

fn bind_to(addr: &str, port: u16) -> Option<TcpListener> {
    TcpListener::bind(format!("{addr}:{port}")).ok()
}
