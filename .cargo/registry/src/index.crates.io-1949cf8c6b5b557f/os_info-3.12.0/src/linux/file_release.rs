// spell-checker:ignore sles, AOSCOS

use std::{fmt, fs::File, io::Read, path::Path};

use log::{trace, warn};

use crate::{matcher::Matcher, Bitness, Info, Type, Version};

pub fn get() -> Option<Info> {
    retrieve(&DISTRIBUTIONS, "/")
}

fn retrieve(distributions: &[ReleaseInfo], root: &str) -> Option<Info> {
    for release_info in distributions {
        let path = Path::new(root).join(release_info.path);

        if !path.exists() {
            trace!("Path '{}' doesn't exist", release_info.path);
            continue;
        }

        let mut file = match File::open(&path) {
            Ok(val) => val,
            Err(e) => {
                warn!("Unable to open {:?} file: {:?}", &path, e);
                continue;
            }
        };

        let mut file_content = String::new();
        if let Err(e) = file.read_to_string(&mut file_content) {
            warn!("Unable to read {:?} file: {:?}", &path, e);
            continue;
        }

        let os_type = (release_info.os_type)(&file_content);

        // If os_type is indeterminate, try the next release_info
        if os_type.is_none() {
            continue;
        }

        let version = (release_info.version)(&file_content);

        return Some(Info {
            // Unwrap is OK here because of the `os_type.is_none()` check above.
            os_type: os_type.unwrap(),
            version: version.unwrap_or(Version::Unknown),
            bitness: Bitness::Unknown,
            ..Default::default()
        });
    }

    // Failed to determine os info
    None
}

/// Struct containing information on how to parse distribution info from a release file.
#[derive(Clone)]
struct ReleaseInfo<'a> {
    /// Relative path to the release file this struct corresponds to from root.
    path: &'a str,

    /// A closure that determines the os type from the release file contents.
    os_type: for<'b> fn(&'b str) -> Option<Type>,

    /// A closure that determines the os version from the release file contents.
    version: for<'b> fn(&'b str) -> Option<Version>,
}

impl fmt::Debug for ReleaseInfo<'_> {
    fn fmt<'a>(&'a self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReleaseInfo")
            .field("path", &self.path)
            .field("os_type", &(self.os_type as fn(&'a str) -> Option<Type>))
            .field("version", &(self.version as fn(&'a str) -> Option<Version>))
            .finish()
    }
}

/// List of all supported distributions and the information on how to parse their version from the
/// release file.
static DISTRIBUTIONS: [ReleaseInfo; 6] = [
    // Keep this first; most modern distributions have this file.
    ReleaseInfo {
        path: "etc/os-release",
        os_type: |release| {
            Matcher::KeyValue { key: "ID" }
                .find(release)
                .and_then(|id| match id.as_str() {
                    // os-release information collected from
                    // https://github.com/chef/os_release
                    "almalinux" => Some(Type::AlmaLinux),
                    "alpaquita" => Some(Type::Alpaquita),
                    "alpine" => Some(Type::Alpine),
                    "amzn" => Some(Type::Amazon),
                    //"antergos" => Antergos
                    "aosc" => Some(Type::AOSC),
                    "arch" => Some(Type::Arch),
                    "archarm" => Some(Type::Arch),
                    "artix" => Some(Type::Artix),
                    "bluefin" => Some(Type::Bluefin),
                    "cachyos" => Some(Type::CachyOS),
                    "centos" => Some(Type::CentOS),
                    //"clear-linux-os" => ClearLinuxOS
                    //"clearos" => ClearOS
                    //"coreos"
                    //"cumulus-linux" => Cumulus
                    "debian" => Some(Type::Debian),
                    //"devuan" => Devuan
                    //"elementary" => Elementary
                    "fedora" => Some(Type::Fedora),
                    //"gentoo" => Gentoo
                    //"ios_xr" => ios_xr
                    "kali" => Some(Type::Kali),
                    //"mageia" => Mageia
                    //"manjaro" => Manjaro
                    "manjaro-arm" => Some(Type::Manjaro),
                    "linuxmint" => Some(Type::Mint),
                    "mariner" => Some(Type::Mariner),
                    //"nexus" => Nexus
                    "nixos" => Some(Type::NixOS),
                    "nobara" => Some(Type::Nobara),
                    "Uos" => Some(Type::Uos),
                    "opencloudos" => Some(Type::OpenCloudOS),
                    "openEuler" => Some(Type::openEuler),
                    "ol" => Some(Type::OracleLinux),
                    "opensuse" => Some(Type::openSUSE),
                    "opensuse-leap" => Some(Type::openSUSE),
                    "opensuse-microos" => Some(Type::openSUSE),
                    "opensuse-tumbleweed" => Some(Type::openSUSE),
                    //"rancheros" => RancherOS
                    //"raspbian" => Raspbian
                    // note XBian also uses "raspbian"
                    "rhel" => Some(Type::RedHatEnterprise),
                    "rocky" => Some(Type::RockyLinux),
                    //"sabayon" => Sabayon
                    //"scientific" => Scientific
                    //"slackware" => Slackware
                    "sled" => Some(Type::SUSE), // SUSE desktop
                    "sles" => Some(Type::SUSE),
                    "sles_sap" => Some(Type::SUSE), // SUSE SAP
                    "ubuntu" => Some(Type::Ubuntu),
                    "ultramarine" => Some(Type::Ultramarine),
                    //"virtuozzo" => Virtuozzo
                    "void" => Some(Type::Void),
                    //"XCP-ng" => xcp-ng
                    //"xenenterprise" => xcp-ng
                    //"xenserver" => xcp-ng
                    _ => None,
                })
        },
        version: |release| {
            Matcher::KeyValue { key: "VERSION_ID" }
                .find(release)
                .map(Version::from_string)
        },
    },
    // Older distributions must have their specific release file parsed.
    ReleaseInfo {
        path: "etc/mariner-release",
        os_type: |_| Some(Type::Mariner),
        version: |release| {
            Matcher::PrefixedVersion {
                prefix: "CBL-Mariner",
            }
            .find(release)
            .map(Version::from_string)
        },
    },
    ReleaseInfo {
        path: "etc/centos-release",
        os_type: |_| Some(Type::CentOS),
        version: |release| {
            Matcher::PrefixedVersion { prefix: "release" }
                .find(release)
                .map(Version::from_string)
        },
    },
    ReleaseInfo {
        path: "etc/fedora-release",
        os_type: |_| Some(Type::Fedora),
        version: |release| {
            Matcher::PrefixedVersion { prefix: "release" }
                .find(release)
                .map(Version::from_string)
        },
    },
    ReleaseInfo {
        path: "etc/alpine-release",
        os_type: |_| Some(Type::Alpine),
        version: |release| Matcher::AllTrimmed.find(release).map(Version::from_string),
    },
    ReleaseInfo {
        path: "etc/redhat-release",
        os_type: |_| Some(Type::RedHatEnterprise),
        version: |release| {
            Matcher::PrefixedVersion { prefix: "release" }
                .find(release)
                .map(Version::from_string)
        },
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn almalinux_9_0_release() {
        let root = "src/linux/tests/AlmaLinux-9.0";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::AlmaLinux);
        assert_eq!(info.version, Version::Semantic(9, 0, 0));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn alpaquita_os_release() {
        let root = "src/linux/tests/Alpaquita";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::Alpaquita);
        assert_eq!(info.version, Version::Semantic(23, 0, 0));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn alpine_3_12_os_release() {
        let root = "src/linux/tests/Alpine_3_12";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::Alpine);
        assert_eq!(info.version, Version::Semantic(3, 12, 0));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn alpine_release() {
        let root = "src/linux/tests/Alpine";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::Alpine);
        assert_eq!(info.version, Version::Custom("A.B.C".to_owned()));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn amazon_1_os_release() {
        let root = "src/linux/tests/Amazon_1";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::Amazon);
        assert_eq!(info.version, Version::Semantic(2018, 3, 0));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn amazon_2_os_release() {
        let root = "src/linux/tests/Amazon_2";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::Amazon);
        assert_eq!(info.version, Version::Semantic(2, 0, 0));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn aosc_os_release() {
        let root = "src/linux/tests/AOSCOS";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::AOSC);
        assert_eq!(info.version, Version::Semantic(12, 1, 3));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn arch_os_release() {
        let root = "src/linux/tests/Arch";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::Arch);
        assert_eq!(info.version, Version::Unknown);
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn archarm_os_release() {
        let root = "src/linux/tests/ArchARM";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::Arch);
        assert_eq!(info.version, Version::Unknown);
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn artix_os_release() {
        let root = "src/linux/tests/Artix";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::Artix);
        assert_eq!(info.version, Version::Unknown);
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn bluefin_os_release() {
        let root = "src/linux/tests/Bluefin";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::Bluefin);
        assert_eq!(info.version, Version::Semantic(41, 0, 0));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn centos_7_os_release() {
        let root = "src/linux/tests/CentOS_7";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::CentOS);
        assert_eq!(info.version, Version::Semantic(7, 0, 0));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn centos_stream_os_release() {
        let root = "src/linux/tests/CentOS_Stream";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::CentOS);
        assert_eq!(info.version, Version::Semantic(8, 0, 0));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn centos_release() {
        let root = "src/linux/tests/CentOS";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::CentOS);
        assert_eq!(info.version, Version::Custom("XX".to_owned()));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn centos_release_unknown() {
        let root = "src/linux/tests/CentOS_Unknown";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::CentOS);
        assert_eq!(info.version, Version::Unknown);
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn debian_11_os_release() {
        let root = "src/linux/tests/Debian_11";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::Debian);
        assert_eq!(info.version, Version::Semantic(11, 0, 0));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn fedora_32_os_release() {
        let root = "src/linux/tests/Fedora_32";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::Fedora);
        assert_eq!(info.version, Version::Semantic(32, 0, 0));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn fedora_35_os_release() {
        let root = "src/linux/tests/Fedora_35";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::Fedora);
        assert_eq!(info.version, Version::Semantic(35, 0, 0));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn fedora_release() {
        let root = "src/linux/tests/Fedora";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::Fedora);
        assert_eq!(info.version, Version::Semantic(26, 0, 0));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn fedora_release_unknown() {
        let root = "src/linux/tests/Fedora_Unknown";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::Fedora);
        assert_eq!(info.version, Version::Unknown);
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn kali_2023_2_os_release() {
        let root = "src/linux/tests/Kali_2023_2";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::Kali);
        assert_eq!(info.version, Version::Semantic(2023, 2, 0));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn manjaro_arm_release() {
        let root = "src/linux/tests/ManjaroArm";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::Manjaro);
        assert_eq!(info.version, Version::Unknown);
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn mariner_release() {
        let root = "src/linux/tests/Mariner";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::Mariner);
        assert_eq!(info.version, Version::Semantic(2, 0, 20220210));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn mariner_release_unknown() {
        let root = "src/linux/tests/Mariner_Unknown";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::Mariner);
        assert_eq!(info.version, Version::Unknown);
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn mint_os_release() {
        let root = "src/linux/tests/Mint";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::Mint);
        assert_eq!(info.version, Version::Semantic(20, 0, 0));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn nixos_os_release() {
        let root = "src/linux/tests/NixOS";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::NixOS);
        assert_eq!(
            info.version,
            Version::Custom("21.05pre275822.916ee862e87".to_string())
        );
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn nobara_os_release() {
        let root = "src/linux/tests/Nobara";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::Nobara);
        assert_eq!(info.version, Version::Semantic(39, 0, 0));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn uos_os_release() {
        let root = "src/linux/tests/Uos";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::Uos);
        assert_eq!(info.version, Version::Semantic(20, 0, 0));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn none_invalid_os_release() {
        let root = "src/linux/tests/none_invalid_os_release";

        let info = retrieve(&DISTRIBUTIONS, root);
        assert_eq!(info, None);
    }

    #[test]
    fn none_no_release() {
        let root = "src/linux/tests/none_no_release";

        let info = retrieve(&DISTRIBUTIONS, root);
        assert_eq!(info, None);
    }

    #[test]
    fn none_no_path() {
        let root = "src/linux/tests/none_no_path";

        let info = retrieve(&DISTRIBUTIONS, root);
        assert_eq!(info, None);
    }

    #[test]
    fn opencloudos_os_release() {
        let root = "src/linux/tests/OpenCloudOS";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::OpenCloudOS);
        assert_eq!(info.version, Version::Semantic(8, 6, 0));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn openeuler_os_release() {
        let root = "src/linux/tests/openEuler";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::openEuler);
        assert_eq!(info.version, Version::Semantic(22, 3, 0));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn opensuse_tumbleweed_os_release() {
        let root = "src/linux/tests/openSUSE_Tumbleweed";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::openSUSE);
        assert_eq!(info.version, Version::Semantic(20230816, 0, 0));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn oracle_linux_os_release() {
        let root = "src/linux/tests/OracleLinux";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::OracleLinux);
        assert_eq!(info.version, Version::Semantic(8, 1, 0));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn rhel_8_os_release() {
        let root = "src/linux/tests/RedHatEnterprise_8";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::RedHatEnterprise);
        assert_eq!(info.version, Version::Semantic(8, 2, 0));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn rhel_7_os_release() {
        let root = "src/linux/tests/RedHatEnterprise_7";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::RedHatEnterprise);
        assert_eq!(info.version, Version::Semantic(7, 9, 0));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn redhat_release() {
        let root = "src/linux/tests/RedHatEnterprise";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::RedHatEnterprise);
        assert_eq!(info.version, Version::Custom("XX".to_owned()));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn redhat_release_unknown() {
        let root = "src/linux/tests/RedHatEnterprise_Unknown";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::RedHatEnterprise);
        assert_eq!(info.version, Version::Unknown);
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn rocky_9_2_release() {
        let root = "src/linux/tests/RockyLinux-9.2";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::RockyLinux);
        assert_eq!(info.version, Version::Semantic(9, 2, 0));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn suse_12_os_release() {
        let root = "src/linux/tests/SUSE_12";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::SUSE);
        assert_eq!(info.version, Version::Semantic(12, 5, 0));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn suse_15_os_release() {
        let root = "src/linux/tests/SUSE_15";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::SUSE);
        assert_eq!(info.version, Version::Semantic(15, 2, 0));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn ubuntu_os_release() {
        let root = "src/linux/tests/Ubuntu";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::Ubuntu);
        assert_eq!(info.version, Version::Semantic(18, 10, 0));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn ultramarine_os_release() {
        let root = "src/linux/tests/Ultramarine";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::Ultramarine);
        assert_eq!(info.version, Version::Semantic(39, 0, 0));
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn void_os_release() {
        let root = "src/linux/tests/Void";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::Void);
        assert_eq!(info.version, Version::Unknown);
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn cachy_os_release() {
        let root = "src/linux/tests/CachyOS";

        let info = retrieve(&DISTRIBUTIONS, root).unwrap();
        assert_eq!(info.os_type(), Type::CachyOS);
        assert_eq!(info.version, Version::Unknown);
        assert_eq!(info.edition, None);
        assert_eq!(info.codename, None);
    }

    #[test]
    fn release_info_debug() {
        dbg!("{:?}", &DISTRIBUTIONS[0]);
    }
}
