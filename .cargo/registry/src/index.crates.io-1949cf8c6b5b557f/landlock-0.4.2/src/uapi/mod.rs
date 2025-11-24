// SPDX-License-Identifier: Apache-2.0 OR MIT

#[allow(dead_code)]
#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[allow(non_upper_case_globals)]
mod landlock;

#[rustfmt::skip]
pub use self::landlock::{
    landlock_net_port_attr,
    landlock_path_beneath_attr,
    landlock_rule_type,
    landlock_rule_type_LANDLOCK_RULE_NET_PORT,
    landlock_rule_type_LANDLOCK_RULE_PATH_BENEATH,
    landlock_ruleset_attr,
    LANDLOCK_ACCESS_FS_EXECUTE,
    LANDLOCK_ACCESS_FS_WRITE_FILE,
    LANDLOCK_ACCESS_FS_READ_FILE,
    LANDLOCK_ACCESS_FS_READ_DIR,
    LANDLOCK_ACCESS_FS_REMOVE_DIR,
    LANDLOCK_ACCESS_FS_REMOVE_FILE,
    LANDLOCK_ACCESS_FS_MAKE_CHAR,
    LANDLOCK_ACCESS_FS_MAKE_DIR,
    LANDLOCK_ACCESS_FS_MAKE_REG,
    LANDLOCK_ACCESS_FS_MAKE_SOCK,
    LANDLOCK_ACCESS_FS_MAKE_FIFO,
    LANDLOCK_ACCESS_FS_MAKE_BLOCK,
    LANDLOCK_ACCESS_FS_MAKE_SYM,
    LANDLOCK_ACCESS_FS_REFER,
    LANDLOCK_ACCESS_FS_TRUNCATE,
    LANDLOCK_ACCESS_FS_IOCTL_DEV,
    LANDLOCK_ACCESS_NET_BIND_TCP,
    LANDLOCK_ACCESS_NET_CONNECT_TCP,
    LANDLOCK_CREATE_RULESET_VERSION,
    LANDLOCK_SCOPE_ABSTRACT_UNIX_SOCKET,
    LANDLOCK_SCOPE_SIGNAL,
};

use libc::{
    __u32, c_int, c_void, size_t, syscall, SYS_landlock_add_rule, SYS_landlock_create_ruleset,
    SYS_landlock_restrict_self,
};

#[rustfmt::skip]
pub unsafe fn landlock_create_ruleset(attr: *const landlock_ruleset_attr, size: size_t,
                                      flags: __u32) -> c_int {
    syscall(SYS_landlock_create_ruleset, attr, size, flags) as c_int
}

#[rustfmt::skip]
pub unsafe fn landlock_add_rule(ruleset_fd: c_int, rule_type: landlock_rule_type,
                                rule_attr: *const c_void, flags: __u32) -> c_int {
    syscall(SYS_landlock_add_rule, ruleset_fd, rule_type, rule_attr, flags) as c_int
}

pub unsafe fn landlock_restrict_self(ruleset_fd: c_int, flags: __u32) -> c_int {
    syscall(SYS_landlock_restrict_self, ruleset_fd, flags) as c_int
}
