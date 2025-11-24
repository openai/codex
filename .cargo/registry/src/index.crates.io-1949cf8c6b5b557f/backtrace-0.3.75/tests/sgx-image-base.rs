#![cfg(all(target_env = "sgx", target_vendor = "fortanix"))]
#![feature(sgx_platform)]

#[cfg(feature = "std")]
#[test]
fn sgx_image_base_with_std() {
    use backtrace::trace;

    let image_base = std::os::fortanix_sgx::mem::image_base();

    let mut frame_ips = Vec::new();
    trace(|frame| {
        frame_ips.push(frame.ip());
        true
    });

    assert!(frame_ips.len() > 0);
    for ip in frame_ips {
        let ip: u64 = ip as _;
        assert!(ip < image_base);
    }
}

#[cfg(not(feature = "std"))]
#[test]
fn sgx_image_base_no_std() {
    use backtrace::trace_unsynchronized;

    fn guess_image_base() -> u64 {
        let mut top_frame_ip = None;
        unsafe {
            trace_unsynchronized(|frame| {
                top_frame_ip = Some(frame.ip());
                false
            });
        }
        top_frame_ip.unwrap() as u64 & 0xFFFFFF000000
    }

    let image_base = guess_image_base();
    backtrace::set_image_base(image_base as _);

    let mut frame_ips = Vec::new();
    unsafe {
        trace_unsynchronized(|frame| {
            frame_ips.push(frame.ip());
            true
        });
    }

    assert!(frame_ips.len() > 0);
    for ip in frame_ips {
        let ip: u64 = ip as _;
        assert!(ip < image_base);
    }
}
