use crate::off_t;
use crate::prelude::*;

// This file follows the Haiku API for Haiku R1 beta 5. It is organized by the
// C/C++ header files in which the concepts can be found, while adhering to the
// style guide for this crate.

// Helper macro to generate u32 constants. The Haiku API uses (non-standard)
// multi-character constants (like 'UPDA' or 'MSGM') to represent 32 bit
// integer constants.

macro_rules! haiku_constant {
    ($a:tt, $b:tt, $c:tt, $d:tt) => {
        (($a as u32) << 24) + (($b as u32) << 16) + (($c as u32) << 8) + ($d as u32)
    };
}

// support/SupportDefs.h
pub type status_t = i32;
pub type bigtime_t = i64;
pub type nanotime_t = i64;
pub type type_code = u32;
pub type perform_code = u32;

// kernel/OS.h
pub type area_id = i32;
pub type port_id = i32;
pub type sem_id = i32;
pub type team_id = i32;
pub type thread_id = i32;

pub type thread_func = extern "C" fn(*mut c_void) -> status_t;

// kernel/image.h
pub type image_id = i32;

c_enum! {
    // kernel/OS.h
    pub enum thread_state {
        B_THREAD_RUNNING = 1,
        B_THREAD_READY,
        B_THREAD_RECEIVING,
        B_THREAD_ASLEEP,
        B_THREAD_SUSPENDED,
        B_THREAD_WAITING,
    }

    // kernel/image.h
    pub enum image_type {
        B_APP_IMAGE = 1,
        B_LIBRARY_IMAGE,
        B_ADD_ON_IMAGE,
        B_SYSTEM_IMAGE,
    }

    // kernel/scheduler.h

    pub enum be_task_flags {
        B_DEFAULT_MEDIA_PRIORITY = 0x000,
        B_OFFLINE_PROCESSING = 0x001,
        B_STATUS_RENDERING = 0x002,
        B_USER_INPUT_HANDLING = 0x004,
        B_LIVE_VIDEO_MANIPULATION = 0x008,
        B_VIDEO_PLAYBACK = 0x010,
        B_VIDEO_RECORDING = 0x020,
        B_LIVE_AUDIO_MANIPULATION = 0x040,
        B_AUDIO_PLAYBACK = 0x080,
        B_AUDIO_RECORDING = 0x100,
        B_LIVE_3D_RENDERING = 0x200,
        B_NUMBER_CRUNCHING = 0x400,
        B_MIDI_PROCESSING = 0x800,
    }

    pub enum schduler_mode {
        SCHEDULER_MODE_LOW_LATENCY,
        SCHEDULER_MODE_POWER_SAVING,
    }

    // FindDirectory.h
    pub enum path_base_directory {
        B_FIND_PATH_INSTALLATION_LOCATION_DIRECTORY,
        B_FIND_PATH_ADD_ONS_DIRECTORY,
        B_FIND_PATH_APPS_DIRECTORY,
        B_FIND_PATH_BIN_DIRECTORY,
        B_FIND_PATH_BOOT_DIRECTORY,
        B_FIND_PATH_CACHE_DIRECTORY,
        B_FIND_PATH_DATA_DIRECTORY,
        B_FIND_PATH_DEVELOP_DIRECTORY,
        B_FIND_PATH_DEVELOP_LIB_DIRECTORY,
        B_FIND_PATH_DOCUMENTATION_DIRECTORY,
        B_FIND_PATH_ETC_DIRECTORY,
        B_FIND_PATH_FONTS_DIRECTORY,
        B_FIND_PATH_HEADERS_DIRECTORY,
        B_FIND_PATH_LIB_DIRECTORY,
        B_FIND_PATH_LOG_DIRECTORY,
        B_FIND_PATH_MEDIA_NODES_DIRECTORY,
        B_FIND_PATH_PACKAGES_DIRECTORY,
        B_FIND_PATH_PREFERENCES_DIRECTORY,
        B_FIND_PATH_SERVERS_DIRECTORY,
        B_FIND_PATH_SETTINGS_DIRECTORY,
        B_FIND_PATH_SOUNDS_DIRECTORY,
        B_FIND_PATH_SPOOL_DIRECTORY,
        B_FIND_PATH_TRANSLATORS_DIRECTORY,
        B_FIND_PATH_VAR_DIRECTORY,
        B_FIND_PATH_IMAGE_PATH = 1000,
        B_FIND_PATH_PACKAGE_PATH,
    }

    pub enum directory_which {
        B_DESKTOP_DIRECTORY = 0,
        B_TRASH_DIRECTORY,
        B_SYSTEM_DIRECTORY = 1000,
        B_SYSTEM_ADDONS_DIRECTORY = 1002,
        B_SYSTEM_BOOT_DIRECTORY,
        B_SYSTEM_FONTS_DIRECTORY,
        B_SYSTEM_LIB_DIRECTORY,
        B_SYSTEM_SERVERS_DIRECTORY,
        B_SYSTEM_APPS_DIRECTORY,
        B_SYSTEM_BIN_DIRECTORY,
        B_SYSTEM_DOCUMENTATION_DIRECTORY = 1010,
        B_SYSTEM_PREFERENCES_DIRECTORY,
        B_SYSTEM_TRANSLATORS_DIRECTORY,
        B_SYSTEM_MEDIA_NODES_DIRECTORY,
        B_SYSTEM_SOUNDS_DIRECTORY,
        B_SYSTEM_DATA_DIRECTORY,
        B_SYSTEM_DEVELOP_DIRECTORY,
        B_SYSTEM_PACKAGES_DIRECTORY,
        B_SYSTEM_HEADERS_DIRECTORY,
        B_SYSTEM_ETC_DIRECTORY = 2008,
        B_SYSTEM_SETTINGS_DIRECTORY = 2010,
        B_SYSTEM_LOG_DIRECTORY = 2012,
        B_SYSTEM_SPOOL_DIRECTORY,
        B_SYSTEM_TEMP_DIRECTORY,
        B_SYSTEM_VAR_DIRECTORY,
        B_SYSTEM_CACHE_DIRECTORY = 2020,
        B_SYSTEM_NONPACKAGED_DIRECTORY = 2023,
        B_SYSTEM_NONPACKAGED_ADDONS_DIRECTORY,
        B_SYSTEM_NONPACKAGED_TRANSLATORS_DIRECTORY,
        B_SYSTEM_NONPACKAGED_MEDIA_NODES_DIRECTORY,
        B_SYSTEM_NONPACKAGED_BIN_DIRECTORY,
        B_SYSTEM_NONPACKAGED_DATA_DIRECTORY,
        B_SYSTEM_NONPACKAGED_FONTS_DIRECTORY,
        B_SYSTEM_NONPACKAGED_SOUNDS_DIRECTORY,
        B_SYSTEM_NONPACKAGED_DOCUMENTATION_DIRECTORY,
        B_SYSTEM_NONPACKAGED_LIB_DIRECTORY,
        B_SYSTEM_NONPACKAGED_HEADERS_DIRECTORY,
        B_SYSTEM_NONPACKAGED_DEVELOP_DIRECTORY,
        B_USER_DIRECTORY = 3000,
        B_USER_CONFIG_DIRECTORY,
        B_USER_ADDONS_DIRECTORY,
        B_USER_BOOT_DIRECTORY,
        B_USER_FONTS_DIRECTORY,
        B_USER_LIB_DIRECTORY,
        B_USER_SETTINGS_DIRECTORY,
        B_USER_DESKBAR_DIRECTORY,
        B_USER_PRINTERS_DIRECTORY,
        B_USER_TRANSLATORS_DIRECTORY,
        B_USER_MEDIA_NODES_DIRECTORY,
        B_USER_SOUNDS_DIRECTORY,
        B_USER_DATA_DIRECTORY,
        B_USER_CACHE_DIRECTORY,
        B_USER_PACKAGES_DIRECTORY,
        B_USER_HEADERS_DIRECTORY,
        B_USER_NONPACKAGED_DIRECTORY,
        B_USER_NONPACKAGED_ADDONS_DIRECTORY,
        B_USER_NONPACKAGED_TRANSLATORS_DIRECTORY,
        B_USER_NONPACKAGED_MEDIA_NODES_DIRECTORY,
        B_USER_NONPACKAGED_BIN_DIRECTORY,
        B_USER_NONPACKAGED_DATA_DIRECTORY,
        B_USER_NONPACKAGED_FONTS_DIRECTORY,
        B_USER_NONPACKAGED_SOUNDS_DIRECTORY,
        B_USER_NONPACKAGED_DOCUMENTATION_DIRECTORY,
        B_USER_NONPACKAGED_LIB_DIRECTORY,
        B_USER_NONPACKAGED_HEADERS_DIRECTORY,
        B_USER_NONPACKAGED_DEVELOP_DIRECTORY,
        B_USER_DEVELOP_DIRECTORY,
        B_USER_DOCUMENTATION_DIRECTORY,
        B_USER_SERVERS_DIRECTORY,
        B_USER_APPS_DIRECTORY,
        B_USER_BIN_DIRECTORY,
        B_USER_PREFERENCES_DIRECTORY,
        B_USER_ETC_DIRECTORY,
        B_USER_LOG_DIRECTORY,
        B_USER_SPOOL_DIRECTORY,
        B_USER_VAR_DIRECTORY,
        B_APPS_DIRECTORY = 4000,
        B_PREFERENCES_DIRECTORY,
        B_UTILITIES_DIRECTORY,
        B_PACKAGE_LINKS_DIRECTORY,
    }

    // kernel/OS.h

    pub enum topology_level_type {
        B_TOPOLOGY_UNKNOWN,
        B_TOPOLOGY_ROOT,
        B_TOPOLOGY_SMT,
        B_TOPOLOGY_CORE,
        B_TOPOLOGY_PACKAGE,
    }

    pub enum cpu_platform {
        B_CPU_UNKNOWN,
        B_CPU_x86,
        B_CPU_x86_64,
        B_CPU_PPC,
        B_CPU_PPC_64,
        B_CPU_M68K,
        B_CPU_ARM,
        B_CPU_ARM_64,
        B_CPU_ALPHA,
        B_CPU_MIPS,
        B_CPU_SH,
        B_CPU_SPARC,
        B_CPU_RISC_V,
    }

    pub enum cpu_vendor {
        B_CPU_VENDOR_UNKNOWN,
        B_CPU_VENDOR_AMD,
        B_CPU_VENDOR_CYRIX,
        B_CPU_VENDOR_IDT,
        B_CPU_VENDOR_INTEL,
        B_CPU_VENDOR_NATIONAL_SEMICONDUCTOR,
        B_CPU_VENDOR_RISE,
        B_CPU_VENDOR_TRANSMETA,
        B_CPU_VENDOR_VIA,
        B_CPU_VENDOR_IBM,
        B_CPU_VENDOR_MOTOROLA,
        B_CPU_VENDOR_NEC,
        B_CPU_VENDOR_HYGON,
        B_CPU_VENDOR_SUN,
        B_CPU_VENDOR_FUJITSU,
    }
}

s! {
    // kernel/OS.h
    pub struct area_info {
        pub area: area_id,
        pub name: [c_char; B_OS_NAME_LENGTH],
        pub size: usize,
        pub lock: u32,
        pub protection: u32,
        pub team: team_id,
        pub ram_size: u32,
        pub copy_count: u32,
        pub in_count: u32,
        pub out_count: u32,
        pub address: *mut c_void,
    }

    pub struct port_info {
        pub port: port_id,
        pub team: team_id,
        pub name: [c_char; B_OS_NAME_LENGTH],
        pub capacity: i32,
        pub queue_count: i32,
        pub total_count: i32,
    }

    pub struct port_message_info {
        pub size: size_t,
        pub sender: crate::uid_t,
        pub sender_group: crate::gid_t,
        pub sender_team: crate::team_id,
    }

    pub struct team_info {
        pub team: team_id,
        pub thread_count: i32,
        pub image_count: i32,
        pub area_count: i32,
        pub debugger_nub_thread: thread_id,
        pub debugger_nub_port: port_id,
        pub argc: i32,
        pub args: [c_char; 64],
        pub uid: crate::uid_t,
        pub gid: crate::gid_t,
    }

    pub struct sem_info {
        pub sem: sem_id,
        pub team: team_id,
        pub name: [c_char; B_OS_NAME_LENGTH],
        pub count: i32,
        pub latest_holder: thread_id,
    }

    pub struct team_usage_info {
        pub user_time: bigtime_t,
        pub kernel_time: bigtime_t,
    }

    pub struct thread_info {
        pub thread: thread_id,
        pub team: team_id,
        pub name: [c_char; B_OS_NAME_LENGTH],
        pub state: thread_state,
        pub priority: i32,
        pub sem: sem_id,
        pub user_time: bigtime_t,
        pub kernel_time: bigtime_t,
        pub stack_base: *mut c_void,
        pub stack_end: *mut c_void,
    }

    pub struct cpu_info {
        pub active_time: bigtime_t,
        pub enabled: bool,
        pub current_frequency: u64,
    }

    pub struct system_info {
        pub boot_time: bigtime_t,
        pub cpu_count: u32,
        pub max_pages: u64,
        pub used_pages: u64,
        pub cached_pages: u64,
        pub block_cache_pages: u64,
        pub ignored_pages: u64,
        pub needed_memory: u64,
        pub free_memory: u64,
        pub max_swap_pages: u64,
        pub free_swap_pages: u64,
        pub page_faults: u32,
        pub max_sems: u32,
        pub used_sems: u32,
        pub max_ports: u32,
        pub used_ports: u32,
        pub max_threads: u32,
        pub used_threads: u32,
        pub max_teams: u32,
        pub used_teams: u32,
        pub kernel_name: [c_char; B_FILE_NAME_LENGTH],
        pub kernel_build_date: [c_char; B_OS_NAME_LENGTH],
        pub kernel_build_time: [c_char; B_OS_NAME_LENGTH],
        pub kernel_version: i64,
        pub abi: u32,
    }

    pub struct object_wait_info {
        pub object: i32,
        pub type_: u16,
        pub events: u16,
    }

    pub struct cpu_topology_root_info {
        pub platform: cpu_platform,
    }

    pub struct cpu_topology_package_info {
        pub vendor: cpu_vendor,
        pub cache_line_size: u32,
    }

    pub struct cpu_topology_core_info {
        pub model: u32,
        pub default_frequency: u64,
    }
    // kernel/fs_attr.h
    pub struct attr_info {
        pub type_: u32,
        pub size: off_t,
    }

    // kernel/fs_index.h
    pub struct index_info {
        pub type_: u32,
        pub size: off_t,
        pub modification_time: crate::time_t,
        pub creation_time: crate::time_t,
        pub uid: crate::uid_t,
        pub gid: crate::gid_t,
    }

    //kernel/fs_info.h
    pub struct fs_info {
        pub dev: crate::dev_t,
        pub root: crate::ino_t,
        pub flags: u32,
        pub block_size: off_t,
        pub io_size: off_t,
        pub total_blocks: off_t,
        pub free_blocks: off_t,
        pub total_nodes: off_t,
        pub free_nodes: off_t,
        pub device_name: [c_char; 128],
        pub volume_name: [c_char; B_FILE_NAME_LENGTH],
        pub fsh_name: [c_char; B_OS_NAME_LENGTH],
    }

    // kernel/image.h
    // FIXME(1.0): This should not implement `PartialEq`
    #[allow(unpredictable_function_pointer_comparisons)]
    pub struct image_info {
        pub id: image_id,
        pub image_type: c_int,
        pub sequence: i32,
        pub init_order: i32,
        // FIXME(1.0): these should be made optional
        pub init_routine: extern "C" fn(),
        pub term_routine: extern "C" fn(),
        pub device: crate::dev_t,
        pub node: crate::ino_t,
        pub name: [c_char; crate::PATH_MAX as usize],
        pub text: *mut c_void,
        pub data: *mut c_void,
        pub text_size: i32,
        pub data_size: i32,
        pub api_version: i32,
        pub abi: i32,
    }

    pub struct __c_anonymous_eax_0 {
        pub max_eax: u32,
        pub vendor_id: [c_char; 12],
    }

    pub struct __c_anonymous_eax_1 {
        pub stepping: u32,
        pub model: u32,
        pub family: u32,
        pub tpe: u32,
        __reserved_0: u32,
        pub extended_model: u32,
        pub extended_family: u32,
        __reserved_1: u32,
        pub brand_index: u32,
        pub clflush: u32,
        pub logical_cpus: u32,
        pub apic_id: u32,
        pub features: u32,
        pub extended_features: u32,
    }

    pub struct __c_anonymous_eax_2 {
        pub call_num: u8,
        pub cache_descriptors: [u8; 15],
    }

    pub struct __c_anonymous_eax_3 {
        __reserved: [u32; 2],
        pub serial_number_high: u32,
        pub serial_number_low: u32,
    }

    pub struct __c_anonymous_regs {
        pub eax: u32,
        pub ebx: u32,
        pub edx: u32,
        pub ecx: u32,
    }
}

s_no_extra_traits! {
    pub union cpuid_info {
        pub eax_0: __c_anonymous_eax_0,
        pub eax_1: __c_anonymous_eax_1,
        pub eax_2: __c_anonymous_eax_2,
        pub eax_3: __c_anonymous_eax_3,
        pub as_chars: [c_char; 16],
        pub regs: __c_anonymous_regs,
    }

    pub union __c_anonymous_cpu_topology_info_data {
        pub root: cpu_topology_root_info,
        pub package: cpu_topology_package_info,
        pub core: cpu_topology_core_info,
    }

    pub struct cpu_topology_node_info {
        pub id: u32,
        pub type_: topology_level_type,
        pub level: u32,
        pub data: __c_anonymous_cpu_topology_info_data,
    }
}

cfg_if! {
    if #[cfg(feature = "extra_traits")] {
        impl PartialEq for cpuid_info {
            fn eq(&self, other: &cpuid_info) -> bool {
                unsafe {
                    self.eax_0 == other.eax_0
                        || self.eax_1 == other.eax_1
                        || self.eax_2 == other.eax_2
                        || self.eax_3 == other.eax_3
                        || self.as_chars == other.as_chars
                        || self.regs == other.regs
                }
            }
        }
        impl Eq for cpuid_info {}

        impl PartialEq for __c_anonymous_cpu_topology_info_data {
            fn eq(&self, other: &__c_anonymous_cpu_topology_info_data) -> bool {
                unsafe {
                    self.root == other.root
                        || self.package == other.package
                        || self.core == other.core
                }
            }
        }
        impl Eq for __c_anonymous_cpu_topology_info_data {}

        impl PartialEq for cpu_topology_node_info {
            fn eq(&self, other: &cpu_topology_node_info) -> bool {
                self.id == other.id && self.type_ == other.type_ && self.level == other.level
            }
        }

        impl Eq for cpu_topology_node_info {}
    }
}

// kernel/OS.h
pub const B_OS_NAME_LENGTH: usize = 32;
pub const B_PAGE_SIZE: usize = 4096;
pub const B_INFINITE_TIMEOUT: usize = 9223372036854775807;

pub const B_RELATIVE_TIMEOUT: u32 = 0x8;
pub const B_ABSOLUTE_TIMEOUT: u32 = 0x10;
pub const B_TIMEOUT_REAL_TIME_BASE: u32 = 0x40;
pub const B_ABSOLUTE_REAL_TIME_TIMEOUT: u32 = B_ABSOLUTE_TIMEOUT | B_TIMEOUT_REAL_TIME_BASE;

pub const B_NO_LOCK: u32 = 0;
pub const B_LAZY_LOCK: u32 = 1;
pub const B_FULL_LOCK: u32 = 2;
pub const B_CONTIGUOUS: u32 = 3;
pub const B_LOMEM: u32 = 4;
pub const B_32_BIT_FULL_LOCK: u32 = 5;
pub const B_32_BIT_CONTIGUOUS: u32 = 6;

pub const B_ANY_ADDRESS: u32 = 0;
pub const B_EXACT_ADDRESS: u32 = 1;
pub const B_BASE_ADDRESS: u32 = 2;
pub const B_CLONE_ADDRESS: u32 = 3;
pub const B_ANY_KERNEL_ADDRESS: u32 = 4;
pub const B_RANDOMIZED_ANY_ADDRESS: u32 = 6;
pub const B_RANDOMIZED_BASE_ADDRESS: u32 = 7;

pub const B_READ_AREA: u32 = 1 << 0;
pub const B_WRITE_AREA: u32 = 1 << 1;
pub const B_EXECUTE_AREA: u32 = 1 << 2;
pub const B_STACK_AREA: u32 = 1 << 3;
pub const B_CLONEABLE_AREA: u32 = 1 << 8;

pub const B_CAN_INTERRUPT: u32 = 0x01;
pub const B_CHECK_PERMISSION: u32 = 0x04;
pub const B_KILL_CAN_INTERRUPT: u32 = 0x20;
pub const B_DO_NOT_RESCHEDULE: u32 = 0x02;
pub const B_RELEASE_ALL: u32 = 0x08;
pub const B_RELEASE_IF_WAITING_ONLY: u32 = 0x10;

pub const B_CURRENT_TEAM: team_id = 0;
pub const B_SYSTEM_TEAM: team_id = 1;

pub const B_TEAM_USAGE_SELF: i32 = 0;
pub const B_TEAM_USAGE_CHILDREN: i32 = -1;

pub const B_IDLE_PRIORITY: i32 = 0;
pub const B_LOWEST_ACTIVE_PRIORITY: i32 = 1;
pub const B_LOW_PRIORITY: i32 = 5;
pub const B_NORMAL_PRIORITY: i32 = 10;
pub const B_DISPLAY_PRIORITY: i32 = 15;
pub const B_URGENT_DISPLAY_PRIORITY: i32 = 20;
pub const B_REAL_TIME_DISPLAY_PRIORITY: i32 = 100;
pub const B_URGENT_PRIORITY: i32 = 110;
pub const B_REAL_TIME_PRIORITY: i32 = 120;

pub const B_SYSTEM_TIMEBASE: i32 = 0;
pub const B_FIRST_REAL_TIME_PRIORITY: i32 = B_REAL_TIME_DISPLAY_PRIORITY;

pub const B_ONE_SHOT_ABSOLUTE_ALARM: u32 = 1;
pub const B_ONE_SHOT_RELATIVE_ALARM: u32 = 2;
pub const B_PERIODIC_ALARM: u32 = 3;

pub const B_OBJECT_TYPE_FD: u16 = 0;
pub const B_OBJECT_TYPE_SEMAPHORE: u16 = 1;
pub const B_OBJECT_TYPE_PORT: u16 = 2;
pub const B_OBJECT_TYPE_THREAD: u16 = 3;

pub const B_EVENT_READ: u16 = 0x0001;
pub const B_EVENT_WRITE: u16 = 0x0002;
pub const B_EVENT_ERROR: u16 = 0x0004;
pub const B_EVENT_PRIORITY_READ: u16 = 0x0008;
pub const B_EVENT_PRIORITY_WRITE: u16 = 0x0010;
pub const B_EVENT_HIGH_PRIORITY_READ: u16 = 0x0020;
pub const B_EVENT_HIGH_PRIORITY_WRITE: u16 = 0x0040;
pub const B_EVENT_DISCONNECTED: u16 = 0x0080;
pub const B_EVENT_ACQUIRE_SEMAPHORE: u16 = 0x0001;
pub const B_EVENT_INVALID: u16 = 0x1000;

// kernel/fs_info.h
pub const B_FS_IS_READONLY: u32 = 0x00000001;
pub const B_FS_IS_REMOVABLE: u32 = 0x00000002;
pub const B_FS_IS_PERSISTENT: u32 = 0x00000004;
pub const B_FS_IS_SHARED: u32 = 0x00000008;
pub const B_FS_HAS_MIME: u32 = 0x00010000;
pub const B_FS_HAS_ATTR: u32 = 0x00020000;
pub const B_FS_HAS_QUERY: u32 = 0x00040000;
pub const B_FS_HAS_SELF_HEALING_LINKS: u32 = 0x00080000;
pub const B_FS_HAS_ALIASES: u32 = 0x00100000;
pub const B_FS_SUPPORTS_NODE_MONITORING: u32 = 0x00200000;
pub const B_FS_SUPPORTS_MONITOR_CHILDREN: u32 = 0x00400000;

// kernel/fs_query.h
pub const B_LIVE_QUERY: u32 = 0x00000001;
pub const B_QUERY_NON_INDEXED: u32 = 0x00000002;

// kernel/fs_volume.h
pub const B_MOUNT_READ_ONLY: u32 = 1;
pub const B_MOUNT_VIRTUAL_DEVICE: u32 = 2;
pub const B_FORCE_UNMOUNT: u32 = 1;

// kernel/image.h
pub const B_FLUSH_DCACHE: u32 = 0x0001;
pub const B_FLUSH_ICACHE: u32 = 0x0004;
pub const B_INVALIDATE_DCACHE: u32 = 0x0002;
pub const B_INVALIDATE_ICACHE: u32 = 0x0008;

pub const B_SYMBOL_TYPE_DATA: i32 = 0x1;
pub const B_SYMBOL_TYPE_TEXT: i32 = 0x2;
pub const B_SYMBOL_TYPE_ANY: i32 = 0x5;

// storage/StorageDefs.h
pub const B_DEV_NAME_LENGTH: usize = 128;
pub const B_FILE_NAME_LENGTH: usize = crate::FILENAME_MAX as usize;
pub const B_PATH_NAME_LENGTH: usize = crate::PATH_MAX as usize;
pub const B_ATTR_NAME_LENGTH: usize = B_FILE_NAME_LENGTH - 1;
pub const B_MIME_TYPE_LENGTH: usize = B_ATTR_NAME_LENGTH - 15;
pub const B_MAX_SYMLINKS: usize = 16;

// Haiku open modes in BFile are passed as u32
pub const B_READ_ONLY: u32 = crate::O_RDONLY as u32;
pub const B_WRITE_ONLY: u32 = crate::O_WRONLY as u32;
pub const B_READ_WRITE: u32 = crate::O_RDWR as u32;

pub const B_FAIL_IF_EXISTS: u32 = crate::O_EXCL as u32;
pub const B_CREATE_FILE: u32 = crate::O_CREAT as u32;
pub const B_ERASE_FILE: u32 = crate::O_TRUNC as u32;
pub const B_OPEN_AT_END: u32 = crate::O_APPEND as u32;

pub const B_FILE_NODE: u32 = 0x01;
pub const B_SYMLINK_NODE: u32 = 0x02;
pub const B_DIRECTORY_NODE: u32 = 0x04;
pub const B_ANY_NODE: u32 = 0x07;

// support/Errors.h
pub const B_GENERAL_ERROR_BASE: status_t = core::i32::MIN;
pub const B_OS_ERROR_BASE: status_t = B_GENERAL_ERROR_BASE + 0x1000;
pub const B_APP_ERROR_BASE: status_t = B_GENERAL_ERROR_BASE + 0x2000;
pub const B_INTERFACE_ERROR_BASE: status_t = B_GENERAL_ERROR_BASE + 0x3000;
pub const B_MEDIA_ERROR_BASE: status_t = B_GENERAL_ERROR_BASE + 0x4000;
pub const B_TRANSLATION_ERROR_BASE: status_t = B_GENERAL_ERROR_BASE + 0x4800;
pub const B_MIDI_ERROR_BASE: status_t = B_GENERAL_ERROR_BASE + 0x5000;
pub const B_STORAGE_ERROR_BASE: status_t = B_GENERAL_ERROR_BASE + 0x6000;
pub const B_POSIX_ERROR_BASE: status_t = B_GENERAL_ERROR_BASE + 0x7000;
pub const B_MAIL_ERROR_BASE: status_t = B_GENERAL_ERROR_BASE + 0x8000;
pub const B_PRINT_ERROR_BASE: status_t = B_GENERAL_ERROR_BASE + 0x9000;
pub const B_DEVICE_ERROR_BASE: status_t = B_GENERAL_ERROR_BASE + 0xa000;
pub const B_ERRORS_END: status_t = B_GENERAL_ERROR_BASE + 0xffff;

// General errors
pub const B_NO_MEMORY: status_t = B_GENERAL_ERROR_BASE + 0;
pub const B_IO_ERROR: status_t = B_GENERAL_ERROR_BASE + 1;
pub const B_PERMISSION_DENIED: status_t = B_GENERAL_ERROR_BASE + 2;
pub const B_BAD_INDEX: status_t = B_GENERAL_ERROR_BASE + 3;
pub const B_BAD_TYPE: status_t = B_GENERAL_ERROR_BASE + 4;
pub const B_BAD_VALUE: status_t = B_GENERAL_ERROR_BASE + 5;
pub const B_MISMATCHED_VALUES: status_t = B_GENERAL_ERROR_BASE + 6;
pub const B_NAME_NOT_FOUND: status_t = B_GENERAL_ERROR_BASE + 7;
pub const B_NAME_IN_USE: status_t = B_GENERAL_ERROR_BASE + 8;
pub const B_TIMED_OUT: status_t = B_GENERAL_ERROR_BASE + 9;
pub const B_INTERRUPTED: status_t = B_GENERAL_ERROR_BASE + 10;
pub const B_WOULD_BLOCK: status_t = B_GENERAL_ERROR_BASE + 11;
pub const B_CANCELED: status_t = B_GENERAL_ERROR_BASE + 12;
pub const B_NO_INIT: status_t = B_GENERAL_ERROR_BASE + 13;
pub const B_NOT_INITIALIZED: status_t = B_GENERAL_ERROR_BASE + 13;
pub const B_BUSY: status_t = B_GENERAL_ERROR_BASE + 14;
pub const B_NOT_ALLOWED: status_t = B_GENERAL_ERROR_BASE + 15;
pub const B_BAD_DATA: status_t = B_GENERAL_ERROR_BASE + 16;
pub const B_DONT_DO_THAT: status_t = B_GENERAL_ERROR_BASE + 17;

pub const B_ERROR: status_t = -1;
pub const B_OK: status_t = 0;
pub const B_NO_ERROR: status_t = 0;

// Kernel kit errors
pub const B_BAD_SEM_ID: status_t = B_OS_ERROR_BASE + 0;
pub const B_NO_MORE_SEMS: status_t = B_OS_ERROR_BASE + 1;

pub const B_BAD_THREAD_ID: status_t = B_OS_ERROR_BASE + 0x100;
pub const B_NO_MORE_THREADS: status_t = B_OS_ERROR_BASE + 0x101;
pub const B_BAD_THREAD_STATE: status_t = B_OS_ERROR_BASE + 0x102;
pub const B_BAD_TEAM_ID: status_t = B_OS_ERROR_BASE + 0x103;
pub const B_NO_MORE_TEAMS: status_t = B_OS_ERROR_BASE + 0x104;

pub const B_BAD_PORT_ID: status_t = B_OS_ERROR_BASE + 0x200;
pub const B_NO_MORE_PORTS: status_t = B_OS_ERROR_BASE + 0x201;

pub const B_BAD_IMAGE_ID: status_t = B_OS_ERROR_BASE + 0x300;
pub const B_BAD_ADDRESS: status_t = B_OS_ERROR_BASE + 0x301;
pub const B_NOT_AN_EXECUTABLE: status_t = B_OS_ERROR_BASE + 0x302;
pub const B_MISSING_LIBRARY: status_t = B_OS_ERROR_BASE + 0x303;
pub const B_MISSING_SYMBOL: status_t = B_OS_ERROR_BASE + 0x304;
pub const B_UNKNOWN_EXECUTABLE: status_t = B_OS_ERROR_BASE + 0x305;
pub const B_LEGACY_EXECUTABLE: status_t = B_OS_ERROR_BASE + 0x306;

pub const B_DEBUGGER_ALREADY_INSTALLED: status_t = B_OS_ERROR_BASE + 0x400;

// Application kit errors
pub const B_BAD_REPLY: status_t = B_APP_ERROR_BASE + 0;
pub const B_DUPLICATE_REPLY: status_t = B_APP_ERROR_BASE + 1;
pub const B_MESSAGE_TO_SELF: status_t = B_APP_ERROR_BASE + 2;
pub const B_BAD_HANDLER: status_t = B_APP_ERROR_BASE + 3;
pub const B_ALREADY_RUNNING: status_t = B_APP_ERROR_BASE + 4;
pub const B_LAUNCH_FAILED: status_t = B_APP_ERROR_BASE + 5;
pub const B_AMBIGUOUS_APP_LAUNCH: status_t = B_APP_ERROR_BASE + 6;
pub const B_UNKNOWN_MIME_TYPE: status_t = B_APP_ERROR_BASE + 7;
pub const B_BAD_SCRIPT_SYNTAX: status_t = B_APP_ERROR_BASE + 8;
pub const B_LAUNCH_FAILED_NO_RESOLVE_LINK: status_t = B_APP_ERROR_BASE + 9;
pub const B_LAUNCH_FAILED_EXECUTABLE: status_t = B_APP_ERROR_BASE + 10;
pub const B_LAUNCH_FAILED_APP_NOT_FOUND: status_t = B_APP_ERROR_BASE + 11;
pub const B_LAUNCH_FAILED_APP_IN_TRASH: status_t = B_APP_ERROR_BASE + 12;
pub const B_LAUNCH_FAILED_NO_PREFERRED_APP: status_t = B_APP_ERROR_BASE + 13;
pub const B_LAUNCH_FAILED_FILES_APP_NOT_FOUND: status_t = B_APP_ERROR_BASE + 14;
pub const B_BAD_MIME_SNIFFER_RULE: status_t = B_APP_ERROR_BASE + 15;
pub const B_NOT_A_MESSAGE: status_t = B_APP_ERROR_BASE + 16;
pub const B_SHUTDOWN_CANCELLED: status_t = B_APP_ERROR_BASE + 17;
pub const B_SHUTTING_DOWN: status_t = B_APP_ERROR_BASE + 18;

// Storage kit errors
pub const B_FILE_ERROR: status_t = B_STORAGE_ERROR_BASE + 0;
pub const B_FILE_EXISTS: status_t = B_STORAGE_ERROR_BASE + 2;
pub const B_ENTRY_NOT_FOUND: status_t = B_STORAGE_ERROR_BASE + 3;
pub const B_NAME_TOO_LONG: status_t = B_STORAGE_ERROR_BASE + 4;
pub const B_NOT_A_DIRECTORY: status_t = B_STORAGE_ERROR_BASE + 5;
pub const B_DIRECTORY_NOT_EMPTY: status_t = B_STORAGE_ERROR_BASE + 6;
pub const B_DEVICE_FULL: status_t = B_STORAGE_ERROR_BASE + 7;
pub const B_READ_ONLY_DEVICE: status_t = B_STORAGE_ERROR_BASE + 8;
pub const B_IS_A_DIRECTORY: status_t = B_STORAGE_ERROR_BASE + 9;
pub const B_NO_MORE_FDS: status_t = B_STORAGE_ERROR_BASE + 10;
pub const B_CROSS_DEVICE_LINK: status_t = B_STORAGE_ERROR_BASE + 11;
pub const B_LINK_LIMIT: status_t = B_STORAGE_ERROR_BASE + 12;
pub const B_BUSTED_PIPE: status_t = B_STORAGE_ERROR_BASE + 13;
pub const B_UNSUPPORTED: status_t = B_STORAGE_ERROR_BASE + 14;
pub const B_PARTITION_TOO_SMALL: status_t = B_STORAGE_ERROR_BASE + 15;
pub const B_PARTIAL_READ: status_t = B_STORAGE_ERROR_BASE + 16;
pub const B_PARTIAL_WRITE: status_t = B_STORAGE_ERROR_BASE + 17;

// Mapped posix errors
pub const B_BUFFER_OVERFLOW: status_t = crate::EOVERFLOW;
pub const B_TOO_MANY_ARGS: status_t = crate::E2BIG;
pub const B_FILE_TOO_LARGE: status_t = crate::EFBIG;
pub const B_RESULT_NOT_REPRESENTABLE: status_t = crate::ERANGE;
pub const B_DEVICE_NOT_FOUND: status_t = crate::ENODEV;
pub const B_NOT_SUPPORTED: status_t = crate::EOPNOTSUPP;

// Media kit errors
pub const B_STREAM_NOT_FOUND: status_t = B_MEDIA_ERROR_BASE + 0;
pub const B_SERVER_NOT_FOUND: status_t = B_MEDIA_ERROR_BASE + 1;
pub const B_RESOURCE_NOT_FOUND: status_t = B_MEDIA_ERROR_BASE + 2;
pub const B_RESOURCE_UNAVAILABLE: status_t = B_MEDIA_ERROR_BASE + 3;
pub const B_BAD_SUBSCRIBER: status_t = B_MEDIA_ERROR_BASE + 4;
pub const B_SUBSCRIBER_NOT_ENTERED: status_t = B_MEDIA_ERROR_BASE + 5;
pub const B_BUFFER_NOT_AVAILABLE: status_t = B_MEDIA_ERROR_BASE + 6;
pub const B_LAST_BUFFER_ERROR: status_t = B_MEDIA_ERROR_BASE + 7;

pub const B_MEDIA_SYSTEM_FAILURE: status_t = B_MEDIA_ERROR_BASE + 100;
pub const B_MEDIA_BAD_NODE: status_t = B_MEDIA_ERROR_BASE + 101;
pub const B_MEDIA_NODE_BUSY: status_t = B_MEDIA_ERROR_BASE + 102;
pub const B_MEDIA_BAD_FORMAT: status_t = B_MEDIA_ERROR_BASE + 103;
pub const B_MEDIA_BAD_BUFFER: status_t = B_MEDIA_ERROR_BASE + 104;
pub const B_MEDIA_TOO_MANY_NODES: status_t = B_MEDIA_ERROR_BASE + 105;
pub const B_MEDIA_TOO_MANY_BUFFERS: status_t = B_MEDIA_ERROR_BASE + 106;
pub const B_MEDIA_NODE_ALREADY_EXISTS: status_t = B_MEDIA_ERROR_BASE + 107;
pub const B_MEDIA_BUFFER_ALREADY_EXISTS: status_t = B_MEDIA_ERROR_BASE + 108;
pub const B_MEDIA_CANNOT_SEEK: status_t = B_MEDIA_ERROR_BASE + 109;
pub const B_MEDIA_CANNOT_CHANGE_RUN_MODE: status_t = B_MEDIA_ERROR_BASE + 110;
pub const B_MEDIA_APP_ALREADY_REGISTERED: status_t = B_MEDIA_ERROR_BASE + 111;
pub const B_MEDIA_APP_NOT_REGISTERED: status_t = B_MEDIA_ERROR_BASE + 112;
pub const B_MEDIA_CANNOT_RECLAIM_BUFFERS: status_t = B_MEDIA_ERROR_BASE + 113;
pub const B_MEDIA_BUFFERS_NOT_RECLAIMED: status_t = B_MEDIA_ERROR_BASE + 114;
pub const B_MEDIA_TIME_SOURCE_STOPPED: status_t = B_MEDIA_ERROR_BASE + 115;
pub const B_MEDIA_TIME_SOURCE_BUSY: status_t = B_MEDIA_ERROR_BASE + 116;
pub const B_MEDIA_BAD_SOURCE: status_t = B_MEDIA_ERROR_BASE + 117;
pub const B_MEDIA_BAD_DESTINATION: status_t = B_MEDIA_ERROR_BASE + 118;
pub const B_MEDIA_ALREADY_CONNECTED: status_t = B_MEDIA_ERROR_BASE + 119;
pub const B_MEDIA_NOT_CONNECTED: status_t = B_MEDIA_ERROR_BASE + 120;
pub const B_MEDIA_BAD_CLIP_FORMAT: status_t = B_MEDIA_ERROR_BASE + 121;
pub const B_MEDIA_ADDON_FAILED: status_t = B_MEDIA_ERROR_BASE + 122;
pub const B_MEDIA_ADDON_DISABLED: status_t = B_MEDIA_ERROR_BASE + 123;
pub const B_MEDIA_CHANGE_IN_PROGRESS: status_t = B_MEDIA_ERROR_BASE + 124;
pub const B_MEDIA_STALE_CHANGE_COUNT: status_t = B_MEDIA_ERROR_BASE + 125;
pub const B_MEDIA_ADDON_RESTRICTED: status_t = B_MEDIA_ERROR_BASE + 126;
pub const B_MEDIA_NO_HANDLER: status_t = B_MEDIA_ERROR_BASE + 127;
pub const B_MEDIA_DUPLICATE_FORMAT: status_t = B_MEDIA_ERROR_BASE + 128;
pub const B_MEDIA_REALTIME_DISABLED: status_t = B_MEDIA_ERROR_BASE + 129;
pub const B_MEDIA_REALTIME_UNAVAILABLE: status_t = B_MEDIA_ERROR_BASE + 130;

// Mail kit errors
pub const B_MAIL_NO_DAEMON: status_t = B_MAIL_ERROR_BASE + 0;
pub const B_MAIL_UNKNOWN_USER: status_t = B_MAIL_ERROR_BASE + 1;
pub const B_MAIL_WRONG_PASSWORD: status_t = B_MAIL_ERROR_BASE + 2;
pub const B_MAIL_UNKNOWN_HOST: status_t = B_MAIL_ERROR_BASE + 3;
pub const B_MAIL_ACCESS_ERROR: status_t = B_MAIL_ERROR_BASE + 4;
pub const B_MAIL_UNKNOWN_FIELD: status_t = B_MAIL_ERROR_BASE + 5;
pub const B_MAIL_NO_RECIPIENT: status_t = B_MAIL_ERROR_BASE + 6;
pub const B_MAIL_INVALID_MAIL: status_t = B_MAIL_ERROR_BASE + 7;

// Print kit errors
pub const B_NO_PRINT_SERVER: status_t = B_PRINT_ERROR_BASE + 0;

// Device kit errors
pub const B_DEV_INVALID_IOCTL: status_t = B_DEVICE_ERROR_BASE + 0;
pub const B_DEV_NO_MEMORY: status_t = B_DEVICE_ERROR_BASE + 1;
pub const B_DEV_BAD_DRIVE_NUM: status_t = B_DEVICE_ERROR_BASE + 2;
pub const B_DEV_NO_MEDIA: status_t = B_DEVICE_ERROR_BASE + 3;
pub const B_DEV_UNREADABLE: status_t = B_DEVICE_ERROR_BASE + 4;
pub const B_DEV_FORMAT_ERROR: status_t = B_DEVICE_ERROR_BASE + 5;
pub const B_DEV_TIMEOUT: status_t = B_DEVICE_ERROR_BASE + 6;
pub const B_DEV_RECALIBRATE_ERROR: status_t = B_DEVICE_ERROR_BASE + 7;
pub const B_DEV_SEEK_ERROR: status_t = B_DEVICE_ERROR_BASE + 8;
pub const B_DEV_ID_ERROR: status_t = B_DEVICE_ERROR_BASE + 9;
pub const B_DEV_READ_ERROR: status_t = B_DEVICE_ERROR_BASE + 10;
pub const B_DEV_WRITE_ERROR: status_t = B_DEVICE_ERROR_BASE + 11;
pub const B_DEV_NOT_READY: status_t = B_DEVICE_ERROR_BASE + 12;
pub const B_DEV_MEDIA_CHANGED: status_t = B_DEVICE_ERROR_BASE + 13;
pub const B_DEV_MEDIA_CHANGE_REQUESTED: status_t = B_DEVICE_ERROR_BASE + 14;
pub const B_DEV_RESOURCE_CONFLICT: status_t = B_DEVICE_ERROR_BASE + 15;
pub const B_DEV_CONFIGURATION_ERROR: status_t = B_DEVICE_ERROR_BASE + 16;
pub const B_DEV_DISABLED_BY_USER: status_t = B_DEVICE_ERROR_BASE + 17;
pub const B_DEV_DOOR_OPEN: status_t = B_DEVICE_ERROR_BASE + 18;

pub const B_DEV_INVALID_PIPE: status_t = B_DEVICE_ERROR_BASE + 19;
pub const B_DEV_CRC_ERROR: status_t = B_DEVICE_ERROR_BASE + 20;
pub const B_DEV_STALLED: status_t = B_DEVICE_ERROR_BASE + 21;
pub const B_DEV_BAD_PID: status_t = B_DEVICE_ERROR_BASE + 22;
pub const B_DEV_UNEXPECTED_PID: status_t = B_DEVICE_ERROR_BASE + 23;
pub const B_DEV_DATA_OVERRUN: status_t = B_DEVICE_ERROR_BASE + 24;
pub const B_DEV_DATA_UNDERRUN: status_t = B_DEVICE_ERROR_BASE + 25;
pub const B_DEV_FIFO_OVERRUN: status_t = B_DEVICE_ERROR_BASE + 26;
pub const B_DEV_FIFO_UNDERRUN: status_t = B_DEVICE_ERROR_BASE + 27;
pub const B_DEV_PENDING: status_t = B_DEVICE_ERROR_BASE + 28;
pub const B_DEV_MULTIPLE_ERRORS: status_t = B_DEVICE_ERROR_BASE + 29;
pub const B_DEV_TOO_LATE: status_t = B_DEVICE_ERROR_BASE + 30;

// translation kit errors
pub const B_TRANSLATION_BASE_ERROR: status_t = B_TRANSLATION_ERROR_BASE + 0;
pub const B_NO_TRANSLATOR: status_t = B_TRANSLATION_ERROR_BASE + 1;
pub const B_ILLEGAL_DATA: status_t = B_TRANSLATION_ERROR_BASE + 2;

// support/TypeConstants.h
pub const B_AFFINE_TRANSFORM_TYPE: u32 = haiku_constant!('A', 'M', 'T', 'X');
pub const B_ALIGNMENT_TYPE: u32 = haiku_constant!('A', 'L', 'G', 'N');
pub const B_ANY_TYPE: u32 = haiku_constant!('A', 'N', 'Y', 'T');
pub const B_ATOM_TYPE: u32 = haiku_constant!('A', 'T', 'O', 'M');
pub const B_ATOMREF_TYPE: u32 = haiku_constant!('A', 'T', 'M', 'R');
pub const B_BOOL_TYPE: u32 = haiku_constant!('B', 'O', 'O', 'L');
pub const B_CHAR_TYPE: u32 = haiku_constant!('C', 'H', 'A', 'R');
pub const B_COLOR_8_BIT_TYPE: u32 = haiku_constant!('C', 'L', 'R', 'B');
pub const B_DOUBLE_TYPE: u32 = haiku_constant!('D', 'B', 'L', 'E');
pub const B_FLOAT_TYPE: u32 = haiku_constant!('F', 'L', 'O', 'T');
pub const B_GRAYSCALE_8_BIT_TYPE: u32 = haiku_constant!('G', 'R', 'Y', 'B');
pub const B_INT16_TYPE: u32 = haiku_constant!('S', 'H', 'R', 'T');
pub const B_INT32_TYPE: u32 = haiku_constant!('L', 'O', 'N', 'G');
pub const B_INT64_TYPE: u32 = haiku_constant!('L', 'L', 'N', 'G');
pub const B_INT8_TYPE: u32 = haiku_constant!('B', 'Y', 'T', 'E');
pub const B_LARGE_ICON_TYPE: u32 = haiku_constant!('I', 'C', 'O', 'N');
pub const B_MEDIA_PARAMETER_GROUP_TYPE: u32 = haiku_constant!('B', 'M', 'C', 'G');
pub const B_MEDIA_PARAMETER_TYPE: u32 = haiku_constant!('B', 'M', 'C', 'T');
pub const B_MEDIA_PARAMETER_WEB_TYPE: u32 = haiku_constant!('B', 'M', 'C', 'W');
pub const B_MESSAGE_TYPE: u32 = haiku_constant!('M', 'S', 'G', 'G');
pub const B_MESSENGER_TYPE: u32 = haiku_constant!('M', 'S', 'N', 'G');
pub const B_MIME_TYPE: u32 = haiku_constant!('M', 'I', 'M', 'E');
pub const B_MINI_ICON_TYPE: u32 = haiku_constant!('M', 'I', 'C', 'N');
pub const B_MONOCHROME_1_BIT_TYPE: u32 = haiku_constant!('M', 'N', 'O', 'B');
pub const B_OBJECT_TYPE: u32 = haiku_constant!('O', 'P', 'T', 'R');
pub const B_OFF_T_TYPE: u32 = haiku_constant!('O', 'F', 'F', 'T');
pub const B_PATTERN_TYPE: u32 = haiku_constant!('P', 'A', 'T', 'N');
pub const B_POINTER_TYPE: u32 = haiku_constant!('P', 'N', 'T', 'R');
pub const B_POINT_TYPE: u32 = haiku_constant!('B', 'P', 'N', 'T');
pub const B_PROPERTY_INFO_TYPE: u32 = haiku_constant!('S', 'C', 'T', 'D');
pub const B_RAW_TYPE: u32 = haiku_constant!('R', 'A', 'W', 'T');
pub const B_RECT_TYPE: u32 = haiku_constant!('R', 'E', 'C', 'T');
pub const B_REF_TYPE: u32 = haiku_constant!('R', 'R', 'E', 'F');
pub const B_RGB_32_BIT_TYPE: u32 = haiku_constant!('R', 'G', 'B', 'B');
pub const B_RGB_COLOR_TYPE: u32 = haiku_constant!('R', 'G', 'B', 'C');
pub const B_SIZE_TYPE: u32 = haiku_constant!('S', 'I', 'Z', 'E');
pub const B_SIZE_T_TYPE: u32 = haiku_constant!('S', 'I', 'Z', 'T');
pub const B_SSIZE_T_TYPE: u32 = haiku_constant!('S', 'S', 'Z', 'T');
pub const B_STRING_TYPE: u32 = haiku_constant!('C', 'S', 'T', 'R');
pub const B_STRING_LIST_TYPE: u32 = haiku_constant!('S', 'T', 'R', 'L');
pub const B_TIME_TYPE: u32 = haiku_constant!('T', 'I', 'M', 'E');
pub const B_UINT16_TYPE: u32 = haiku_constant!('U', 'S', 'H', 'T');
pub const B_UINT32_TYPE: u32 = haiku_constant!('U', 'L', 'N', 'G');
pub const B_UINT64_TYPE: u32 = haiku_constant!('U', 'L', 'L', 'G');
pub const B_UINT8_TYPE: u32 = haiku_constant!('U', 'B', 'Y', 'T');
pub const B_VECTOR_ICON_TYPE: u32 = haiku_constant!('V', 'I', 'C', 'N');
pub const B_XATTR_TYPE: u32 = haiku_constant!('X', 'A', 'T', 'R');
pub const B_NETWORK_ADDRESS_TYPE: u32 = haiku_constant!('N', 'W', 'A', 'D');
pub const B_MIME_STRING_TYPE: u32 = haiku_constant!('M', 'I', 'M', 'S');
pub const B_ASCII_TYPE: u32 = haiku_constant!('T', 'E', 'X', 'T');
pub const B_APP_IMAGE_SYMBOL: *const c_void = core::ptr::null();

extern "C" {
    // kernel/OS.h
    pub fn create_area(
        name: *const c_char,
        startAddress: *mut *mut c_void,
        addressSpec: u32,
        size: usize,
        lock: u32,
        protection: u32,
    ) -> area_id;
    pub fn clone_area(
        name: *const c_char,
        destAddress: *mut *mut c_void,
        addressSpec: u32,
        protection: u32,
        source: area_id,
    ) -> area_id;
    pub fn find_area(name: *const c_char) -> area_id;
    pub fn area_for(address: *mut c_void) -> area_id;
    pub fn delete_area(id: area_id) -> status_t;
    pub fn resize_area(id: area_id, newSize: usize) -> status_t;
    pub fn set_area_protection(id: area_id, newProtection: u32) -> status_t;
    pub fn _get_area_info(id: area_id, areaInfo: *mut area_info, size: usize) -> status_t;
    pub fn _get_next_area_info(
        team: team_id,
        cookie: *mut isize,
        areaInfo: *mut area_info,
        size: usize,
    ) -> status_t;

    pub fn create_port(capacity: i32, name: *const c_char) -> port_id;
    pub fn find_port(name: *const c_char) -> port_id;
    pub fn read_port(
        port: port_id,
        code: *mut i32,
        buffer: *mut c_void,
        bufferSize: size_t,
    ) -> ssize_t;
    pub fn read_port_etc(
        port: port_id,
        code: *mut i32,
        buffer: *mut c_void,
        bufferSize: size_t,
        flags: u32,
        timeout: bigtime_t,
    ) -> ssize_t;
    pub fn write_port(
        port: port_id,
        code: i32,
        buffer: *const c_void,
        bufferSize: size_t,
    ) -> status_t;
    pub fn write_port_etc(
        port: port_id,
        code: i32,
        buffer: *const c_void,
        bufferSize: size_t,
        flags: u32,
        timeout: bigtime_t,
    ) -> status_t;
    pub fn close_port(port: port_id) -> status_t;
    pub fn delete_port(port: port_id) -> status_t;
    pub fn port_buffer_size(port: port_id) -> ssize_t;
    pub fn port_buffer_size_etc(port: port_id, flags: u32, timeout: bigtime_t) -> ssize_t;
    pub fn port_count(port: port_id) -> ssize_t;
    pub fn set_port_owner(port: port_id, team: team_id) -> status_t;

    pub fn _get_port_info(port: port_id, buf: *mut port_info, portInfoSize: size_t) -> status_t;
    pub fn _get_next_port_info(
        port: port_id,
        cookie: *mut i32,
        portInfo: *mut port_info,
        portInfoSize: size_t,
    ) -> status_t;
    pub fn _get_port_message_info_etc(
        port: port_id,
        info: *mut port_message_info,
        infoSize: size_t,
        flags: u32,
        timeout: bigtime_t,
    ) -> status_t;

    pub fn create_sem(count: i32, name: *const c_char) -> sem_id;
    pub fn delete_sem(id: sem_id) -> status_t;
    pub fn acquire_sem(id: sem_id) -> status_t;
    pub fn acquire_sem_etc(id: sem_id, count: i32, flags: u32, timeout: bigtime_t) -> status_t;
    pub fn release_sem(id: sem_id) -> status_t;
    pub fn release_sem_etc(id: sem_id, count: i32, flags: u32) -> status_t;
    pub fn switch_sem(semToBeReleased: sem_id, id: sem_id) -> status_t;
    pub fn switch_sem_etc(
        semToBeReleased: sem_id,
        id: sem_id,
        count: i32,
        flags: u32,
        timeout: bigtime_t,
    ) -> status_t;
    pub fn get_sem_count(id: sem_id, threadCount: *mut i32) -> status_t;
    pub fn set_sem_owner(id: sem_id, team: team_id) -> status_t;
    pub fn _get_sem_info(id: sem_id, info: *mut sem_info, infoSize: size_t) -> status_t;
    pub fn _get_next_sem_info(
        team: team_id,
        cookie: *mut i32,
        info: *mut sem_info,
        infoSize: size_t,
    ) -> status_t;

    pub fn kill_team(team: team_id) -> status_t;
    pub fn _get_team_info(team: team_id, info: *mut team_info, size: size_t) -> status_t;
    pub fn _get_next_team_info(cookie: *mut i32, info: *mut team_info, size: size_t) -> status_t;

    pub fn spawn_thread(
        func: thread_func,
        name: *const c_char,
        priority: i32,
        data: *mut c_void,
    ) -> thread_id;
    pub fn kill_thread(thread: thread_id) -> status_t;
    pub fn resume_thread(thread: thread_id) -> status_t;
    pub fn suspend_thread(thread: thread_id) -> status_t;

    pub fn rename_thread(thread: thread_id, newName: *const c_char) -> status_t;
    pub fn set_thread_priority(thread: thread_id, newPriority: i32) -> status_t;
    pub fn suggest_thread_priority(
        what: u32,
        period: i32,
        jitter: crate::bigtime_t,
        length: crate::bigtime_t,
    ) -> i32;
    pub fn estimate_max_scheduling_latency(th: crate::thread_id) -> crate::bigtime_t;
    pub fn exit_thread(status: status_t);
    pub fn wait_for_thread(thread: thread_id, returnValue: *mut status_t) -> status_t;
    pub fn on_exit_thread(callback: extern "C" fn(*mut c_void), data: *mut c_void) -> status_t;

    pub fn find_thread(name: *const c_char) -> thread_id;

    pub fn get_scheduler_mode() -> i32;
    pub fn set_scheduler_mode(mode: i32) -> status_t;

    pub fn send_data(
        thread: thread_id,
        code: i32,
        buffer: *const c_void,
        bufferSize: size_t,
    ) -> status_t;
    pub fn receive_data(sender: *mut thread_id, buffer: *mut c_void, bufferSize: size_t) -> i32;
    pub fn has_data(thread: thread_id) -> bool;

    pub fn snooze(amount: bigtime_t) -> status_t;
    pub fn snooze_etc(amount: bigtime_t, timeBase: c_int, flags: u32) -> status_t;
    pub fn snooze_until(time: bigtime_t, timeBase: c_int) -> status_t;

    pub fn _get_thread_info(id: thread_id, info: *mut thread_info, size: size_t) -> status_t;
    pub fn _get_next_thread_info(
        team: team_id,
        cookie: *mut i32,
        info: *mut thread_info,
        size: size_t,
    ) -> status_t;

    pub fn get_pthread_thread_id(thread: crate::pthread_t) -> thread_id;

    pub fn _get_team_usage_info(
        team: team_id,
        who: i32,
        info: *mut team_usage_info,
        size: size_t,
    ) -> status_t;

    pub fn real_time_clock() -> c_ulong;
    pub fn set_real_time_clock(secsSinceJan1st1970: c_ulong);
    pub fn real_time_clock_usecs() -> bigtime_t;
    pub fn system_time() -> bigtime_t;
    pub fn system_time_nsecs() -> nanotime_t;
    // set_timezone() is deprecated and a no-op

    pub fn set_alarm(when: bigtime_t, flags: u32) -> bigtime_t;
    pub fn debugger(message: *const c_char);
    pub fn disable_debugger(state: c_int) -> c_int;

    pub fn get_system_info(info: *mut system_info) -> status_t;
    pub fn _get_cpu_info_etc(
        firstCPU: u32,
        cpuCount: u32,
        info: *mut cpu_info,
        size: size_t,
    ) -> status_t;
    pub fn get_cpu_topology_info(
        topologyInfos: *mut cpu_topology_node_info,
        topologyInfoCount: *mut u32,
    ) -> status_t;
    pub fn is_computer_on() -> i32;
    pub fn is_computer_on_fire() -> c_double;
    pub fn send_signal(threadID: thread_id, signal: c_uint) -> c_int;
    pub fn set_signal_stack(base: *mut c_void, size: size_t);

    pub fn wait_for_objects(infos: *mut object_wait_info, numInfos: c_int) -> ssize_t;
    pub fn wait_for_objects_etc(
        infos: *mut object_wait_info,
        numInfos: c_int,
        flags: u32,
        timeout: bigtime_t,
    ) -> ssize_t;

    // kernel/fs_attr.h
    pub fn fs_read_attr(
        fd: c_int,
        attribute: *const c_char,
        type_: u32,
        pos: off_t,
        buffer: *mut c_void,
        readBytes: size_t,
    ) -> ssize_t;
    pub fn fs_write_attr(
        fd: c_int,
        attribute: *const c_char,
        type_: u32,
        pos: off_t,
        buffer: *const c_void,
        writeBytes: size_t,
    ) -> ssize_t;
    pub fn fs_remove_attr(fd: c_int, attribute: *const c_char) -> c_int;
    pub fn fs_stat_attr(fd: c_int, attribute: *const c_char, attrInfo: *mut attr_info) -> c_int;

    pub fn fs_open_attr(
        path: *const c_char,
        attribute: *const c_char,
        type_: u32,
        openMode: c_int,
    ) -> c_int;
    pub fn fs_fopen_attr(fd: c_int, attribute: *const c_char, type_: u32, openMode: c_int)
        -> c_int;
    pub fn fs_close_attr(fd: c_int) -> c_int;

    pub fn fs_open_attr_dir(path: *const c_char) -> *mut crate::DIR;
    pub fn fs_lopen_attr_dir(path: *const c_char) -> *mut crate::DIR;
    pub fn fs_fopen_attr_dir(fd: c_int) -> *mut crate::DIR;
    pub fn fs_close_attr_dir(dir: *mut crate::DIR) -> c_int;
    pub fn fs_read_attr_dir(dir: *mut crate::DIR) -> *mut crate::dirent;
    pub fn fs_rewind_attr_dir(dir: *mut crate::DIR);

    // kernel/fs_image.h
    pub fn fs_create_index(
        device: crate::dev_t,
        name: *const c_char,
        type_: u32,
        flags: u32,
    ) -> c_int;
    pub fn fs_remove_index(device: crate::dev_t, name: *const c_char) -> c_int;
    pub fn fs_stat_index(
        device: crate::dev_t,
        name: *const c_char,
        indexInfo: *mut index_info,
    ) -> c_int;

    pub fn fs_open_index_dir(device: crate::dev_t) -> *mut crate::DIR;
    pub fn fs_close_index_dir(indexDirectory: *mut crate::DIR) -> c_int;
    pub fn fs_read_index_dir(indexDirectory: *mut crate::DIR) -> *mut crate::dirent;
    pub fn fs_rewind_index_dir(indexDirectory: *mut crate::DIR);

    // kernel/fs_info.h
    pub fn dev_for_path(path: *const c_char) -> crate::dev_t;
    pub fn next_dev(pos: *mut i32) -> crate::dev_t;
    pub fn fs_stat_dev(dev: crate::dev_t, info: *mut fs_info) -> c_int;

    // kernel/fs_query.h
    pub fn fs_open_query(device: crate::dev_t, query: *const c_char, flags: u32)
        -> *mut crate::DIR;
    pub fn fs_open_live_query(
        device: crate::dev_t,
        query: *const c_char,
        flags: u32,
        port: port_id,
        token: i32,
    ) -> *mut crate::DIR;
    pub fn fs_close_query(d: *mut crate::DIR) -> c_int;
    pub fn fs_read_query(d: *mut crate::DIR) -> *mut crate::dirent;
    pub fn get_path_for_dirent(dent: *mut crate::dirent, buf: *mut c_char, len: size_t)
        -> status_t;

    // kernel/fs_volume.h
    pub fn fs_mount_volume(
        where_: *const c_char,
        device: *const c_char,
        filesystem: *const c_char,
        flags: u32,
        parameters: *const c_char,
    ) -> crate::dev_t;
    pub fn fs_unmount_volume(path: *const c_char, flags: u32) -> status_t;

    // kernel/image.h
    pub fn load_image(
        argc: i32,
        argv: *mut *const c_char,
        environ: *mut *const c_char,
    ) -> thread_id;
    pub fn load_add_on(path: *const c_char) -> image_id;
    pub fn unload_add_on(image: image_id) -> status_t;
    pub fn get_image_symbol(
        image: image_id,
        name: *const c_char,
        symbolType: i32,
        symbolLocation: *mut *mut c_void,
    ) -> status_t;
    pub fn get_nth_image_symbol(
        image: image_id,
        n: i32,
        nameBuffer: *mut c_char,
        nameLength: *mut i32,
        symbolType: *mut i32,
        symbolLocation: *mut *mut c_void,
    ) -> status_t;
    pub fn clear_caches(address: *mut c_void, length: size_t, flags: u32);
    pub fn _get_image_info(image: image_id, info: *mut image_info, size: size_t) -> status_t;
    pub fn _get_next_image_info(
        team: team_id,
        cookie: *mut i32,
        info: *mut image_info,
        size: size_t,
    ) -> status_t;
    pub fn find_path(
        codePointer: *const c_void,
        baseDirectory: path_base_directory,
        subPath: *const c_char,
        pathBuffer: *mut c_char,
        bufferSize: usize,
    ) -> status_t;
    pub fn find_path_etc(
        codePointer: *const c_void,
        dependency: *const c_char,
        architecture: *const c_char,
        baseDirectory: path_base_directory,
        subPath: *const c_char,
        flags: u32,
        pathBuffer: *mut c_char,
        bufferSize: size_t,
    ) -> status_t;
    pub fn find_path_for_path(
        path: *const c_char,
        baseDirectory: path_base_directory,
        subPath: *const c_char,
        pathBuffer: *mut c_char,
        bufferSize: size_t,
    ) -> status_t;
    pub fn find_path_for_path_etc(
        path: *const c_char,
        dependency: *const c_char,
        architecture: *const c_char,
        baseDirectory: path_base_directory,
        subPath: *const c_char,
        flags: u32,
        pathBuffer: *mut c_char,
        bufferSize: size_t,
    ) -> status_t;
    pub fn find_paths(
        baseDirectory: path_base_directory,
        subPath: *const c_char,
        _paths: *mut *mut *mut c_char,
        pathCount: *mut size_t,
    ) -> status_t;
    pub fn find_paths_etc(
        architecture: *const c_char,
        baseDirectory: path_base_directory,
        subPath: *const c_char,
        flags: u32,
        _paths: *mut *mut *mut c_char,
        pathCount: *mut size_t,
    ) -> status_t;
    pub fn find_directory(
        which: directory_which,
        volume: crate::dev_t,
        createIt: bool,
        pathString: *mut c_char,
        length: i32,
    ) -> status_t;

    pub fn get_cpuid(info: *mut cpuid_info, eaxRegister: u32, cpuNum: u32) -> status_t;
}

// The following functions are defined as macros in C/C++
#[inline]
pub unsafe fn get_cpu_info(firstCPU: u32, cpuCount: u32, info: *mut cpu_info) -> status_t {
    _get_cpu_info_etc(firstCPU, cpuCount, info, size_of::<cpu_info>() as size_t)
}

#[inline]
pub unsafe fn get_area_info(id: area_id, info: *mut area_info) -> status_t {
    _get_area_info(id, info, size_of::<area_info>() as usize)
}

#[inline]
pub unsafe fn get_next_area_info(
    team: team_id,
    cookie: *mut isize,
    info: *mut area_info,
) -> status_t {
    _get_next_area_info(team, cookie, info, size_of::<area_info>() as usize)
}

#[inline]
pub unsafe fn get_port_info(port: port_id, buf: *mut port_info) -> status_t {
    _get_port_info(port, buf, size_of::<port_info>() as size_t)
}

#[inline]
pub unsafe fn get_next_port_info(
    port: port_id,
    cookie: *mut i32,
    portInfo: *mut port_info,
) -> status_t {
    _get_next_port_info(port, cookie, portInfo, size_of::<port_info>() as size_t)
}

#[inline]
pub unsafe fn get_port_message_info_etc(
    port: port_id,
    info: *mut port_message_info,
    flags: u32,
    timeout: bigtime_t,
) -> status_t {
    _get_port_message_info_etc(
        port,
        info,
        size_of::<port_message_info>() as size_t,
        flags,
        timeout,
    )
}

#[inline]
pub unsafe fn get_sem_info(id: sem_id, info: *mut sem_info) -> status_t {
    _get_sem_info(id, info, size_of::<sem_info>() as size_t)
}

#[inline]
pub unsafe fn get_next_sem_info(team: team_id, cookie: *mut i32, info: *mut sem_info) -> status_t {
    _get_next_sem_info(team, cookie, info, size_of::<sem_info>() as size_t)
}

#[inline]
pub unsafe fn get_team_info(team: team_id, info: *mut team_info) -> status_t {
    _get_team_info(team, info, size_of::<team_info>() as size_t)
}

#[inline]
pub unsafe fn get_next_team_info(cookie: *mut i32, info: *mut team_info) -> status_t {
    _get_next_team_info(cookie, info, size_of::<team_info>() as size_t)
}

#[inline]
pub unsafe fn get_team_usage_info(team: team_id, who: i32, info: *mut team_usage_info) -> status_t {
    _get_team_usage_info(team, who, info, size_of::<team_usage_info>() as size_t)
}

#[inline]
pub unsafe fn get_thread_info(id: thread_id, info: *mut thread_info) -> status_t {
    _get_thread_info(id, info, size_of::<thread_info>() as size_t)
}

#[inline]
pub unsafe fn get_next_thread_info(
    team: team_id,
    cookie: *mut i32,
    info: *mut thread_info,
) -> status_t {
    _get_next_thread_info(team, cookie, info, size_of::<thread_info>() as size_t)
}

// kernel/image.h
#[inline]
pub unsafe fn get_image_info(image: image_id, info: *mut image_info) -> status_t {
    _get_image_info(image, info, size_of::<image_info>() as size_t)
}

#[inline]
pub unsafe fn get_next_image_info(
    team: team_id,
    cookie: *mut i32,
    info: *mut image_info,
) -> status_t {
    _get_next_image_info(team, cookie, info, size_of::<image_info>() as size_t)
}
