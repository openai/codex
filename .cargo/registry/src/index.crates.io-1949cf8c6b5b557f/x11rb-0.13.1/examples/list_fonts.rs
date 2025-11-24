// This program should produce output identical to `xlsfonts -lu`.

extern crate x11rb;

use x11rb::protocol::xproto::{ConnectionExt, FontDraw};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (conn, _) = connect(None)?;

    println!("DIR  MIN  MAX EXIST DFLT PROP ASC DESC NAME");
    for reply in conn.list_fonts_with_info(u16::max_value(), b"*")? {
        let reply = reply?;

        let dir = if reply.draw_direction == FontDraw::LEFT_TO_RIGHT {
            "-->"
        } else if reply.draw_direction == FontDraw::RIGHT_TO_LEFT {
            "<--"
        } else {
            "???"
        };

        let (min, max, indicator) = if reply.min_byte1 == 0 && reply.max_byte1 == 0 {
            (reply.min_char_or_byte2, reply.max_char_or_byte2, ' ')
        } else {
            (u16::from(reply.min_byte1), u16::from(reply.max_byte1), '*')
        };

        let all = if reply.all_chars_exist { "all" } else { "some" };

        let name = String::from_utf8_lossy(&reply.name);

        println!(
            "{} {}{:3} {}{:3} {:>5} {:4} {:4} {:3} {:4} {}",
            dir,
            indicator,
            min,
            indicator,
            max,
            all,
            reply.default_char,
            reply.properties.len(),
            reply.font_ascent,
            reply.font_descent,
            name
        );
    }
    Ok(())
}

include!("integration_test_util/connect.rs");
