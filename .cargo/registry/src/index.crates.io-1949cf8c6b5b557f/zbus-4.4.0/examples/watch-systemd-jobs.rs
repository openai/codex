//! Example demonstrating how to monitor for D-Bus Signal events.
//!
//! Prints a message every time a systemd service or other job starts.
//!
//! Run with command: `cargo run --example watch-systemd-jobs`

use futures_util::stream::StreamExt;
use zbus::Connection;
use zbus_macros::proxy;
use zvariant::OwnedObjectPath;

fn main() {
    async_io::block_on(watch_systemd_jobs()).expect("Error listening to signal");
}

#[proxy(
    default_service = "org.freedesktop.systemd1",
    default_path = "/org/freedesktop/systemd1",
    interface = "org.freedesktop.systemd1.Manager"
)]
trait Systemd1Manager {
    // Defines signature for D-Bus signal named `JobNew`
    #[zbus(signal)]
    fn job_new(&self, id: u32, job: OwnedObjectPath, unit: String) -> zbus::Result<()>;
}

// NOTE: When changing this, please also keep `book/src/client.md` in sync.
async fn watch_systemd_jobs() -> zbus::Result<()> {
    let connection = Connection::system().await?;
    // `Systemd1ManagerProxy` is generated from `Systemd1Manager` trait
    let systemd_proxy = Systemd1ManagerProxy::new(&connection).await?;
    // Method `receive_job_new` is generated from `job_new` signal
    let mut new_jobs_stream = systemd_proxy.receive_job_new().await?;

    println!("Monitoring started systemd services or jobs...");

    while let Some(msg) = new_jobs_stream.next().await {
        // struct `JobNewArgs` is generated from `job_new` signal function arguments
        let args: JobNewArgs = msg.args().expect("Error parsing message");

        println!(
            "JobNew received: unit={} id={} path={}",
            args.unit, args.id, args.job
        );
    }

    panic!("Stream ended unexpectedly");
}
