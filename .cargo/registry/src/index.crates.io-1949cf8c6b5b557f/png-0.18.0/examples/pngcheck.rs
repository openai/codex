#![allow(non_upper_case_globals)]

use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::path::{Path, PathBuf};

use clap::Parser;

use png::chunk;

#[derive(Parser)]
#[command(about, version)]
struct Config {
    /// test quietly (output only errors)
    #[arg(short, long)]
    quiet: bool,
    /// test verbosely (print most chunk data)
    #[arg(short, long)]
    verbose: bool,
    /// print contents of tEXt/zTXt/iTXt chunks (can be used with -q)
    #[arg(short, long)]
    text: bool,
    paths: Vec<PathBuf>,
}

fn display_interlaced(i: bool) -> &'static str {
    if i {
        "interlaced"
    } else {
        "non-interlaced"
    }
}

fn display_image_type(bits: u8, color: png::ColorType) -> String {
    use png::ColorType::*;
    format!(
        "{}-bit {}",
        bits,
        match color {
            Grayscale => "grayscale",
            Rgb => "RGB",
            Indexed => "palette",
            GrayscaleAlpha => "grayscale+alpha",
            Rgba => "RGB+alpha",
        }
    )
}
// channels after expansion of tRNS
fn final_channels(c: png::ColorType, trns: bool) -> u8 {
    use png::ColorType::*;
    match c {
        Grayscale => 1 + u8::from(trns),
        Rgb => 3,
        Indexed => 3 + u8::from(trns),
        GrayscaleAlpha => 2,
        Rgba => 4,
    }
}

fn check_image<P: AsRef<Path>>(c: &Config, fname: P) -> io::Result<()> {
    // TODO improve performance by reusing allocations from decoder
    use png::Decoded::*;
    let data = &mut vec![0; 10 * 1024][..];
    let mut reader = io::BufReader::new(File::open(&fname)?);
    let fname = fname.as_ref().to_string_lossy();
    let n = reader.read(data)?;
    let mut buf = &data[..n];
    let mut pos = 0;
    let mut decoder = png::StreamingDecoder::new();
    // Image data
    let mut width = 0;
    let mut height = 0;
    let mut color = png::ColorType::Grayscale;
    let mut bits = 0;
    let mut trns = false;
    let mut interlaced = false;
    let mut compressed_size = 0;
    let mut n_chunks = 0;
    let mut have_idat = false;
    macro_rules! c_ratio(
        // TODO add palette entries to compressed_size
        () => ({
            compressed_size as f32/(
                height as u64 *
                (width as u64 * final_channels(color, trns) as u64 * bits as u64 + 7)>>3
            ) as f32
        });
    );
    let display_error = |err| -> Result<_, io::Error> {
        if c.verbose {
            println!(": {}", err);
            print!("ERRORS DETECTED");
            println!(" in {}", fname);
        } else {
            if !c.quiet {
                println!("ERROR: {}", fname)
            }
            print!("{}: ", fname);
            println!("{}", err);
        }
        Ok(())
    };

    if c.verbose {
        print!("File: ");
        print!("{}", fname);
        print!(" ({}) bytes", data.len())
    }
    loop {
        if buf.is_empty() {
            // circumvent borrow checker
            assert!(!data.is_empty());
            let n = reader.read(data)?;

            // EOF
            if n == 0 {
                println!("ERROR: premature end of file {}", fname);
                break;
            }
            buf = &data[..n];
        }
        match decoder.update(buf, None) {
            Ok((_, ChunkComplete(chunk::IEND))) => {
                if !have_idat {
                    // This isn't beautiful. But it works.
                    display_error(png::DecodingError::IoError(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "IDAT chunk missing",
                    )))?;
                    break;
                }
                if !c.verbose && !c.quiet {
                    print!("OK: {}", fname);
                    println!(
                        " ({}x{}, {}{}, {}, {:.1}%)",
                        width,
                        height,
                        display_image_type(bits, color),
                        (if trns { "+trns" } else { "" }),
                        display_interlaced(interlaced),
                        100.0 * (1.0 - c_ratio!())
                    )
                } else if !c.quiet {
                    println!();
                    print!("No errors detected ");
                    println!(
                        "in {} ({} chunks, {:.1}% compression)",
                        fname,
                        n_chunks,
                        100.0 * (1.0 - c_ratio!()),
                    )
                }
                break;
            }
            Ok((n, res)) => {
                buf = &buf[n..];
                pos += n;
                match res {
                    ChunkBegin(len, type_str) => {
                        n_chunks += 1;
                        if c.verbose {
                            let chunk = type_str;
                            println!();
                            print!("  chunk ");
                            print!("{:?}", chunk);
                            print!(
                                " at offset {:#07x}, length {}",
                                pos - 4, // subtract chunk name length
                                len
                            )
                        }
                        match type_str {
                            chunk::IDAT => {
                                have_idat = true;
                                compressed_size += len
                            }
                            chunk::tRNS => {
                                trns = true;
                            }
                            _ => (),
                        }
                    }
                    ChunkComplete(chunk::IHDR) => {
                        width = decoder.info().unwrap().width;
                        height = decoder.info().unwrap().height;
                        bits = decoder.info().unwrap().bit_depth as u8;
                        color = decoder.info().unwrap().color_type;
                        interlaced = decoder.info().unwrap().interlaced;

                        if c.verbose {
                            println!();
                            print!(
                                "    {} x {} image, {}{}, {}",
                                width,
                                height,
                                display_image_type(bits, color),
                                (if trns { "+trns" } else { "" }),
                                display_interlaced(interlaced),
                            );
                        }
                    }
                    ChunkComplete(chunk::acTL) => {
                        let actl = decoder.info().unwrap().animation_control.unwrap();
                        println!();
                        print!("    {} frames, {} plays", actl.num_frames, actl.num_plays,);
                    }
                    ChunkComplete(chunk::fdAT) => {
                        let fctl = decoder.info().unwrap().frame_control.unwrap();
                        println!();
                        println!(
                            "    sequence #{}, {} x {} pixels @ ({}, {})",
                            fctl.sequence_number,
                            fctl.width,
                            fctl.height,
                            fctl.x_offset,
                            fctl.y_offset,
                            /*fctl.delay_num,
                            fctl.delay_den,
                            fctl.dispose_op,
                            fctl.blend_op,*/
                        );
                        print!(
                            "    {}/{} s delay, dispose: {}, blend: {}",
                            fctl.delay_num,
                            if fctl.delay_den == 0 {
                                100
                            } else {
                                fctl.delay_den
                            },
                            fctl.dispose_op,
                            fctl.blend_op,
                        );
                    }
                    ImageData => {
                        //println!("got {} bytes of image data", data.len())
                    }
                    _ => (),
                }
                //println!("{} {:?}", n, res)
            }
            Err(err) => {
                let _ = display_error(err);
                break;
            }
        }
    }
    if c.text {
        println!("Parsed tEXt chunks:");
        for text_chunk in &decoder.info().unwrap().uncompressed_latin1_text {
            println!("{:#?}", text_chunk);
        }

        println!("Parsed zTXt chunks:");
        for text_chunk in &decoder.info().unwrap().compressed_latin1_text {
            let mut cloned_text_chunk = text_chunk.clone();
            cloned_text_chunk.decompress_text()?;
            println!("{:#?}", cloned_text_chunk);
        }

        println!("Parsed iTXt chunks:");
        for text_chunk in &decoder.info().unwrap().utf8_text {
            let mut cloned_text_chunk = text_chunk.clone();
            cloned_text_chunk.decompress_text()?;
            println!("{:#?}", cloned_text_chunk);
        }
    }

    Ok(())
}

fn main() {
    let config = Config::parse();

    for file in &config.paths {
        let result = if let Some(glob) = file.to_str().filter(|n| n.contains('*')) {
            glob::glob(glob)
                .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
                .and_then(|mut glob| {
                    glob.try_for_each(|entry| {
                        entry
                            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
                            .and_then(|file| check_image(&config, file))
                    })
                })
        } else {
            check_image(&config, &file)
        };

        result.unwrap_or_else(|err| {
            println!("{}: {}", file.display(), err);
            std::process::exit(1)
        });
    }
}
