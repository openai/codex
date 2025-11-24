#![allow(missing_docs)]
//! <https://langsec.org> cat says:
//!
//! ```text
//!  ___ _   _ _    _      ___ ___ ___ ___   ___ _  _ ___ _____ ___ ___  _  _
//! | __| | | | |  | |    | _ \ __/ __/ _ \ / __| \| |_ _|_   _|_ _/ _ \| \| |
//! | _|| |_| | |__| |__  |   / _| (_| (_) | (_ | .` || |  | |  | | (_) | .` |
//! |_|  \___/|____|____| |_|_\___\___\___/ \___|_|\_|___| |_| |___\___/|_|\_|
//!
//!
//!                      .__....._             _.....__,
//!                        .": o :':         ;': o :".
//!                        `. `-' .'.       .'. `-' .'
//!                          `---'             `---'
//!
//!                _...----...      ...   ...      ...----..._
//!             .-'__..-""'----    `.  `"`  .'    ----'""-..__`-.
//!            '.-'   _.--"""'       `-._.-'       '"""--._   `-.`
//!            '  .-"'                  :                  `"-.  `
//!              '   `.              _.'"'._              .'   `
//!                    `.       ,.-'"       "'-.,       .'
//!                      `.                           .'
//!                        `-._                   _.-'
//!                            `"'--...___...--'"`
//!
//!  ___ ___ ___ ___  ___ ___   ___ ___  ___   ___ ___ ___ ___ ___ _  _  ___
//! | _ ) __| __/ _ \| _ \ __| | _ \ _ \/ _ \ / __| __/ __/ __|_ _| \| |/ __|
//! | _ \ _|| _| (_) |   / _|  |  _/   / (_) | (__| _|\__ \__ \| || .` | (_ |
//! |___/___|_| \___/|_|_\___| |_| |_|_\\___/ \___|___|___/___/___|_|\_|\___|
//! ```
//!
//! <https://langsec.org/ForWantOfANail-h2hc2014.pdf>

#[macro_use]
mod macros;

pub(crate) mod alert;
pub(crate) mod base;
pub(crate) mod ccs;
pub(crate) mod codec;
pub(crate) mod deframer;
pub(crate) mod enums;
pub(crate) mod fragmenter;
pub(crate) mod handshake;
pub(crate) mod message;
pub(crate) mod persist;

#[cfg(test)]
mod handshake_test;

pub mod ffdhe_groups;
#[cfg(test)]
mod message_test;

#[cfg(test)]
mod tests {
    use super::codec::Reader;
    use super::message::{Message, OutboundOpaqueMessage};

    #[test]
    fn smoketest() {
        let bytes = include_bytes!("../testdata/handshake-test.1.bin");
        let mut r = Reader::init(bytes);

        while r.any_left() {
            let m = OutboundOpaqueMessage::read(&mut r).unwrap();

            let out = m.clone().encode();
            assert!(!out.is_empty());

            Message::try_from(m.into_plain_message()).unwrap();
        }
    }
}
