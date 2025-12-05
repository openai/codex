//! Low-level ANSI-styling

/// Write ANSI colored text to the stream
pub fn write_colored<S: std::io::Write + ?Sized>(
    stream: &mut S,
    fg: Option<anstyle::AnsiColor>,
    bg: Option<anstyle::AnsiColor>,
    data: &[u8],
) -> std::io::Result<usize> {
    let non_default = fg.is_some() || bg.is_some();

    if non_default {
        if let Some(fg) = fg {
            write!(stream, "{}", fg.render_fg())?;
        }
        if let Some(bg) = bg {
            write!(stream, "{}", bg.render_bg())?;
        }
    }
    let written = stream.write(data)?;
    if non_default {
        write!(stream, "{}", anstyle::Reset.render())?;
    }
    Ok(written)
}
