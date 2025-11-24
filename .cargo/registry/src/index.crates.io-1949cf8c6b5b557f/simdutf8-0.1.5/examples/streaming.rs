#[allow(unused_imports)]
use std::io::{stdin, Read, Result};

#[cfg(all(
    feature = "public_imp",
    any(target_arch = "x86", target_arch = "x86_64")
))]
fn main() -> Result<()> {
    use simdutf8::basic::imp::Utf8Validator;

    unsafe {
        if !std::is_x86_feature_detected!("avx2") {
            panic!("This example only works with CPUs supporting AVX 2");
        }

        let mut validator = simdutf8::basic::imp::x86::avx2::Utf8ValidatorImp::new();
        let mut buf = vec![0; 8192];
        loop {
            let bytes_read = stdin().read(buf.as_mut())?;
            if bytes_read == 0 {
                break;
            }
            validator.update(&buf);
        }

        if validator.finalize().is_ok() {
            println!("Input is valid UTF-8");
        } else {
            println!("Input is not valid UTF-8");
        }
    }

    Ok(())
}

/// Dummy main. This example requires the crate feature `public_imp`.
#[cfg(not(all(
    feature = "public_imp",
    any(target_arch = "x86", target_arch = "x86_64")
)))]
fn main() {}
