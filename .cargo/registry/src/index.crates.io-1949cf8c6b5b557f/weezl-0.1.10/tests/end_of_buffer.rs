use weezl::{decode, BitOrder};

#[test]
fn stop_after_end_of_buffer() {
    let inp = vec![0x00u8, 0x01, 0x02, 0xff];
    let mut decoder = decode::Configuration::new(BitOrder::Lsb, 7)
        .with_yield_on_full_buffer(true)
        .build();
    let mut out = vec![0u8, 0u8, 0u8];
    let status = decoder.decode_bytes(&inp, &mut out).status;
    assert!(status.is_ok(), "{:?} {:?}", status, out);
}
