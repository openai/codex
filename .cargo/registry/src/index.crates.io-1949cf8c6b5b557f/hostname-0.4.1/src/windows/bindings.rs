#![allow(
    non_snake_case,
    non_upper_case_globals,
    non_camel_case_types,
    dead_code,
    clippy::all
)]

windows_link::link!("kernel32.dll" "system" fn GetComputerNameExW(nametype : COMPUTER_NAME_FORMAT, lpbuffer : PWSTR, nsize : *mut u32) -> BOOL);
windows_link::link!("kernel32.dll" "system" fn SetComputerNameExW(nametype : COMPUTER_NAME_FORMAT, lpbuffer : PCWSTR) -> BOOL);
pub type BOOL = i32;
pub type COMPUTER_NAME_FORMAT = i32;
pub const ComputerNamePhysicalDnsHostname: COMPUTER_NAME_FORMAT = 5i32;
pub type PCWSTR = *const u16;
pub type PWSTR = *mut u16;
