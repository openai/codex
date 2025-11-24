extern crate x11rb;

use std::fs::{remove_file, File, OpenOptions};
use std::io::{Result as IOResult, Write};
use std::os::unix::io::AsRawFd;
use std::ptr::null_mut;

use libc::{mmap, MAP_FAILED, MAP_SHARED, PROT_READ, PROT_WRITE};
use rustix::fd::OwnedFd;

use x11rb::connection::Connection;
use x11rb::errors::{ConnectionError, ReplyError, ReplyOrIdError};
use x11rb::protocol::shm::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{self, ImageFormat, PixmapWrapper};

const TEMP_FILE_CONTENT: [u8; 8] = [0x00, 0x01, 0x02, 0x03, 0xff, 0xfe, 0xfd, 0xfc];

/// Get the supported SHM version from the X11 server
fn check_shm_version<C: Connection>(conn: &C) -> Result<Option<(u16, u16)>, ReplyError> {
    if conn
        .extension_information(shm::X11_EXTENSION_NAME)?
        .is_none()
    {
        return Ok(None);
    }

    let shm_version = conn.shm_query_version()?.reply()?;
    Ok(Some((shm_version.major_version, shm_version.minor_version)))
}

/// Get the bytes describing the first pixel at the given offset of the given shared memory segment
/// (interpreted in the screen's root_visual)
fn get_shared_memory_content_at_offset<C: Connection>(
    conn: &C,
    screen: &xproto::Screen,
    shmseg: shm::Seg,
    offset: u32,
) -> Result<Vec<u8>, ReplyOrIdError> {
    let width = match screen.root_depth {
        24 => 1,
        16 => 2,
        8 => 4,
        _ => panic!("I do not know how to handle depth {}", screen.root_depth),
    };
    let pixmap = conn.generate_id()?;
    conn.shm_create_pixmap(
        pixmap,
        screen.root,
        width,
        1,
        screen.root_depth,
        shmseg,
        offset,
    )?;
    let pixmap = PixmapWrapper::for_pixmap(conn, pixmap);

    let image = xproto::get_image(
        conn,
        ImageFormat::Z_PIXMAP,
        pixmap.pixmap(),
        0,
        0,
        width,
        1,
        !0,
    )?
    .reply()?;
    Ok(image.data)
}

fn use_shared_mem<C: Connection>(
    conn: &C,
    screen_num: usize,
    shmseg: shm::Seg,
) -> Result<(), ReplyOrIdError> {
    let screen = &conn.setup().roots[screen_num];

    let content = get_shared_memory_content_at_offset(conn, screen, shmseg, 0)?;
    assert_eq!(content[..], TEMP_FILE_CONTENT[0..4]);

    let content = get_shared_memory_content_at_offset(conn, screen, shmseg, 4)?;
    assert_eq!(content[..], TEMP_FILE_CONTENT[4..8]);

    Ok(())
}

/// Make a temporary file
fn make_file() -> IOResult<File> {
    let file_name = "shared_memory.bin";
    let mut file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(true)
        .open(file_name)?;
    file.write_all(&TEMP_FILE_CONTENT)?;
    remove_file(file_name)?;
    Ok(file)
}

fn send_fd<C: Connection>(conn: &C, screen_num: usize, file: File) -> Result<(), ReplyOrIdError> {
    let shmseg = conn.generate_id()?;
    conn.shm_attach_fd(shmseg, OwnedFd::from(file), false)?;

    use_shared_mem(conn, screen_num, shmseg)?;

    conn.shm_detach(shmseg)?;

    Ok(())
}

fn receive_fd<C: Connection>(conn: &C, screen_num: usize) -> Result<(), ReplyOrIdError> {
    let shmseg = conn.generate_id()?;
    let segment_size = TEMP_FILE_CONTENT.len() as _;
    let reply = conn
        .shm_create_segment(shmseg, segment_size, false)?
        .reply()?;
    let shm::CreateSegmentReply { shm_fd, .. } = reply;

    let addr = unsafe {
        mmap(
            null_mut(),
            segment_size as _,
            PROT_READ | PROT_WRITE,
            MAP_SHARED,
            shm_fd.as_raw_fd(),
            0,
        )
    };
    if addr == MAP_FAILED {
        conn.shm_detach(shmseg)?;
        return Err(ConnectionError::InsufficientMemory.into());
    }

    unsafe {
        let slice = std::slice::from_raw_parts_mut(addr as *mut u8, segment_size as _);
        slice.copy_from_slice(&TEMP_FILE_CONTENT);
    }

    use_shared_mem(conn, screen_num, shmseg)?;

    conn.shm_detach(shmseg)?;
    // Let's ignore the munmap() that we should do here

    Ok(())
}

fn main() {
    let file = make_file().expect("Failed to create temporary file for FD passing");
    match connect(None) {
        Err(err) => eprintln!("Failed to connect to the X11 server: {}", err),
        Ok((conn, screen_num)) => {
            // Check for SHM 1.2 support (needed for fd passing)
            if let Some((major, minor)) = check_shm_version(&conn).unwrap() {
                if major < 1 || (major == 1 && minor < 2) {
                    eprintln!(
                        "X11 server supports version {}.{} of the SHM extension, but version 1.2 \
                         is needed",
                        major, minor,
                    );
                    return;
                }
            } else {
                eprintln!("X11 server does not support SHM extension");
                return;
            }

            println!("Trying to send an FD");
            send_fd(&conn, screen_num, file).unwrap();
            println!("Trying to receive an FD");
            receive_fd(&conn, screen_num).unwrap();
        }
    }
}

include!("integration_test_util/connect.rs");
