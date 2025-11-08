//! Build script for OpenXR SDK integration
//!
//! Based on: https://github.com/KhronosGroup/OpenXR-SDK

use std::env;
use std::path::PathBuf;

fn main() {
    // Tell cargo to link OpenXR loader
    println!("cargo:rustc-link-lib=openxr_loader");

    // Search for OpenXR SDK
    let openxr_sdk_path = find_openxr_sdk();
    
    if let Some(sdk_path) = openxr_sdk_path {
        println!("cargo:rustc-link-search=native={}/lib", sdk_path.display());
        println!("cargo:include={}/include", sdk_path.display());
    } else {
        // Fallback: use system-installed OpenXR
        #[cfg(target_os = "windows")]
        {
            // Windows: OpenXR loader is typically in System32 or alongside executable
            println!("cargo:rustc-link-search=native=C:/Windows/System32");
        }
        
        #[cfg(target_os = "linux")]
        {
            // Linux: Use pkg-config if available
            if let Ok(libdir) = pkg_config::Config::new()
                .probe("openxr")
                .map(|lib| lib.link_paths[0].clone())
            {
                println!("cargo:rustc-link-search=native={}", libdir.display());
            }
        }
        
        #[cfg(target_os = "macos")]
        {
            // macOS: OpenXR may be in /usr/local/lib or via Homebrew
            println!("cargo:rustc-link-search=native=/usr/local/lib");
        }
    }

    // Generate bindings if openxr feature is enabled
    #[cfg(feature = "openxr")]
    {
        generate_openxr_bindings();
    }
}

/// Find OpenXR SDK installation
fn find_openxr_sdk() -> Option<PathBuf> {
    // Check environment variable
    if let Ok(path) = env::var("OPENXR_SDK_PATH") {
        let path = PathBuf::from(path);
        if path.join("include").join("openxr").join("openxr.h").exists() {
            return Some(path);
        }
    }

    // Check common installation paths
    let common_paths = [
        #[cfg(target_os = "windows")]
        "C:/OpenXR-SDK",
        #[cfg(target_os = "windows")]
        "C:/Program Files/OpenXR SDK",
        #[cfg(target_os = "linux")]
        "/usr/local",
        #[cfg(target_os = "linux")]
        "/opt/openxr",
        #[cfg(target_os = "macos")]
        "/usr/local",
        #[cfg(target_os = "macos")]
        "/opt/homebrew",
    ];

    for path_str in &common_paths {
        let path = PathBuf::from(path_str);
        if path.join("include").join("openxr").join("openxr.h").exists() {
            return Some(path);
        }
    }

    None
}

/// Generate OpenXR bindings using bindgen
#[cfg(feature = "openxr")]
fn generate_openxr_bindings() {
    use std::path::PathBuf;

    let openxr_sdk_path = find_openxr_sdk();
    
    let header_path = if let Some(sdk_path) = &openxr_sdk_path {
        sdk_path.join("include").join("openxr").join("openxr.h")
    } else {
        // Try to find system header
        #[cfg(target_os = "linux")]
        {
            PathBuf::from("/usr/include/openxr/openxr.h")
        }
        #[cfg(target_os = "windows")]
        {
            PathBuf::from("C:/OpenXR-SDK/include/openxr/openxr.h")
        }
        #[cfg(target_os = "macos")]
        {
            PathBuf::from("/usr/local/include/openxr/openxr.h")
        }
    };

    if !header_path.exists() {
        println!("cargo:warning=OpenXR header not found at {:?}", header_path);
        println!("cargo:warning=Set OPENXR_SDK_PATH environment variable or install OpenXR SDK");
        return;
    }

    let bindings = bindgen::Builder::default()
        .header(header_path.to_string_lossy())
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .allowlist_type("Xr.*")
        .allowlist_function("xr.*")
        .allowlist_var("XR_.*")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("openxr_bindings.rs"))
        .expect("Couldn't write bindings!");
}











