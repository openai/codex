#[derive(Copy, Clone)]
enum Value {
    Short(u16),
    Long(u32),
    Rational(u32, u32),
    DataOffset,
}

pub fn wrap(data: &[u8], width: u32, height: u32) -> Vec<u8> {
    use Value::*;
    let header_data = [
        (256, Long(width)), // ImageWidth
        (257, Long(height)), // ImageLength
        (259, Short(4)), // Compression
        (262, Short(0)), // PhotometricInterpretation
        (273, DataOffset), // StripOffsets
        (274, Short(1)), // Orientation
        (278, Long(height)), // RowsPerStrip
        (279, Long(data.len() as u32)), // StripByteCounts
        (282, Rational(200, 1)), // XResolution
        (283, Rational(200, 1)), // YResolution
        (296, Short(2)), // ResolutionUnit
    ];
    let rat_data_len = 2 * 8; // number of rationals * 8
    let ifd_end = 
        4 + // magic
        4 + // IFD offset
        2 + // IFD entry count
        12 * header_data.len() + // IFD enties
        4; // null pointer at end of IFD
    let header_size = ifd_end + rat_data_len;
    
    let mut out = Vec::with_capacity(header_size + data.len());

    out.extend_from_slice(&[73, 73, 42, 0]);
    let ifd_offset: u32 = 8;
    out.extend_from_slice(&ifd_offset.to_le_bytes());

    out.extend_from_slice(&u16::to_le_bytes(header_data.len() as u16));

    let mut num_rat = 0;
    for &(tag, val) in header_data.iter() {
        let (typ_num, val) = match val {
            Short(n) => (3, n as u32),
            Long(n) => (4, n),
            Rational(_, _) => {
                let o = ifd_end + 8 * num_rat;
                num_rat += 1;
                (5, o as u32)
            }
            DataOffset => (4, header_size as u32)
        };
        let count = 1;
        out.extend_from_slice(&u16::to_le_bytes(tag));
        out.extend_from_slice(&u16::to_le_bytes(typ_num));
        out.extend_from_slice(&u32::to_le_bytes(count));
        out.extend_from_slice(&u32::to_le_bytes(val));
    }
    // NULL at IFD end
    out.extend_from_slice(&[0; 4]);

    // write additional data
    for &(_, val) in header_data.iter() {
        if let Value::Rational(nom, denom) = val {
            out.extend_from_slice(&nom.to_le_bytes());
            out.extend_from_slice(&denom.to_le_bytes());
        }
    }

    assert_eq!(out.len(), header_size);
    out.extend_from_slice(data);
    out
}
