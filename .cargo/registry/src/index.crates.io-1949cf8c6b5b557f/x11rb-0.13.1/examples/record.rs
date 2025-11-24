// This example shows how to use the record extension.
//
// The short version is: You do not want to use the record extension.
//
// The long version is: It's ugly and here is how it works.
//
// If you want to learn more about the record extension, it is recommended to read
// https://www.x.org/releases/X11R7.6/doc/recordproto/record.html
//
// This example is based on
// https://github.com/nibrahim/showkeys/blob/master/tests/record-example.c, which is GPLv3 and
// contains no copyright information. According to the git history, it was written by
// Noufal Ibrahim <noufal@nibrahim.net.in> in 2011.

use std::convert::TryFrom;

use x11rb::connection::Connection;
use x11rb::connection::RequestConnection;
use x11rb::errors::ParseError;
use x11rb::protocol::record::{self, ConnectionExt as _};
use x11rb::protocol::xproto;
use x11rb::wrapper::ConnectionExt;
use x11rb::x11_utils::TryParse;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // From https://www.x.org/releases/X11R7.6/doc/recordproto/record.html:
    // "The typical communication model for a recording client is to open two connections to the
    // server and use one for RC control and the other for reading protocol data."
    let (ctrl_conn, _) = connect(None)?;
    let (data_conn, _) = connect(None)?;

    // Check if the record extension is supported.
    if ctrl_conn
        .extension_information(record::X11_EXTENSION_NAME)?
        .is_none()
    {
        eprintln!("The X11 server does not support the RECORD extension");
        return Ok(());
    }
    let ver = ctrl_conn
        .record_query_version(
            record::X11_XML_VERSION.0 as _,
            record::X11_XML_VERSION.1 as _,
        )?
        .reply()?;
    println!(
        "requested RECORD extension version {:?}, server supports {:?}",
        record::X11_XML_VERSION,
        (ver.major_version, ver.minor_version)
    );

    // Set up a recording context
    let rc = ctrl_conn.generate_id()?;
    let empty = record::Range8 { first: 0, last: 0 };
    let empty_ext = record::ExtRange {
        major: empty,
        minor: record::Range16 { first: 0, last: 0 },
    };
    let range = record::Range {
        core_requests: empty,
        core_replies: empty,
        ext_requests: empty_ext,
        ext_replies: empty_ext,
        delivered_events: empty,
        device_events: record::Range8 {
            // We want notification of core X11 events between key press and motion notify
            first: xproto::KEY_PRESS_EVENT,
            last: xproto::MOTION_NOTIFY_EVENT,
        },
        errors: empty,
        client_started: false,
        client_died: false,
    };
    ctrl_conn
        .record_create_context(rc, 0, &[record::CS::ALL_CLIENTS.into()], &[range])?
        .check()?;

    // Apply a timeout if we are requested to do so.
    match std::env::var("X11RB_EXAMPLE_TIMEOUT")
        .ok()
        .and_then(|str| str.parse().ok())
    {
        None => {}
        Some(timeout) => {
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(timeout));
                ctrl_conn.record_disable_context(rc).unwrap();
                ctrl_conn.sync().unwrap();
            });
        }
    }

    // The above check() makes sure that the server already handled the CreateContext request.
    // Alternatively, we could do ctrl_conn.sync() here for the same effect.
    // We now switch to using "the other" connection.

    // FIXME: These constants should be added to the XML
    const START_OF_DATA: u8 = 4;
    const RECORD_FROM_SERVER: u8 = 0;
    for reply in data_conn.record_enable_context(rc)? {
        let reply = reply?;
        if reply.client_swapped {
            println!("Byte swapped clients are unsupported");
        } else if reply.category == RECORD_FROM_SERVER {
            let mut remaining = &reply.data[..];
            let mut should_exit = false;
            while !remaining.is_empty() {
                let (r, exit) = print_reply_data(&reply.data)?;
                remaining = r;
                if exit {
                    should_exit = true;
                }
            }
            if should_exit {
                break;
            }
        } else if reply.category == START_OF_DATA {
            println!("Press Escape to exit...");
        } else {
            println!("Got a reply with an unsupported category: {:?}", reply);
        }
    }
    Ok(())
}

// Print a single reply data packet and return the remaining data. When escape is pressed, true is
// also returned to indicate that we should exit.
fn print_reply_data(data: &[u8]) -> Result<(&[u8], bool), ParseError> {
    match data[0] {
        xproto::KEY_PRESS_EVENT => {
            let (event, remaining) = xproto::KeyPressEvent::try_parse(data)?;
            println!("key press: {:?}", event);
            Ok((remaining, false))
        }
        xproto::KEY_RELEASE_EVENT => {
            let (event, remaining) = xproto::KeyReleaseEvent::try_parse(data)?;
            println!("key release: {:?}", event);
            Ok((remaining, event.detail == 9))
        }
        xproto::BUTTON_PRESS_EVENT => {
            let (event, remaining) = xproto::ButtonPressEvent::try_parse(data)?;
            println!("button press: {:?}", event);
            Ok((remaining, false))
        }
        xproto::BUTTON_RELEASE_EVENT => {
            let (event, remaining) = xproto::ButtonReleaseEvent::try_parse(data)?;
            println!("button release: {:?}", event);
            Ok((remaining, false))
        }
        xproto::MOTION_NOTIFY_EVENT => {
            let (event, remaining) = xproto::MotionNotifyEvent::try_parse(data)?;
            println!("motion notify: {:?}", event);
            Ok((remaining, false))
        }
        0 => {
            // This is a reply, we compute its length as follows
            let (length, _) = u32::try_parse(&data[4..])?;
            let length = usize::try_from(length).unwrap() * 4 + 32;
            println!("unparsed reply: {:?}", &data[..length]);
            Ok((&data[length..], false))
        }
        _ => {
            // Error or event, they always have length 32
            // TODO: What about XGE events?
            println!("unparsed error/event: {:?}", &data[..32]);
            Ok((&data[32..], false))
        }
    }
}

include!("integration_test_util/connect.rs");
