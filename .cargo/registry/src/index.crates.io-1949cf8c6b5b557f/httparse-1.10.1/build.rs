use std::env;
use std::ffi::OsString;
use std::process::Command;

fn main() {
    // We check rustc version to enable features beyond MSRV, such as:
    // - 1.59 => neon_intrinsics
    let rustc = env::var_os("RUSTC").unwrap_or(OsString::from("rustc"));
    let output = Command::new(rustc)
        .arg("--version")
        .output()
        .expect("failed to check 'rustc --version'")
        .stdout;

    let raw_version = String::from_utf8(output)
        .expect("rustc version output should be utf-8");
    
    let version = match Version::parse(&raw_version) {
        Ok(version) => version,
        Err(err) => {
            println!("cargo:warning=failed to parse `rustc --version`: {}", err);
            return;
        }
    };

    enable_new_features(version);
}

fn enable_new_features(version: Version) {
    enable_simd(version);
}

fn enable_simd(version: Version) {
    if env::var_os("CARGO_FEATURE_STD").is_none() {
        println!("cargo:warning=building for no_std disables httparse SIMD");
        return;
    }
    if env::var_os("CARGO_CFG_MIRI").is_some() {
        println!("cargo:warning=building for Miri disables httparse SIMD");
        return;
    }

    let env_disable = "CARGO_CFG_HTTPARSE_DISABLE_SIMD";
    if var_is(env_disable, "1") {
        println!("cargo:warning=detected {} environment variable, disabling SIMD", env_disable);
        return;
    }

    // 1.59.0 is the first version to support neon_intrinsics
    if version >= Version(1, 59, 0) {
        println!("cargo:rustc-cfg=httparse_simd_neon_intrinsics");
    }

    println!("cargo:rustc-cfg=httparse_simd");

    // cfg(target_feature) isn't stable yet, but CARGO_CFG_TARGET_FEATURE has
    // a list... We aren't doing anything unsafe, since the is_x86_feature_detected
    // macro still checks in the actual lib, BUT!
    //
    // By peeking at the list here, we can change up slightly how we do feature
    // detection in the lib. If our features aren't in the feature list, we
    // stick with a cached runtime detection strategy.
    //
    // But if the features *are* in the list, we benefit from removing our cache,
    // since the compiler will eliminate several branches with its internal
    // cfg(target_feature) usage.


    let env_runtime_only = "CARGO_CFG_HTTPARSE_DISABLE_SIMD_COMPILETIME";
    if var_is(env_runtime_only, "1") {
        println!("cargo:warning=detected {} environment variable, using runtime SIMD detection only", env_runtime_only);
        return;
    }
    let feature_list = match env::var_os("CARGO_CFG_TARGET_FEATURE") {
        Some(var) => match var.into_string() {
            Ok(s) => s,
            Err(_) => {
                println!("cargo:warning=CARGO_CFG_TARGET_FEATURE was not valid utf-8");
                return;
            },
        },
        None => {
            println!("cargo:warning=CARGO_CFG_TARGET_FEATURE was not set");
            return
        },
    };
    
    let features = feature_list.split(',').map(|s| s.trim());
    if features.clone().any(|f| f == "sse4.2") {
        println!("cargo:rustc-cfg=httparse_simd_target_feature_sse42");
    }
    if features.clone().any(|f| f == "avx2") {
        println!("cargo:rustc-cfg=httparse_simd_target_feature_avx2");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
struct Version (u32, u32, u32);

impl Version {
    fn parse(s: &str) -> Result<Version, String> {
        if !s.starts_with("rustc ") {
            return Err(format!("unrecognized version string: {}", s));
        }
        let s = s.trim_start_matches("rustc ");
        
        let mut iter = s
            .split('.')
            .take(3)
            .map(|s| match s.find(|c: char| !c.is_ascii_digit()) {
                Some(end) => &s[..end],
                None => s,
            })
            .map(|s| s.parse::<u32>().map_err(|e| e.to_string()));
    
        if iter.clone().count() != 3 {
            return Err(format!("not enough version parts: {:?}", s));
        }
        
        let major = iter.next().unwrap()?;
        let minor = iter.next().unwrap()?;
        let patch = iter.next().unwrap()?;

        Ok(Version(major, minor, patch))
    }
}

fn var_is(key: &str, val: &str) -> bool {
    match env::var(key) {
        Ok(v) => v == val,
        Err(_) => false,
    }
}
