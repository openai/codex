use std::fmt::{self, Display, Formatter};

/// A list of supported operating system types.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(non_camel_case_types, clippy::upper_case_acronyms)]
#[non_exhaustive]
pub enum Type {
    /// IBM AIX (<https://en.wikipedia.org/wiki/IBM_AIX>).
    AIX,
    /// AlmaLinux (<https://en.wikipedia.org/wiki/AlmaLinux>).
    AlmaLinux,
    /// Alpaquita Linux (<https://bell-sw.com/alpaquita-linux/>).
    Alpaquita,
    /// Alpine Linux (<https://en.wikipedia.org/wiki/Alpine_Linux>).
    Alpine,
    /// Amazon Linux AMI (<https://en.wikipedia.org/wiki/Amazon_Machine_Image#Amazon_Linux_AMI>).
    Amazon,
    /// Android (<https://en.wikipedia.org/wiki/Android_(operating_system)>).
    Android,
    /// AOSC OS (<https://aosc.io/aosc-os/>).
    AOSC,
    /// Arch Linux (<https://en.wikipedia.org/wiki/Arch_Linux>).
    Arch,
    /// Artix Linux (<https://en.wikipedia.org/wiki/Artix_Linux>).
    Artix,
    /// Bluefin (<https://projectbluefin.io>).
    Bluefin,
    /// CachyOS (<https://en.wikipedia.org/wiki/Arch_Linux#Derivatives>).
    CachyOS,
    /// CentOS (<https://en.wikipedia.org/wiki/CentOS>).
    CentOS,
    /// Cygwin (<https://en.wikipedia.org/wiki/Cygwin>).
    Cygwin,
    /// Debian (<https://en.wikipedia.org/wiki/Debian>).
    Debian,
    /// DragonFly BSD (<https://en.wikipedia.org/wiki/DragonFly_BSD>).
    DragonFly,
    /// Emscripten (<https://en.wikipedia.org/wiki/Emscripten>).
    Emscripten,
    /// EndeavourOS (<https://en.wikipedia.org/wiki/EndeavourOS>).
    EndeavourOS,
    /// Fedora (<https://en.wikipedia.org/wiki/Fedora_(operating_system)>).
    Fedora,
    /// FreeBSD (<https://en.wikipedia.org/wiki/FreeBSD>).
    FreeBSD,
    /// Garuda Linux (<https://en.wikipedia.org/wiki/Garuda_Linux>)
    Garuda,
    /// Gentoo Linux (<https://en.wikipedia.org/wiki/Gentoo_Linux>).
    Gentoo,
    /// HardenedBSD (https://hardenedbsd.org/).
    HardenedBSD,
    /// Illumos (https://en.wikipedia.org/wiki/Illumos).
    Illumos,
    /// Kali Linux (https://en.wikipedia.org/wiki/Kali_Linux).
    Kali,
    /// Linux based operating system (<https://en.wikipedia.org/wiki/Linux>).
    Linux,
    /// Mabox (<https://maboxlinux.org/>).
    Mabox,
    /// Mac OS X/OS X/macOS (<https://en.wikipedia.org/wiki/MacOS>).
    Macos,
    /// Manjaro (<https://en.wikipedia.org/wiki/Manjaro>).
    Manjaro,
    /// Mariner (<https://en.wikipedia.org/wiki/CBL-Mariner>).
    Mariner,
    /// MidnightBSD (<https://en.wikipedia.org/wiki/MidnightBSD>).
    MidnightBSD,
    /// Mint (<https://en.wikipedia.org/wiki/Linux_Mint>).
    Mint,
    /// NetBSD (<https://en.wikipedia.org/wiki/NetBSD>).
    NetBSD,
    /// NixOS (<https://en.wikipedia.org/wiki/NixOS>).
    NixOS,
    /// Nobara (<https://nobaraproject.org/>).
    Nobara,
    /// OpenBSD (<https://en.wikipedia.org/wiki/OpenBSD>).
    OpenBSD,
    /// OpenCloudOS (<https://www.opencloudos.org>).
    OpenCloudOS,
    /// openEuler (<https://en.wikipedia.org/wiki/EulerOS>).
    openEuler,
    /// openSUSE (<https://en.wikipedia.org/wiki/OpenSUSE>).
    openSUSE,
    /// Oracle Linux (<https://en.wikipedia.org/wiki/Oracle_Linux>).
    OracleLinux,
    /// Pop!_OS (<https://en.wikipedia.org/wiki/Pop!_OS>)
    Pop,
    /// Raspberry Pi OS (<https://en.wikipedia.org/wiki/Raspberry_Pi_OS>).
    Raspbian,
    /// Red Hat Linux (<https://en.wikipedia.org/wiki/Red_Hat_Linux>).
    Redhat,
    /// Red Hat Enterprise Linux (<https://en.wikipedia.org/wiki/Red_Hat_Enterprise_Linux>).
    RedHatEnterprise,
    /// Redox (<https://en.wikipedia.org/wiki/Redox_(operating_system)>).
    Redox,
    /// Rocky Linux (<https://en.wikipedia.org/wiki/Rocky_Linux>).
    RockyLinux,
    /// Solus (<https://en.wikipedia.org/wiki/Solus_(operating_system)>).
    Solus,
    /// SUSE Linux Enterprise Server (<https://en.wikipedia.org/wiki/SUSE_Linux_Enterprise>).
    SUSE,
    /// Ubuntu (<https://en.wikipedia.org/wiki/Ubuntu_(operating_system)>).
    Ubuntu,
    /// Ultramarine (<https://ultramarine-linux.org/>).
    Ultramarine,
    /// Uos (<https://uos.uniontech.com/>).
    Uos,
    /// Void Linux (<https://en.wikipedia.org/wiki/Void_Linux>).
    Void,
    /// Unknown operating system.
    Unknown,
    /// Windows (<https://en.wikipedia.org/wiki/Microsoft_Windows>).
    Windows,
}

impl Default for Type {
    fn default() -> Self {
        Type::Unknown
    }
}

impl Display for Type {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            Type::Alpaquita => write!(f, "Alpaquita Linux"),
            Type::Alpine => write!(f, "Alpine Linux"),
            Type::AlmaLinux => write!(f, "AlmaLinux"),
            Type::Amazon => write!(f, "Amazon Linux AMI"),
            Type::AOSC => write!(f, "AOSC OS"),
            Type::Arch => write!(f, "Arch Linux"),
            Type::Bluefin => write!(f, "Bluefin"),
            Type::CachyOS => write!(f, "CachyOS Linux"),
            Type::Artix => write!(f, "Artix Linux"),
            Type::DragonFly => write!(f, "DragonFly BSD"),
            Type::Garuda => write!(f, "Garuda Linux"),
            Type::Gentoo => write!(f, "Gentoo Linux"),
            Type::Illumos => write!(f, "illumos"),
            Type::Kali => write!(f, "Kali Linux"),
            Type::Macos => write!(f, "Mac OS"),
            Type::MidnightBSD => write!(f, "Midnight BSD"),
            Type::Mint => write!(f, "Linux Mint"),
            Type::Nobara => write!(f, "Nobara Linux"),
            Type::openEuler => write!(f, "EulerOS"),
            Type::OracleLinux => write!(f, "Oracle Linux"),
            Type::Pop => write!(f, "Pop!_OS"),
            Type::Raspbian => write!(f, "Raspberry Pi OS"),
            Type::Redhat => write!(f, "Red Hat Linux"),
            Type::RedHatEnterprise => write!(f, "Red Hat Enterprise Linux"),
            Type::RockyLinux => write!(f, "Rocky Linux"),
            Type::SUSE => write!(f, "SUSE Linux Enterprise Server"),
            Type::Ultramarine => write!(f, "Ultramarine Linux"),
            Type::Uos => write!(f, "UOS"),
            Type::Void => write!(f, "Void Linux"),
            _ => write!(f, "{self:?}"),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn default() {
        assert_eq!(Type::Unknown, Type::default());
    }

    #[test]
    fn display() {
        let data = [
            (Type::AIX, "AIX"),
            (Type::AlmaLinux, "AlmaLinux"),
            (Type::Alpaquita, "Alpaquita Linux"),
            (Type::Alpine, "Alpine Linux"),
            (Type::Amazon, "Amazon Linux AMI"),
            (Type::Android, "Android"),
            (Type::AOSC, "AOSC OS"),
            (Type::Arch, "Arch Linux"),
            (Type::Artix, "Artix Linux"),
            (Type::Bluefin, "Bluefin"),
            (Type::CachyOS, "CachyOS Linux"),
            (Type::CentOS, "CentOS"),
            (Type::Cygwin, "Cygwin"),
            (Type::Debian, "Debian"),
            (Type::DragonFly, "DragonFly BSD"),
            (Type::Emscripten, "Emscripten"),
            (Type::EndeavourOS, "EndeavourOS"),
            (Type::Fedora, "Fedora"),
            (Type::FreeBSD, "FreeBSD"),
            (Type::Garuda, "Garuda Linux"),
            (Type::Gentoo, "Gentoo Linux"),
            (Type::HardenedBSD, "HardenedBSD"),
            (Type::Illumos, "illumos"),
            (Type::Kali, "Kali Linux"),
            (Type::Linux, "Linux"),
            (Type::Mabox, "Mabox"),
            (Type::Macos, "Mac OS"),
            (Type::Manjaro, "Manjaro"),
            (Type::Mariner, "Mariner"),
            (Type::MidnightBSD, "Midnight BSD"),
            (Type::Mint, "Linux Mint"),
            (Type::NetBSD, "NetBSD"),
            (Type::NixOS, "NixOS"),
            (Type::Nobara, "Nobara Linux"),
            (Type::OpenCloudOS, "OpenCloudOS"),
            (Type::OpenBSD, "OpenBSD"),
            (Type::openEuler, "EulerOS"),
            (Type::openSUSE, "openSUSE"),
            (Type::OracleLinux, "Oracle Linux"),
            (Type::Pop, "Pop!_OS"),
            (Type::Raspbian, "Raspberry Pi OS"),
            (Type::Redhat, "Red Hat Linux"),
            (Type::RedHatEnterprise, "Red Hat Enterprise Linux"),
            (Type::Redox, "Redox"),
            (Type::RockyLinux, "Rocky Linux"),
            (Type::Solus, "Solus"),
            (Type::SUSE, "SUSE Linux Enterprise Server"),
            (Type::Ubuntu, "Ubuntu"),
            (Type::Ultramarine, "Ultramarine Linux"),
            (Type::Unknown, "Unknown"),
            (Type::Uos, "UOS"),
            (Type::Void, "Void Linux"),
            (Type::Windows, "Windows"),
        ];

        for (t, expected) in &data {
            assert_eq!(&t.to_string(), expected);
        }
    }
}
