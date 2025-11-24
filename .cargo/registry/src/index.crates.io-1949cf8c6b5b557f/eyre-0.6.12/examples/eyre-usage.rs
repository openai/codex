use eyre::{eyre, Report, WrapErr};

fn main() -> Result<(), Report> {
    let e: Report = eyre!("oh no this program is just bad!");

    Err(e).wrap_err("usage example successfully experienced a failure")
}
