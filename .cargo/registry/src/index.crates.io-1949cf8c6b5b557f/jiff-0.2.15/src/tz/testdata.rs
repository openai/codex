#[cfg(not(miri))]
use crate::tz::tzif::TzifOwned;

/// A concatenated list of TZif data with a header and an index block.
///
/// This was exactracted from an Android emulator file system via `adb`.
pub(crate) static ANDROID_CONCATENATED_TZIF: &'static [u8] =
    include_bytes!("testdata/android/tzdata");

/// A list of all TZif files in our testdata directory.
///
/// Feel free to add more if there are other "interesting" cases. Note that
/// some tests iterate over this slice to check each entry. So adding a new
/// test file here might require updating some tests.
///
/// # Revisions
///
/// * 2024-03-27: Initial set pulled from my local copy of `tzdata 2024a`.
/// * 2024-07-05: Added `UTC`.
/// * 2024-11-30: Added special Sydney time zone from RHEL8.
pub(crate) static TZIF_TEST_FILES: &[TzifTestFile] = &[
    TzifTestFile {
        name: "America/New_York",
        data: include_bytes!("testdata/america-new-york.tzif"),
    },
    TzifTestFile {
        name: "America/Sitka",
        data: include_bytes!("testdata/america-sitka.tzif"),
    },
    TzifTestFile {
        name: "America/St_Johns",
        data: include_bytes!("testdata/america-st-johns.tzif"),
    },
    TzifTestFile {
        name: "right/America/New_York",
        data: include_bytes!("testdata/right-america-new-york.tzif"),
    },
    TzifTestFile {
        name: "Antarctica/Troll",
        data: include_bytes!("testdata/antarctica-troll.tzif"),
    },
    TzifTestFile {
        name: "Australia/Tasmania",
        data: include_bytes!("testdata/australia-tasmania.tzif"),
    },
    TzifTestFile {
        name: "Europe/Dublin",
        data: include_bytes!("testdata/europe-dublin.tzif"),
    },
    TzifTestFile {
        name: "Pacific/Honolulu",
        data: include_bytes!("testdata/pacific-honolulu.tzif"),
    },
    // This TZif file comes from a RHEL8 installation[1]. Apparently RHEL
    // added a special first transition far back in the past. It is probably
    // similar in purpose to what Jiff does internally, which is to add a
    // dummy "minimum" transition. We test this case here because RHEL's dummy
    // transition falls outside of Jiff's supported `Timestamp` range, which
    // did result in an error at parse time. But we should be able to parse it,
    // so we test it here.
    //
    // [1]: https://github.com/BurntSushi/jiff/issues/163
    TzifTestFile {
        name: "Australia/Sydney/RHEL8",
        data: include_bytes!("testdata/australia-sydney-rhel8.tzif"),
    },
    // I added this to test finding previous time zone transitions in a time
    // zone that had somewhat recently eliminated DST. The bug was that Jiff
    // wasn't reporting *any* previous time zone transitions.
    TzifTestFile {
        name: "America/Sao_Paulo",
        data: include_bytes!("testdata/america-sao-paulo.tzif"),
    },
    // Another test file I added for a region that eliminated DST and thus
    // has a "final" time zone transition.
    TzifTestFile {
        name: "America/Boa_Vista",
        data: include_bytes!("testdata/america-boa-vista.tzif"),
    },
    TzifTestFile { name: "UTC", data: include_bytes!("testdata/utc.tzif") },
];

/// A single TZif datum.
///
/// It contains the name of the time zone and the raw bytes of the
/// corresponding TZif file.
#[derive(Clone, Copy)]
pub(crate) struct TzifTestFile {
    pub(crate) name: &'static str,
    pub(crate) data: &'static [u8],
}

impl TzifTestFile {
    /// Look up the TZif test file for the given time zone name.
    ///
    /// If one doesn't exist, then this panics and fails the current
    /// test.
    pub(crate) fn get(name: &str) -> TzifTestFile {
        for &tzif_file in TZIF_TEST_FILES {
            if tzif_file.name == name {
                return tzif_file;
            }
        }
        panic!("could not find TZif test file for {name:?}")
    }

    /// Parse this test TZif data into a structured representation.
    #[cfg(not(miri))]
    pub(crate) fn parse(self) -> TzifOwned {
        use alloc::string::ToString;

        let name = Some(self.name.to_string());
        TzifOwned::parse(name, self.data).unwrap_or_else(|err| {
            panic!("failed to parse TZif test file for {:?}: {err}", self.name)
        })
    }

    /// Parse this test TZif data as if it were V1.
    #[cfg(not(miri))]
    pub(crate) fn parse_v1(self) -> TzifOwned {
        use alloc::string::ToString;

        let name = Some(self.name.to_string());
        let mut data = self.data.to_vec();
        data[4] = 0;
        TzifOwned::parse(name, &data).unwrap_or_else(|err| {
            panic!(
                "failed to parse V1 TZif test file for {:?}: {err}",
                self.name
            )
        })
    }
}
