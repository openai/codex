use dotenvy::{dotenv_iter, Error};

fn main() -> Result<(), Error> {
    dotenvy::dotenv()?;
    for item in dotenv_iter()? {
        let (key, val) = item?;
        println!("{}={}", key, val);
    }
    Ok(())
}
