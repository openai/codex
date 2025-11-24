//! Rust parser of ZoneInfoDb(`tzdata`) on Android and OpenHarmony
//!
//! Ported from: https://android.googlesource.com/platform/prebuilts/fullsdk/sources/+/refs/heads/androidx-appcompat-release/android-34/com/android/i18n/timezone/ZoneInfoDb.java
use std::{
    ffi::CStr,
    fmt::Debug,
    fs::File,
    io::{Error, ErrorKind, Read, Result, Seek, SeekFrom},
};

/// Get timezone data from the `tzdata` file of HarmonyOS NEXT.
#[cfg(target_env = "ohos")]
pub(crate) fn for_zone(tz_string: &str) -> Result<Option<Vec<u8>>> {
    let mut file = File::open("/system/etc/zoneinfo/tzdata")?;
    find_tz_data::<OHOS_ENTRY_LEN>(&mut file, tz_string.as_bytes())
}

/// Get timezone data from the `tzdata` file of Android.
#[cfg(target_os = "android")]
pub(crate) fn for_zone(tz_string: &str) -> Result<Option<Vec<u8>>> {
    let mut file = open_android_tz_data_file()?;
    find_tz_data::<ANDROID_ENTRY_LEN>(&mut file, tz_string.as_bytes())
}

/// Open the `tzdata` file of Android from the environment variables.
#[cfg(target_os = "android")]
fn open_android_tz_data_file() -> Result<File> {
    for (env_var, path) in
        [("ANDROID_DATA", "/misc/zoneinfo"), ("ANDROID_ROOT", "/usr/share/zoneinfo")]
    {
        if let Ok(env_value) = std::env::var(env_var) {
            if let Ok(file) = File::open(format!("{}{}/tzdata", env_value, path)) {
                return Ok(file);
            }
        }
    }
    Err(Error::from(ErrorKind::NotFound))
}

/// Get timezone data from the `tzdata` file reader
#[cfg(any(test, target_env = "ohos", target_os = "android"))]
fn find_tz_data<const ENTRY_LEN: usize>(
    mut reader: impl Read + Seek,
    tz_name: &[u8],
) -> Result<Option<Vec<u8>>> {
    let header = TzDataHeader::new(&mut reader)?;
    let index = TzDataIndexes::new::<ENTRY_LEN>(&mut reader, &header)?;
    Ok(if let Some(entry) = index.find_timezone(tz_name) {
        Some(index.find_tzdata(reader, &header, entry)?)
    } else {
        None
    })
}

/// Header of the `tzdata` file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TzDataHeader {
    version: [u8; 5],
    index_offset: u32,
    data_offset: u32,
    zonetab_offset: u32,
}

impl TzDataHeader {
    /// Parse the header of the `tzdata` file.
    fn new(mut data: impl Read) -> Result<Self> {
        let version = {
            let mut magic = [0; TZDATA_VERSION_LEN];
            data.read_exact(&mut magic)?;
            if !magic.starts_with(b"tzdata") || magic[TZDATA_VERSION_LEN - 1] != 0 {
                return Err(Error::new(ErrorKind::Other, "invalid tzdata header magic"));
            }
            let mut version = [0; 5];
            version.copy_from_slice(&magic[6..11]);
            version
        };

        let mut offset = [0; 4];
        data.read_exact(&mut offset)?;
        let index_offset = u32::from_be_bytes(offset);
        data.read_exact(&mut offset)?;
        let data_offset = u32::from_be_bytes(offset);
        data.read_exact(&mut offset)?;
        let zonetab_offset = u32::from_be_bytes(offset);

        Ok(Self { version, index_offset, data_offset, zonetab_offset })
    }
}

/// Indexes of the `tzdata` file.
struct TzDataIndexes {
    indexes: Vec<TzDataIndex>,
}

impl TzDataIndexes {
    /// Create a new `TzDataIndexes` from the `tzdata` file reader.
    fn new<const ENTRY_LEN: usize>(mut reader: impl Read, header: &TzDataHeader) -> Result<Self> {
        let mut buf = vec![0; header.data_offset.saturating_sub(header.index_offset) as usize];
        reader.read_exact(&mut buf)?;
        // replace chunks with array_chunks when it's stable
        Ok(TzDataIndexes {
            indexes: buf
                .chunks(ENTRY_LEN)
                .filter_map(|chunk| {
                    from_bytes_until_nul(&chunk[..TZ_NAME_LEN]).map(|name| {
                        let name = name.to_bytes().to_vec().into_boxed_slice();
                        let offset = u32::from_be_bytes(
                            chunk[TZ_NAME_LEN..TZ_NAME_LEN + 4].try_into().unwrap(),
                        );
                        let length = u32::from_be_bytes(
                            chunk[TZ_NAME_LEN + 4..TZ_NAME_LEN + 8].try_into().unwrap(),
                        );
                        TzDataIndex { name, offset, length }
                    })
                })
                .collect(),
        })
    }

    /// Find a timezone by name.
    fn find_timezone(&self, timezone: &[u8]) -> Option<&TzDataIndex> {
        // timezones in tzdata are sorted by name.
        self.indexes.binary_search_by_key(&timezone, |x| &x.name).map(|x| &self.indexes[x]).ok()
    }

    /// Retrieve a chunk of timezone data by the index.
    fn find_tzdata(
        &self,
        mut reader: impl Read + Seek,
        header: &TzDataHeader,
        index: &TzDataIndex,
    ) -> Result<Vec<u8>> {
        reader.seek(SeekFrom::Start(index.offset as u64 + header.data_offset as u64))?;
        let mut buffer = vec![0; index.length as usize];
        reader.read_exact(&mut buffer)?;
        Ok(buffer)
    }
}

/// Index entry of the `tzdata` file.
struct TzDataIndex {
    name: Box<[u8]>,
    offset: u32,
    length: u32,
}

/// TODO: Change this `CStr::from_bytes_until_nul` once MSRV was bumped above 1.72.0
fn from_bytes_until_nul(bytes: &[u8]) -> Option<&CStr> {
    let nul_pos = bytes.iter().position(|&b| b == 0)?;
    // SAFETY:
    // 1. nul_pos + 1 <= bytes.len()
    // 2. We know there is a nul byte at nul_pos, so this slice (ending at the nul byte) is a well-formed C string.
    Some(unsafe { CStr::from_bytes_with_nul_unchecked(&bytes[..=nul_pos]) })
}

/// Ohos tzdata index entry size: `name + offset + length`
#[cfg(any(test, target_env = "ohos"))]
const OHOS_ENTRY_LEN: usize = TZ_NAME_LEN + 2 * size_of::<u32>();
/// Android tzdata index entry size: `name + offset + length + raw_utc_offset(legacy)`:
/// [reference](https://android.googlesource.com/platform/prebuilts/fullsdk/sources/+/refs/heads/androidx-appcompat-release/android-34/com/android/i18n/timezone/ZoneInfoDb.java#271)
#[cfg(any(test, target_os = "android"))]
const ANDROID_ENTRY_LEN: usize = TZ_NAME_LEN + 3 * size_of::<u32>();
/// The database reserves 40 bytes for each id.
const TZ_NAME_LEN: usize = 40;
/// Size of the version string in the header of `tzdata` file.
/// e.g. `tzdata2024b\0`
const TZDATA_VERSION_LEN: usize = 12;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ohos_tzdata_header_and_index() {
        let file = File::open("./tests/ohos/tzdata").unwrap();
        let header = TzDataHeader::new(&file).unwrap();
        assert_eq!(header.version, *b"2024a");
        assert_eq!(header.index_offset, 24);
        assert_eq!(header.data_offset, 21240);
        assert_eq!(header.zonetab_offset, 272428);

        let iter = TzDataIndexes::new::<OHOS_ENTRY_LEN>(&file, &header).unwrap();
        assert_eq!(iter.indexes.len(), 442);
        assert!(iter.find_timezone(b"Asia/Shanghai").is_some());
        assert!(iter.find_timezone(b"Pacific/Noumea").is_some());
    }

    #[test]
    fn test_ohos_tzdata_loading() {
        let file = File::open("./tests/ohos/tzdata").unwrap();
        let header = TzDataHeader::new(&file).unwrap();
        let iter = TzDataIndexes::new::<OHOS_ENTRY_LEN>(&file, &header).unwrap();
        let timezone = iter.find_timezone(b"Asia/Shanghai").unwrap();
        let tzdata = iter.find_tzdata(&file, &header, timezone).unwrap();
        assert_eq!(tzdata.len(), 393);
    }

    #[test]
    fn test_invalid_tzdata_header() {
        TzDataHeader::new(&b"tzdaaa2024aaaaaaaaaaaaaaa\0"[..]).unwrap_err();
    }

    #[test]
    fn test_android_tzdata_header_and_index() {
        let file = File::open("./tests/android/tzdata").unwrap();
        let header = TzDataHeader::new(&file).unwrap();
        assert_eq!(header.version, *b"2021a");
        assert_eq!(header.index_offset, 24);
        assert_eq!(header.data_offset, 30860);
        assert_eq!(header.zonetab_offset, 491837);

        let iter = TzDataIndexes::new::<ANDROID_ENTRY_LEN>(&file, &header).unwrap();
        assert_eq!(iter.indexes.len(), 593);
        assert!(iter.find_timezone(b"Asia/Shanghai").is_some());
        assert!(iter.find_timezone(b"Pacific/Noumea").is_some());
    }

    #[test]
    fn test_android_tzdata_loading() {
        let file = File::open("./tests/android/tzdata").unwrap();
        let header = TzDataHeader::new(&file).unwrap();
        let iter = TzDataIndexes::new::<ANDROID_ENTRY_LEN>(&file, &header).unwrap();
        let timezone = iter.find_timezone(b"Asia/Shanghai").unwrap();
        let tzdata = iter.find_tzdata(&file, &header, timezone).unwrap();
        assert_eq!(tzdata.len(), 573);
    }

    #[test]
    fn test_ohos_tzdata_find() {
        let file = File::open("./tests/ohos/tzdata").unwrap();
        let tzdata = find_tz_data::<OHOS_ENTRY_LEN>(file, b"Asia/Shanghai").unwrap().unwrap();
        assert_eq!(tzdata.len(), 393);
    }

    #[test]
    fn test_ohos_tzdata_find_missing() {
        let file = File::open("./tests/ohos/tzdata").unwrap();
        assert!(find_tz_data::<OHOS_ENTRY_LEN>(file, b"Asia/Sjasdfai").unwrap().is_none());
    }

    #[test]
    fn test_android_tzdata_find() {
        let file = File::open("./tests/android/tzdata").unwrap();
        let tzdata = find_tz_data::<ANDROID_ENTRY_LEN>(file, b"Asia/Shanghai").unwrap().unwrap();
        assert_eq!(tzdata.len(), 573);
    }

    #[test]
    fn test_android_tzdata_find_missing() {
        let file = File::open("./tests/android/tzdata").unwrap();
        assert!(find_tz_data::<ANDROID_ENTRY_LEN>(file, b"Asia/S000000i").unwrap().is_none());
    }

    #[cfg(target_env = "ohos")]
    #[test]
    fn test_ohos_machine_tz_data_loading() {
        let tzdata = for_zone(b"Asia/Shanghai").unwrap().unwrap();
        assert!(!tzdata.is_empty());
    }

    #[cfg(target_os = "android")]
    #[test]
    fn test_android_machine_tz_data_loading() {
        let tzdata = for_zone(b"Asia/Shanghai").unwrap().unwrap();
        assert!(!tzdata.is_empty());
    }
}
