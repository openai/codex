use std::env;
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time;

fn examples_dir() -> PathBuf {
    let target_dir: PathBuf = env::var("CARGO_TARGET_DIR")
        .unwrap_or_else(|_| "target".to_string())
        .into();
    target_dir
        .join("debug")
        .join("examples")
}

fn server_command() -> Command {
    Command::new(examples_dir().join("server"))
}

fn client_command() -> Command {
    Command::new(examples_dir().join("client"))
}

fn wait_for_server(addr: &str) {
    for i in 0..10 {
        if TcpStream::connect(addr).is_ok() {
            return;
        }
        thread::sleep(time::Duration::from_millis(i * 100));
    }
    panic!("failed to connect to {:?} after 10 tries", addr);
}

#[test]
fn client() {
    let rc = client_command()
        .arg("https://google.com")
        .output()
        .expect("cannot run client example");

    assert!(rc.status.success());
}

#[test]
fn server() {
    let mut srv = server_command()
        .arg("1337")
        .spawn()
        .expect("cannot run server example");

    let addr = "localhost:1337";
    wait_for_server(addr);

    let output = Command::new("curl")
        .arg("--insecure")
        .arg("--http1.0")
        .arg(format!("https://{}", addr))
        .output()
        .expect("cannot run curl");

    srv.kill().unwrap();
    srv.wait()
        .expect("failed to wait on server process");

    if !output.status.success() {
        let version_stdout = Command::new("curl")
            .arg("--version")
            .output()
            .expect("cannot run curl to collect --version")
            .stdout;
        println!("curl version: {}", String::from_utf8_lossy(&version_stdout));
        println!("curl stderr:\n{}", String::from_utf8_lossy(&output.stderr));
    }

    assert_eq!(String::from_utf8_lossy(&output.stdout), "Try POST /echo\n");
}

#[test]
fn custom_ca_store() {
    let mut srv = server_command()
        .arg("1338")
        .spawn()
        .expect("cannot run server example");

    let addr = "localhost:1338";
    wait_for_server(addr);

    let rc = client_command()
        .arg(format!("https://{}", addr))
        .arg("examples/sample.pem")
        .output()
        .expect("cannot run client example");

    srv.kill().unwrap();
    srv.wait()
        .expect("failed to wait on server process");

    if !rc.status.success() {
        assert_eq!(String::from_utf8_lossy(&rc.stdout), "");
    }
}
