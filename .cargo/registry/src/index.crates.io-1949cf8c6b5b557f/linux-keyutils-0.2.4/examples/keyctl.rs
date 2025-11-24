//! Example CLI application that allows you to interact
//! with the Linux kernel keyring from user space.
//!
//! Demo code for the linux_keyutils crate.
use clap::Parser;
use linux_keyutils::{KeyPermissionsBuilder, Permission};
use linux_keyutils::{KeyRing, KeyRingIdentifier};
use std::error::Error;
use zeroize::Zeroizing;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(subcommand)]
    subcommand: Command,
}

#[derive(clap::Subcommand, Debug, PartialEq)]
enum Command {
    /// Create a new key
    Create {
        #[clap(short, long)]
        description: String,

        #[clap(short, long)]
        secret: String,
    },
    /// Read the secret from a key
    Read {
        #[clap(short, long)]
        description: String,
    },
    /// Change ownership of a key
    Chown {
        #[clap(short, long)]
        description: String,

        #[clap(short, long)]
        uid: Option<u32>,

        #[clap(short, long)]
        gid: Option<u32>,
    },
    /// Change permissions of a key
    Chmod {
        #[clap(short, long)]
        description: String,
    },
    /// Invalidate a key
    Invalidate {
        #[clap(short, long)]
        description: String,
    },
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    // Obtain the default User keyring for the current UID/user
    // See [KeyRingIdentifier] and `man 2 keyctl` for more information on default
    // keyrings for processes.
    let ring = KeyRing::from_special_id(KeyRingIdentifier::Session, false)?;

    _ = match args.subcommand {
        // Add a new key to the keyring
        Command::Create {
            description,
            secret,
        } => {
            let key = ring.add_key(&description, &secret)?;
            println!("Created key with ID {:?}", key.get_id());
        }
        // Search for an existing key by description and read the secret
        // data from the keyring
        Command::Read { description } => {
            let key = ring.search(&description)?;
            let mut buf = Zeroizing::new([0u8; 2048]);
            let len = key.read(&mut buf)?;
            println!("Secret {:?}", std::str::from_utf8(&buf[..len])?);
        }
        // Search for an existing key by description and attempt to
        // change ownership of the key
        Command::Chown {
            description,
            uid,
            gid,
        } => {
            let key = ring.search(&description)?;
            key.chown(uid, gid)?;
        }
        // Search for an existing key by description and attempt to
        // change permissions of the key
        Command::Chmod { description } => {
            let key = ring.search(&description)?;
            let perms = KeyPermissionsBuilder::builder()
                .user(Permission::ALL)
                .build();
            key.set_perms(perms)?;
        }
        // Search for an existing key by description and attempt to
        // invalidate they key
        Command::Invalidate { description } => {
            let key = ring.search(&description)?;
            key.invalidate()?;
            println!("Removed key with ID {:?}", key.get_id());
        }
    };

    Ok(())
}
