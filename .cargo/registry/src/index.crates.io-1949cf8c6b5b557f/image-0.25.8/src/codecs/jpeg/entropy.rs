/// Given an array containing the number of codes of each code length,
/// this function generates the huffman codes lengths and their respective
/// code lengths as specified by the JPEG spec.
const fn derive_codes_and_sizes(bits: &[u8; 16]) -> ([u8; 256], [u16; 256]) {
    let mut huffsize = [0u8; 256];
    let mut huffcode = [0u16; 256];

    let mut k = 0;

    // Annex C.2
    // Figure C.1
    // Generate table of individual code lengths
    let mut i = 0;
    while i < 16 {
        let mut j = 0;
        while j < bits[i as usize] {
            huffsize[k] = i + 1;
            k += 1;
            j += 1;
        }
        i += 1;
    }

    huffsize[k] = 0;

    // Annex C.2
    // Figure C.2
    // Generate table of huffman codes
    k = 0;
    let mut code = 0u16;
    let mut size = huffsize[0];

    while huffsize[k] != 0 {
        huffcode[k] = code;
        code += 1;
        k += 1;

        if huffsize[k] == size {
            continue;
        }

        // FIXME there is something wrong with this code
        let diff = huffsize[k].wrapping_sub(size);
        code = if diff < 16 { code << diff as usize } else { 0 };

        size = size.wrapping_add(diff);
    }

    (huffsize, huffcode)
}

pub(crate) const fn build_huff_lut_const(bits: &[u8; 16], huffval: &[u8]) -> [(u8, u16); 256] {
    let mut lut = [(17u8, 0u16); 256];
    let (huffsize, huffcode) = derive_codes_and_sizes(bits);

    let mut i = 0;
    while i < huffval.len() {
        lut[huffval[i] as usize] = (huffsize[i], huffcode[i]);
        i += 1;
    }

    lut
}
