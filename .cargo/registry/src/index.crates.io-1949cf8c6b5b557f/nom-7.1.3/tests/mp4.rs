#![allow(dead_code)]

use nom::{
  branch::alt,
  bytes::streaming::{tag, take},
  combinator::{map, map_res},
  error::ErrorKind,
  multi::many0,
  number::streaming::{be_f32, be_u16, be_u32, be_u64},
  Err, IResult, Needed,
};

use std::str;

fn mp4_box(input: &[u8]) -> IResult<&[u8], &[u8]> {
  match be_u32(input) {
    Ok((i, offset)) => {
      let sz: usize = offset as usize;
      if i.len() >= sz - 4 {
        Ok((&i[(sz - 4)..], &i[0..(sz - 4)]))
      } else {
        Err(Err::Incomplete(Needed::new(offset as usize + 4)))
      }
    }
    Err(e) => Err(e),
  }
}

#[cfg_attr(rustfmt, rustfmt_skip)]
#[derive(PartialEq,Eq,Debug)]
struct FileType<'a> {
  major_brand:         &'a str,
  major_brand_version: &'a [u8],
  compatible_brands:   Vec<&'a str>
}

#[cfg_attr(rustfmt, rustfmt_skip)]
#[allow(non_snake_case)]
#[derive(Debug,Clone)]
pub struct Mvhd32 {
  version_flags: u32, // actually:
  // version: u8,
  // flags: u24       // 3 bytes
  created_date:  u32,
  modified_date: u32,
  scale:         u32,
  duration:      u32,
  speed:         f32,
  volume:        u16, // actually a 2 bytes decimal
  /* 10 bytes reserved */
  scaleA:        f32,
  rotateB:       f32,
  angleU:        f32,
  rotateC:       f32,
  scaleD:        f32,
  angleV:        f32,
  positionX:     f32,
  positionY:     f32,
  scaleW:        f32,
  preview:       u64,
  poster:        u32,
  selection:     u64,
  current_time:  u32,
  track_id:      u32
}

#[cfg_attr(rustfmt, rustfmt_skip)]
#[allow(non_snake_case)]
#[derive(Debug,Clone)]
pub struct Mvhd64 {
  version_flags: u32, // actually:
  // version: u8,
  // flags: u24       // 3 bytes
  created_date:  u64,
  modified_date: u64,
  scale:         u32,
  duration:      u64,
  speed:         f32,
  volume:        u16, // actually a 2 bytes decimal
  /* 10 bytes reserved */
  scaleA:        f32,
  rotateB:       f32,
  angleU:        f32,
  rotateC:       f32,
  scaleD:        f32,
  angleV:        f32,
  positionX:     f32,
  positionY:     f32,
  scaleW:        f32,
  preview:       u64,
  poster:        u32,
  selection:     u64,
  current_time:  u32,
  track_id:      u32
}

#[cfg_attr(rustfmt, rustfmt_skip)]
fn mvhd32(i: &[u8]) -> IResult<&[u8], MvhdBox> {
  let (i, version_flags) = be_u32(i)?;
  let (i, created_date) =  be_u32(i)?;
  let (i, modified_date) = be_u32(i)?;
  let (i, scale) =         be_u32(i)?;
  let (i, duration) =      be_u32(i)?;
  let (i, speed) =         be_f32(i)?;
  let (i, volume) =        be_u16(i)?; // actually a 2 bytes decimal
  let (i, _) =             take(10_usize)(i)?;
  let (i, scale_a) =       be_f32(i)?;
  let (i, rotate_b) =      be_f32(i)?;
  let (i, angle_u) =       be_f32(i)?;
  let (i, rotate_c) =      be_f32(i)?;
  let (i, scale_d) =       be_f32(i)?;
  let (i, angle_v) =       be_f32(i)?;
  let (i, position_x) =    be_f32(i)?;
  let (i, position_y) =    be_f32(i)?;
  let (i, scale_w) =       be_f32(i)?;
  let (i, preview) =       be_u64(i)?;
  let (i, poster) =        be_u32(i)?;
  let (i, selection) =     be_u64(i)?;
  let (i, current_time) =  be_u32(i)?;
  let (i, track_id) =      be_u32(i)?;

  let mvhd_box = MvhdBox::M32(Mvhd32 {
    version_flags,
    created_date,
    modified_date,
    scale,
    duration,
    speed,
    volume,
    scaleA:    scale_a,
    rotateB:   rotate_b,
    angleU:    angle_u,
    rotateC:   rotate_c,
    scaleD:    scale_d,
    angleV:    angle_v,
    positionX: position_x,
    positionY: position_y,
    scaleW:    scale_w,
    preview,
    poster,
    selection,
    current_time,
    track_id,
  });

  Ok((i, mvhd_box))
}

#[cfg_attr(rustfmt, rustfmt_skip)]
fn mvhd64(i: &[u8]) -> IResult<&[u8], MvhdBox> {
  let (i, version_flags) = be_u32(i)?;
  let (i, created_date) =  be_u64(i)?;
  let (i, modified_date) = be_u64(i)?;
  let (i, scale) =         be_u32(i)?;
  let (i, duration) =      be_u64(i)?;
  let (i, speed) =         be_f32(i)?;
  let (i, volume) =        be_u16(i)?; // actually a 2 bytes decimal
  let (i, _) =             take(10_usize)(i)?;
  let (i, scale_a) =       be_f32(i)?;
  let (i, rotate_b) =      be_f32(i)?;
  let (i, angle_u) =       be_f32(i)?;
  let (i, rotate_c) =      be_f32(i)?;
  let (i, scale_d) =       be_f32(i)?;
  let (i, angle_v) =       be_f32(i)?;
  let (i, position_x) =    be_f32(i)?;
  let (i, position_y) =    be_f32(i)?;
  let (i, scale_w) =       be_f32(i)?;
  let (i, preview) =       be_u64(i)?;
  let (i, poster) =        be_u32(i)?;
  let (i, selection) =     be_u64(i)?;
  let (i, current_time) =  be_u32(i)?;
  let (i, track_id) =      be_u32(i)?;

  let mvhd_box = MvhdBox::M64(Mvhd64 {
    version_flags,
    created_date,
    modified_date,
    scale,
    duration,
    speed,
    volume,
    scaleA:    scale_a,
    rotateB:   rotate_b,
    angleU:    angle_u,
    rotateC:   rotate_c,
    scaleD:    scale_d,
    angleV:    angle_v,
    positionX: position_x,
    positionY: position_y,
    scaleW:    scale_w,
    preview,
    poster,
    selection,
    current_time,
    track_id,
  });

  Ok((i, mvhd_box))
}

#[derive(Debug, Clone)]
pub enum MvhdBox {
  M32(Mvhd32),
  M64(Mvhd64),
}

#[derive(Debug, Clone)]
pub enum MoovBox {
  Mdra,
  Dref,
  Cmov,
  Rmra,
  Iods,
  Mvhd(MvhdBox),
  Clip,
  Trak,
  Udta,
}

#[derive(Debug)]
enum MP4BoxType {
  Ftyp,
  Moov,
  Mdat,
  Free,
  Skip,
  Wide,
  Mdra,
  Dref,
  Cmov,
  Rmra,
  Iods,
  Mvhd,
  Clip,
  Trak,
  Udta,
  Unknown,
}

#[derive(Debug)]
struct MP4BoxHeader {
  length: u32,
  tag: MP4BoxType,
}

fn brand_name(input: &[u8]) -> IResult<&[u8], &str> {
  map_res(take(4_usize), str::from_utf8)(input)
}

fn filetype_parser(input: &[u8]) -> IResult<&[u8], FileType<'_>> {
  let (i, name) = brand_name(input)?;
  let (i, version) = take(4_usize)(i)?;
  let (i, brands) = many0(brand_name)(i)?;

  let ft = FileType {
    major_brand: name,
    major_brand_version: version,
    compatible_brands: brands,
  };
  Ok((i, ft))
}

fn mvhd_box(input: &[u8]) -> IResult<&[u8], MvhdBox> {
  let res = if input.len() < 100 {
    Err(Err::Incomplete(Needed::new(100)))
  } else if input.len() == 100 {
    mvhd32(input)
  } else if input.len() == 112 {
    mvhd64(input)
  } else {
    Err(Err::Error(nom::error_position!(input, ErrorKind::TooLarge)))
  };
  println!("res: {:?}", res);
  res
}

fn unknown_box_type(input: &[u8]) -> IResult<&[u8], MP4BoxType> {
  Ok((input, MP4BoxType::Unknown))
}

fn box_type(input: &[u8]) -> IResult<&[u8], MP4BoxType> {
  alt((
    map(tag("ftyp"), |_| MP4BoxType::Ftyp),
    map(tag("moov"), |_| MP4BoxType::Moov),
    map(tag("mdat"), |_| MP4BoxType::Mdat),
    map(tag("free"), |_| MP4BoxType::Free),
    map(tag("skip"), |_| MP4BoxType::Skip),
    map(tag("wide"), |_| MP4BoxType::Wide),
    unknown_box_type,
  ))(input)
}

// warning, an alt combinator with 9 branches containing a tag combinator
// can make the compilation very slow. Use functions as sub parsers,
// or split into multiple alt parsers if it gets slow
fn moov_type(input: &[u8]) -> IResult<&[u8], MP4BoxType> {
  alt((
    map(tag("mdra"), |_| MP4BoxType::Mdra),
    map(tag("dref"), |_| MP4BoxType::Dref),
    map(tag("cmov"), |_| MP4BoxType::Cmov),
    map(tag("rmra"), |_| MP4BoxType::Rmra),
    map(tag("iods"), |_| MP4BoxType::Iods),
    map(tag("mvhd"), |_| MP4BoxType::Mvhd),
    map(tag("clip"), |_| MP4BoxType::Clip),
    map(tag("trak"), |_| MP4BoxType::Trak),
    map(tag("udta"), |_| MP4BoxType::Udta),
  ))(input)
}

fn box_header(input: &[u8]) -> IResult<&[u8], MP4BoxHeader> {
  let (i, length) = be_u32(input)?;
  let (i, tag) = box_type(i)?;
  Ok((i, MP4BoxHeader { length, tag }))
}

fn moov_header(input: &[u8]) -> IResult<&[u8], MP4BoxHeader> {
  let (i, length) = be_u32(input)?;
  let (i, tag) = moov_type(i)?;
  Ok((i, MP4BoxHeader { length, tag }))
}
