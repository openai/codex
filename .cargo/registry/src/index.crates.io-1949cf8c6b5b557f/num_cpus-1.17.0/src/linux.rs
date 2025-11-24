use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::mem;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Once;

use libc;

macro_rules! debug {
    ($($args:expr),*) => ({
        if false {
        //if true {
            println!($($args),*);
        }
    });
}

macro_rules! some {
    ($e:expr) => {{
        match $e {
            Some(v) => v,
            None => {
                debug!("NONE: {:?}", stringify!($e));
                return None;
            }
        }
    }};
}

pub fn get_num_cpus() -> usize {
    match cgroups_num_cpus() {
        Some(n) => n,
        None => logical_cpus(),
    }
}

fn logical_cpus() -> usize {
    let mut set: libc::cpu_set_t = unsafe { mem::zeroed() };
    if unsafe { libc::sched_getaffinity(0, mem::size_of::<libc::cpu_set_t>(), &mut set) } == 0 {
        let mut count: u32 = 0;
        for i in 0..libc::CPU_SETSIZE as usize {
            if unsafe { libc::CPU_ISSET(i, &set) } {
                count += 1
            }
        }
        count as usize
    } else {
        let cpus = unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) };
        if cpus < 1 {
            1
        } else {
            cpus as usize
        }
    }
}

pub fn get_num_physical_cpus() -> usize {
    let file = match File::open("/proc/cpuinfo") {
        Ok(val) => val,
        Err(_) => return get_num_cpus(),
    };
    let reader = BufReader::new(file);
    let mut map = HashMap::new();
    let mut physid: u32 = 0;
    let mut cores: usize = 0;
    let mut chgcount = 0;
    for line in reader.lines().filter_map(|result| result.ok()) {
        let mut it = line.split(':');
        let (key, value) = match (it.next(), it.next()) {
            (Some(key), Some(value)) => (key.trim(), value.trim()),
            _ => continue,
        };
        if key == "physical id" {
            match value.parse() {
                Ok(val) => physid = val,
                Err(_) => break,
            };
            chgcount += 1;
        }
        if key == "cpu cores" {
            match value.parse() {
                Ok(val) => cores = val,
                Err(_) => break,
            };
            chgcount += 1;
        }
        if chgcount == 2 {
            map.insert(physid, cores);
            chgcount = 0;
        }
    }
    let count = map.into_iter().fold(0, |acc, (_, cores)| acc + cores);

    if count == 0 {
        get_num_cpus()
    } else {
        count
    }
}

/// Cached CPUs calculated from cgroups.
///
/// If 0, check logical cpus.
// Allow deprecation warnings, we want to work on older rustc
#[allow(warnings)]
static CGROUPS_CPUS: AtomicUsize = ::std::sync::atomic::ATOMIC_USIZE_INIT;

fn cgroups_num_cpus() -> Option<usize> {
    #[allow(warnings)]
    static ONCE: Once = ::std::sync::ONCE_INIT;

    ONCE.call_once(init_cgroups);

    let cpus = CGROUPS_CPUS.load(Ordering::Acquire);

    if cpus > 0 {
        Some(cpus)
    } else {
        None
    }
}

fn init_cgroups() {
    // Should only be called once
    debug_assert!(CGROUPS_CPUS.load(Ordering::SeqCst) == 0);

    // Fails in Miri by default (cannot open files), and Miri does not have parallelism anyway.
    if cfg!(miri) {
        return;
    }

    if let Some(quota) = load_cgroups("/proc/self/cgroup", "/proc/self/mountinfo") {
        if quota == 0 {
            return;
        }

        let logical = logical_cpus();
        let count = ::std::cmp::min(quota, logical);

        CGROUPS_CPUS.store(count, Ordering::SeqCst);
    }
}

fn load_cgroups<P1, P2>(cgroup_proc: P1, mountinfo_proc: P2) -> Option<usize>
where
    P1: AsRef<Path>,
    P2: AsRef<Path>,
{
    let subsys = some!(Subsys::load_cpu(cgroup_proc));
    let mntinfo = some!(MountInfo::load_cpu(mountinfo_proc, subsys.version));
    let cgroup = some!(Cgroup::translate(mntinfo, subsys));
    cgroup.cpu_quota()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CgroupVersion {
    V1,
    V2,
}

struct Cgroup {
    version: CgroupVersion,
    base: PathBuf,
}

struct MountInfo {
    version: CgroupVersion,
    root: String,
    mount_point: String,
}

struct Subsys {
    version: CgroupVersion,
    base: String,
}

impl Cgroup {
    fn new(version: CgroupVersion, dir: PathBuf) -> Cgroup {
        Cgroup { version: version, base: dir }
    }

    fn translate(mntinfo: MountInfo, subsys: Subsys) -> Option<Cgroup> {
        // Translate the subsystem directory via the host paths.
        debug!(
            "subsys = {:?}; root = {:?}; mount_point = {:?}",
            subsys.base, mntinfo.root, mntinfo.mount_point
        );

        let rel_from_root = some!(Path::new(&subsys.base).strip_prefix(&mntinfo.root).ok());

        debug!("rel_from_root: {:?}", rel_from_root);

        // join(mp.MountPoint, relPath)
        let mut path = PathBuf::from(mntinfo.mount_point);
        path.push(rel_from_root);
        Some(Cgroup::new(mntinfo.version, path))
    }

    fn cpu_quota(&self) -> Option<usize> {
        let (quota_us, period_us) = match self.version {
            CgroupVersion::V1 => (some!(self.quota_us()), some!(self.period_us())),
            CgroupVersion::V2 => some!(self.max()),
        };

        // protect against dividing by zero
        if period_us == 0 {
            return None;
        }

        // Ceil the division, since we want to be able to saturate
        // the available CPUs, and flooring would leave a CPU un-utilized.

        Some((quota_us as f64 / period_us as f64).ceil() as usize)
    }

    fn quota_us(&self) -> Option<usize> {
        self.param("cpu.cfs_quota_us")
    }

    fn period_us(&self) -> Option<usize> {
        self.param("cpu.cfs_period_us")
    }

    fn max(&self) -> Option<(usize, usize)> {
        let max = some!(self.raw_param("cpu.max"));
        let mut max = some!(max.lines().next()).split(' ');

        let quota = some!(max.next().and_then(|quota| quota.parse().ok()));
        let period = some!(max.next().and_then(|period| period.parse().ok()));

        Some((quota, period))
    }

    fn param(&self, param: &str) -> Option<usize> {
        let buf = some!(self.raw_param(param));

        buf.trim().parse().ok()
    }

    fn raw_param(&self, param: &str) -> Option<String> {
        let mut file = some!(File::open(self.base.join(param)).ok());

        let mut buf = String::new();
        some!(file.read_to_string(&mut buf).ok());

        Some(buf)
    }
}

impl MountInfo {
    fn load_cpu<P: AsRef<Path>>(proc_path: P, version: CgroupVersion) -> Option<MountInfo> {
        let file = some!(File::open(proc_path).ok());
        let file = BufReader::new(file);

        file.lines()
            .filter_map(|result| result.ok())
            .filter_map(MountInfo::parse_line)
            .find(|mount_info| mount_info.version == version)
    }

    fn parse_line(line: String) -> Option<MountInfo> {
        let mut fields = line.split(' ');

        // 7 5 0:6 </> /sys/fs/cgroup/cpu,cpuacct rw,nosuid,nodev,noexec,relatime shared:7 - cgroup cgroup rw,cpu,cpuacct
        let mnt_root = some!(fields.nth(3));
        // 7 5 0:6 / </sys/fs/cgroup/cpu,cpuacct> rw,nosuid,nodev,noexec,relatime shared:7 - cgroup cgroup rw,cpu,cpuacct
        let mnt_point = some!(fields.next());

        // Ignore all fields until the separator(-).
        // Note: there could be zero or more optional fields before hyphen.
        // See: https://man7.org/linux/man-pages/man5/proc.5.html
        // 7 5 0:6 / /sys/fs/cgroup/cpu,cpuacct rw,nosuid,nodev,noexec,relatime shared:7 <-> cgroup cgroup rw,cpu,cpuacct
        // Note: we cannot use `?` here because we need to support Rust 1.13.
        match fields.find(|&s| s == "-") {
            Some(_) => {}
            None => return None,
        };

        // 7 5 0:6 / /sys/fs/cgroup/cpu,cpuacct rw,nosuid,nodev,noexec,relatime shared:7 - <cgroup> cgroup rw,cpu,cpuacct
        let version = match fields.next() {
            Some("cgroup") => CgroupVersion::V1,
            Some("cgroup2") => CgroupVersion::V2,
            _ => return None,
        };

        // cgroups2 only has a single mount point
        if version == CgroupVersion::V1 {
            // 7 5 0:6 / /sys/fs/cgroup/cpu,cpuacct rw,nosuid,nodev,noexec,relatime shared:7 - cgroup cgroup <rw,cpu,cpuacct>
            let super_opts = some!(fields.nth(1));

            // We only care about the 'cpu' option
            if !super_opts.split(',').any(|opt| opt == "cpu") {
                return None;
            }
        }

        Some(MountInfo {
            version: version,
            root: mnt_root.to_owned(),
            mount_point: mnt_point.to_owned(),
        })
    }
}

impl Subsys {
    fn load_cpu<P: AsRef<Path>>(proc_path: P) -> Option<Subsys> {
        let file = some!(File::open(proc_path).ok());
        let file = BufReader::new(file);

        file.lines()
            .filter_map(|result| result.ok())
            .filter_map(Subsys::parse_line)
            .fold(None, |previous, line| {
                // already-found v1 trumps v2 since it explicitly specifies its controllers
                if previous.is_some() && line.version == CgroupVersion::V2 {
                    return previous;
                }

                Some(line)
            })
    }

    fn parse_line(line: String) -> Option<Subsys> {
        // Example format:
        // 11:cpu,cpuacct:/
        let mut fields = line.split(':');

        let sub_systems = some!(fields.nth(1));

        let version = if sub_systems.is_empty() {
            CgroupVersion::V2
        } else {
            CgroupVersion::V1
        };

        if version == CgroupVersion::V1 && !sub_systems.split(',').any(|sub| sub == "cpu") {
            return None;
        }

        fields.next().map(|path| Subsys {
            version: version,
            base: path.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    mod v1 {
        use super::super::{Cgroup, CgroupVersion, MountInfo, Subsys};
        use std::path::{Path, PathBuf};

        // `static_in_const` feature is not stable in Rust 1.13.
        static FIXTURES_PROC: &'static str = "fixtures/cgroups/proc/cgroups";

        static FIXTURES_CGROUPS: &'static str = "fixtures/cgroups/cgroups";

        macro_rules! join {
            ($base:expr, $($path:expr),+) => ({
                Path::new($base)
                    $(.join($path))+
            })
        }

        #[test]
        fn test_load_mountinfo() {
            // test only one optional fields
            let path = join!(FIXTURES_PROC, "mountinfo");

            let mnt_info = MountInfo::load_cpu(path, CgroupVersion::V1).unwrap();

            assert_eq!(mnt_info.root, "/");
            assert_eq!(mnt_info.mount_point, "/sys/fs/cgroup/cpu,cpuacct");

            // test zero optional field
            let path = join!(FIXTURES_PROC, "mountinfo_zero_opt");

            let mnt_info = MountInfo::load_cpu(path, CgroupVersion::V1).unwrap();

            assert_eq!(mnt_info.root, "/");
            assert_eq!(mnt_info.mount_point, "/sys/fs/cgroup/cpu,cpuacct");

            // test multi optional fields
            let path = join!(FIXTURES_PROC, "mountinfo_multi_opt");

            let mnt_info = MountInfo::load_cpu(path, CgroupVersion::V1).unwrap();

            assert_eq!(mnt_info.root, "/");
            assert_eq!(mnt_info.mount_point, "/sys/fs/cgroup/cpu,cpuacct");
        }

        #[test]
        fn test_load_subsys() {
            let path = join!(FIXTURES_PROC, "cgroup");

            let subsys = Subsys::load_cpu(path).unwrap();

            assert_eq!(subsys.base, "/");
            assert_eq!(subsys.version, CgroupVersion::V1);
        }

        #[test]
        fn test_cgroup_mount() {
            let cases = &[
                ("/", "/sys/fs/cgroup/cpu", "/", Some("/sys/fs/cgroup/cpu")),
                (
                    "/docker/01abcd",
                    "/sys/fs/cgroup/cpu",
                    "/docker/01abcd",
                    Some("/sys/fs/cgroup/cpu"),
                ),
                (
                    "/docker/01abcd",
                    "/sys/fs/cgroup/cpu",
                    "/docker/01abcd/",
                    Some("/sys/fs/cgroup/cpu"),
                ),
                (
                    "/docker/01abcd",
                    "/sys/fs/cgroup/cpu",
                    "/docker/01abcd/large",
                    Some("/sys/fs/cgroup/cpu/large"),
                ),
                // fails
                ("/docker/01abcd", "/sys/fs/cgroup/cpu", "/", None),
                ("/docker/01abcd", "/sys/fs/cgroup/cpu", "/docker", None),
                ("/docker/01abcd", "/sys/fs/cgroup/cpu", "/elsewhere", None),
                (
                    "/docker/01abcd",
                    "/sys/fs/cgroup/cpu",
                    "/docker/01abcd-other-dir",
                    None,
                ),
            ];

            for &(root, mount_point, subsys, expected) in cases.iter() {
                let mnt_info = MountInfo {
                    version: CgroupVersion::V1,
                    root: root.into(),
                    mount_point: mount_point.into(),
                };
                let subsys = Subsys {
                    version: CgroupVersion::V1,
                    base: subsys.into(),
                };

                let actual = Cgroup::translate(mnt_info, subsys).map(|c| c.base);
                let expected = expected.map(PathBuf::from);
                assert_eq!(actual, expected);
            }
        }

        #[test]
        fn test_cgroup_cpu_quota() {
            let cgroup = Cgroup::new(CgroupVersion::V1, join!(FIXTURES_CGROUPS, "good"));
            assert_eq!(cgroup.cpu_quota(), Some(6));
        }

        #[test]
        fn test_cgroup_cpu_quota_divide_by_zero() {
            let cgroup = Cgroup::new(CgroupVersion::V1, join!(FIXTURES_CGROUPS, "zero-period"));
            assert!(cgroup.quota_us().is_some());
            assert_eq!(cgroup.period_us(), Some(0));
            assert_eq!(cgroup.cpu_quota(), None);
        }

        #[test]
        fn test_cgroup_cpu_quota_ceil() {
            let cgroup = Cgroup::new(CgroupVersion::V1, join!(FIXTURES_CGROUPS, "ceil"));
            assert_eq!(cgroup.cpu_quota(), Some(2));
        }
    }

    mod v2 {
        use super::super::{Cgroup, CgroupVersion, MountInfo, Subsys};
        use std::path::{Path, PathBuf};

        // `static_in_const` feature is not stable in Rust 1.13.
        static FIXTURES_PROC: &'static str = "fixtures/cgroups2/proc/cgroups";

        static FIXTURES_CGROUPS: &'static str = "fixtures/cgroups2/cgroups";

        macro_rules! join {
            ($base:expr, $($path:expr),+) => ({
                Path::new($base)
                    $(.join($path))+
            })
        }

        #[test]
        fn test_load_mountinfo() {
            // test only one optional fields
            let path = join!(FIXTURES_PROC, "mountinfo");

            let mnt_info = MountInfo::load_cpu(path, CgroupVersion::V2).unwrap();

            assert_eq!(mnt_info.root, "/");
            assert_eq!(mnt_info.mount_point, "/sys/fs/cgroup");
        }

        #[test]
        fn test_load_subsys() {
            let path = join!(FIXTURES_PROC, "cgroup");

            let subsys = Subsys::load_cpu(path).unwrap();

            assert_eq!(subsys.base, "/");
            assert_eq!(subsys.version, CgroupVersion::V2);
        }

        #[test]
        fn test_load_subsys_multi() {
            let path = join!(FIXTURES_PROC, "cgroup_multi");

            let subsys = Subsys::load_cpu(path).unwrap();

            assert_eq!(subsys.base, "/");
            assert_eq!(subsys.version, CgroupVersion::V1);
        }

        #[test]
        fn test_cgroup_mount() {
            let cases = &[
                ("/", "/sys/fs/cgroup/cpu", "/", Some("/sys/fs/cgroup/cpu")),
                (
                    "/docker/01abcd",
                    "/sys/fs/cgroup/cpu",
                    "/docker/01abcd",
                    Some("/sys/fs/cgroup/cpu"),
                ),
                (
                    "/docker/01abcd",
                    "/sys/fs/cgroup/cpu",
                    "/docker/01abcd/",
                    Some("/sys/fs/cgroup/cpu"),
                ),
                (
                    "/docker/01abcd",
                    "/sys/fs/cgroup/cpu",
                    "/docker/01abcd/large",
                    Some("/sys/fs/cgroup/cpu/large"),
                ),
                // fails
                ("/docker/01abcd", "/sys/fs/cgroup/cpu", "/", None),
                ("/docker/01abcd", "/sys/fs/cgroup/cpu", "/docker", None),
                ("/docker/01abcd", "/sys/fs/cgroup/cpu", "/elsewhere", None),
                (
                    "/docker/01abcd",
                    "/sys/fs/cgroup/cpu",
                    "/docker/01abcd-other-dir",
                    None,
                ),
            ];

            for &(root, mount_point, subsys, expected) in cases.iter() {
                let mnt_info = MountInfo {
                    version: CgroupVersion::V1,
                    root: root.into(),
                    mount_point: mount_point.into(),
                };
                let subsys = Subsys {
                    version: CgroupVersion::V1,
                    base: subsys.into(),
                };

                let actual = Cgroup::translate(mnt_info, subsys).map(|c| c.base);
                let expected = expected.map(PathBuf::from);
                assert_eq!(actual, expected);
            }
        }

        #[test]
        fn test_cgroup_cpu_quota() {
            let cgroup = Cgroup::new(CgroupVersion::V2, join!(FIXTURES_CGROUPS, "good"));
            assert_eq!(cgroup.cpu_quota(), Some(6));
        }

        #[test]
        fn test_cgroup_cpu_quota_divide_by_zero() {
            let cgroup = Cgroup::new(CgroupVersion::V2, join!(FIXTURES_CGROUPS, "zero-period"));
            let period = cgroup.max().map(|max| max.1);

            assert_eq!(period, Some(0));
            assert_eq!(cgroup.cpu_quota(), None);
        }

        #[test]
        fn test_cgroup_cpu_quota_ceil() {
            let cgroup = Cgroup::new(CgroupVersion::V2, join!(FIXTURES_CGROUPS, "ceil"));
            assert_eq!(cgroup.cpu_quota(), Some(2));
        }
    }
}
