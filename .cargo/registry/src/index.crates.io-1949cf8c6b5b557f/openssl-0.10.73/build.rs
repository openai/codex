#![allow(
    clippy::inconsistent_digit_grouping,
    clippy::uninlined_format_args,
    clippy::unusual_byte_groupings
)]

use std::env;

fn main() {
    println!("cargo:rustc-check-cfg=cfg(osslconf, values(\"OPENSSL_NO_OCB\", \"OPENSSL_NO_SM4\", \"OPENSSL_NO_SEED\", \"OPENSSL_NO_CHACHA\", \"OPENSSL_NO_CAST\", \"OPENSSL_NO_IDEA\", \"OPENSSL_NO_CAMELLIA\", \"OPENSSL_NO_RC4\", \"OPENSSL_NO_BF\", \"OPENSSL_NO_PSK\", \"OPENSSL_NO_DEPRECATED_3_0\", \"OPENSSL_NO_SCRYPT\", \"OPENSSL_NO_SM3\", \"OPENSSL_NO_RMD160\", \"OPENSSL_NO_EC2M\", \"OPENSSL_NO_OCSP\", \"OPENSSL_NO_CMS\", \"OPENSSL_NO_EC\", \"OPENSSL_NO_ARGON2\", \"OPENSSL_NO_RC2\"))");

    println!("cargo:rustc-check-cfg=cfg(libressl)");
    println!("cargo:rustc-check-cfg=cfg(boringssl)");
    println!("cargo:rustc-check-cfg=cfg(awslc)");

    println!("cargo:rustc-check-cfg=cfg(libressl250)");
    println!("cargo:rustc-check-cfg=cfg(libressl251)");
    println!("cargo:rustc-check-cfg=cfg(libressl261)");
    println!("cargo:rustc-check-cfg=cfg(libressl270)");
    println!("cargo:rustc-check-cfg=cfg(libressl271)");
    println!("cargo:rustc-check-cfg=cfg(libressl273)");
    println!("cargo:rustc-check-cfg=cfg(libressl280)");
    println!("cargo:rustc-check-cfg=cfg(libressl291)");
    println!("cargo:rustc-check-cfg=cfg(libressl310)");
    println!("cargo:rustc-check-cfg=cfg(libressl321)");
    println!("cargo:rustc-check-cfg=cfg(libressl332)");
    println!("cargo:rustc-check-cfg=cfg(libressl340)");
    println!("cargo:rustc-check-cfg=cfg(libressl350)");
    println!("cargo:rustc-check-cfg=cfg(libressl360)");
    println!("cargo:rustc-check-cfg=cfg(libressl361)");
    println!("cargo:rustc-check-cfg=cfg(libressl370)");
    println!("cargo:rustc-check-cfg=cfg(libressl380)");
    println!("cargo:rustc-check-cfg=cfg(libressl382)");
    println!("cargo:rustc-check-cfg=cfg(libressl390)");
    println!("cargo:rustc-check-cfg=cfg(libressl400)");
    println!("cargo:rustc-check-cfg=cfg(libressl410)");

    println!("cargo:rustc-check-cfg=cfg(ossl101)");
    println!("cargo:rustc-check-cfg=cfg(ossl102)");
    println!("cargo:rustc-check-cfg=cfg(ossl110)");
    println!("cargo:rustc-check-cfg=cfg(ossl110g)");
    println!("cargo:rustc-check-cfg=cfg(ossl110h)");
    println!("cargo:rustc-check-cfg=cfg(ossl111)");
    println!("cargo:rustc-check-cfg=cfg(ossl111d)");
    println!("cargo:rustc-check-cfg=cfg(ossl300)");
    println!("cargo:rustc-check-cfg=cfg(ossl310)");
    println!("cargo:rustc-check-cfg=cfg(ossl320)");
    println!("cargo:rustc-check-cfg=cfg(ossl330)");

    if env::var("DEP_OPENSSL_LIBRESSL").is_ok() {
        println!("cargo:rustc-cfg=libressl");
    }

    if env::var("DEP_OPENSSL_BORINGSSL").is_ok() {
        println!("cargo:rustc-cfg=boringssl");
    }

    if env::var("DEP_OPENSSL_AWSLC").is_ok() {
        println!("cargo:rustc-cfg=awslc");
    }

    if let Ok(v) = env::var("DEP_OPENSSL_LIBRESSL_VERSION_NUMBER") {
        let version = u64::from_str_radix(&v, 16).unwrap();

        if version >= 0x2_05_00_00_0 {
            println!("cargo:rustc-cfg=libressl250");
        }
        if version >= 0x2_05_01_00_0 {
            println!("cargo:rustc-cfg=libressl251");
        }
        if version >= 0x2_06_01_00_0 {
            println!("cargo:rustc-cfg=libressl261");
        }
        if version >= 0x2_07_00_00_0 {
            println!("cargo:rustc-cfg=libressl270");
        }
        if version >= 0x2_07_01_00_0 {
            println!("cargo:rustc-cfg=libressl271");
        }
        if version >= 0x2_07_03_00_0 {
            println!("cargo:rustc-cfg=libressl273");
        }
        if version >= 0x2_08_00_00_0 {
            println!("cargo:rustc-cfg=libressl280");
        }
        if version >= 0x2_09_01_00_0 {
            println!("cargo:rustc-cfg=libressl291");
        }
        if version >= 0x3_01_00_00_0 {
            println!("cargo:rustc-cfg=libressl310");
        }
        if version >= 0x3_02_01_00_0 {
            println!("cargo:rustc-cfg=libressl321");
        }
        if version >= 0x3_03_02_00_0 {
            println!("cargo:rustc-cfg=libressl332");
        }
        if version >= 0x3_04_00_00_0 {
            println!("cargo:rustc-cfg=libressl340");
        }
        if version >= 0x3_05_00_00_0 {
            println!("cargo:rustc-cfg=libressl350");
        }
        if version >= 0x3_06_00_00_0 {
            println!("cargo:rustc-cfg=libressl360");
        }
        if version >= 0x3_06_01_00_0 {
            println!("cargo:rustc-cfg=libressl361");
        }
        if version >= 0x3_07_00_00_0 {
            println!("cargo:rustc-cfg=libressl370");
        }
        if version >= 0x3_08_00_00_0 {
            println!("cargo:rustc-cfg=libressl380");
        }
        if version >= 0x3_08_02_00_0 {
            println!("cargo:rustc-cfg=libressl382");
        }
        if version >= 0x3_09_00_00_0 {
            println!("cargo:rustc-cfg=libressl390");
        }
        if version >= 0x4_00_00_00_0 {
            println!("cargo:rustc-cfg=libressl400");
        }
        if version >= 0x4_01_00_00_0 {
            println!("cargo:rustc-cfg=libressl410");
        }
    }

    if let Ok(vars) = env::var("DEP_OPENSSL_CONF") {
        for var in vars.split(',') {
            println!("cargo:rustc-cfg=osslconf=\"{}\"", var);
        }
    }

    if let Ok(version) = env::var("DEP_OPENSSL_VERSION_NUMBER") {
        let version = u64::from_str_radix(&version, 16).unwrap();

        if version >= 0x1_00_01_00_0 {
            println!("cargo:rustc-cfg=ossl101");
        }
        if version >= 0x1_00_02_00_0 {
            println!("cargo:rustc-cfg=ossl102");
        }
        if version >= 0x1_01_00_00_0 {
            println!("cargo:rustc-cfg=ossl110");
        }
        if version >= 0x1_01_00_07_0 {
            println!("cargo:rustc-cfg=ossl110g");
        }
        if version >= 0x1_01_00_08_0 {
            println!("cargo:rustc-cfg=ossl110h");
        }
        if version >= 0x1_01_01_00_0 {
            println!("cargo:rustc-cfg=ossl111");
        }
        if version >= 0x1_01_01_04_0 {
            println!("cargo:rustc-cfg=ossl111d");
        }
        if version >= 0x3_00_00_00_0 {
            println!("cargo:rustc-cfg=ossl300");
        }
        if version >= 0x3_01_00_00_0 {
            println!("cargo:rustc-cfg=ossl310");
        }
        if version >= 0x3_02_00_00_0 {
            println!("cargo:rustc-cfg=ossl320");
        }
        if version >= 0x3_03_00_00_0 {
            println!("cargo:rustc-cfg=ossl330");
        }
    }
}
