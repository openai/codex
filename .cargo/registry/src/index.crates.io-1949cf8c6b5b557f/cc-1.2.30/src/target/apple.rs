use super::TargetInfo;

impl TargetInfo<'_> {
    pub(crate) fn apple_sdk_name(&self) -> &'static str {
        match (self.os, self.abi) {
            ("macos", "") => "macosx",
            ("ios", "") => "iphoneos",
            ("ios", "sim") => "iphonesimulator",
            ("ios", "macabi") => "macosx",
            ("tvos", "") => "appletvos",
            ("tvos", "sim") => "appletvsimulator",
            ("watchos", "") => "watchos",
            ("watchos", "sim") => "watchsimulator",
            ("visionos", "") => "xros",
            ("visionos", "sim") => "xrsimulator",
            (os, _) => panic!("invalid Apple target OS {}", os),
        }
    }

    pub(crate) fn apple_version_flag(&self, min_version: &str) -> String {
        // There are many aliases for these, and `-mtargetos=` is preferred on Clang nowadays, but
        // for compatibility with older Clang, we use the earliest supported name here.
        //
        // NOTE: GCC does not support `-miphoneos-version-min=` etc. (because it does not support
        // iOS in general), but we specify them anyhow in case we actually have a Clang-like
        // compiler disguised as a GNU-like compiler, or in case GCC adds support for these in the
        // future.
        //
        // See also:
        // https://clang.llvm.org/docs/ClangCommandLineReference.html#cmdoption-clang-mmacos-version-min
        // https://clang.llvm.org/docs/AttributeReference.html#availability
        // https://gcc.gnu.org/onlinedocs/gcc/Darwin-Options.html#index-mmacosx-version-min
        match (self.os, self.abi) {
            ("macos", "") => format!("-mmacosx-version-min={min_version}"),
            ("ios", "") => format!("-miphoneos-version-min={min_version}"),
            ("ios", "sim") => format!("-mios-simulator-version-min={min_version}"),
            ("ios", "macabi") => format!("-mtargetos=ios{min_version}-macabi"),
            ("tvos", "") => format!("-mappletvos-version-min={min_version}"),
            ("tvos", "sim") => format!("-mappletvsimulator-version-min={min_version}"),
            ("watchos", "") => format!("-mwatchos-version-min={min_version}"),
            ("watchos", "sim") => format!("-mwatchsimulator-version-min={min_version}"),
            // `-mxros-version-min` does not exist
            // https://github.com/llvm/llvm-project/issues/88271
            ("visionos", "") => format!("-mtargetos=xros{min_version}"),
            ("visionos", "sim") => format!("-mtargetos=xros{min_version}-simulator"),
            (os, _) => panic!("invalid Apple target OS {}", os),
        }
    }
}
