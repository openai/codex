#[cfg(feature = "std")]
use object::{Object, ObjectSection as _};

// Test that we can read compressed sections in Mach-O files as produced
// by the Go compiler.
#[cfg(feature = "std")]
#[test]
fn test_go_macho() {
    let macho_testfiles = std::path::Path::new("testfiles/macho");

    // Section names we expect to find, whether they should be
    // compressed, and the actual name of the section in the file.
    const EXPECTED: &[(&str, bool, &str)] = &[
        (".debug_abbrev", true, "__zdebug_abbrev"),
        (".debug_gdb_scripts", false, "__debug_gdb_scri"),
        (".debug_ranges", true, "__zdebug_ranges"),
        ("__data", false, "__data"),
    ];

    for file in &["go-aarch64", "go-x86_64"] {
        let path = macho_testfiles.join(file);
        let file = std::fs::File::open(path).unwrap();
        let reader = object::read::ReadCache::new(file);
        let object = object::read::File::parse(&reader).unwrap();
        for &(name, compressed, actual_name) in EXPECTED {
            let section = object.section_by_name(name).unwrap();
            assert_eq!(section.name(), Ok(actual_name));
            let compressed_file_range = section.compressed_file_range().unwrap();
            let size = section.size();
            if compressed {
                assert_eq!(
                    compressed_file_range.format,
                    object::CompressionFormat::Zlib
                );
                assert_eq!(compressed_file_range.compressed_size, size - 12);
                assert!(
                    compressed_file_range.uncompressed_size > compressed_file_range.compressed_size,
                    "decompressed size is greater than compressed size"
                );
            } else {
                assert_eq!(
                    compressed_file_range.format,
                    object::CompressionFormat::None
                );
                assert_eq!(compressed_file_range.compressed_size, size);
            }
        }
    }
}
