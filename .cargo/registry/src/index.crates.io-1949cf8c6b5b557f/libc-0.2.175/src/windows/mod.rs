//! Windows CRT definitions

use crate::prelude::*;

pub type intmax_t = i64;
pub type uintmax_t = u64;

pub type size_t = usize;
pub type ptrdiff_t = isize;
pub type intptr_t = isize;
pub type uintptr_t = usize;
pub type ssize_t = isize;
pub type sighandler_t = usize;

pub type wchar_t = u16;

pub type clock_t = i32;

pub type errno_t = c_int;

cfg_if! {
    if #[cfg(all(target_arch = "x86", target_env = "gnu"))] {
        pub type time_t = i32;
    } else {
        pub type time_t = i64;
    }
}

pub type off_t = i32;
pub type dev_t = u32;
pub type ino_t = u16;
#[cfg_attr(feature = "extra_traits", derive(Debug))]
pub enum timezone {}
impl Copy for timezone {}
impl Clone for timezone {
    fn clone(&self) -> timezone {
        *self
    }
}
pub type time64_t = i64;

pub type SOCKET = crate::uintptr_t;

s! {
    // note this is the struct called stat64 in Windows. Not stat, nor stati64.
    pub struct stat {
        pub st_dev: dev_t,
        pub st_ino: ino_t,
        pub st_mode: u16,
        pub st_nlink: c_short,
        pub st_uid: c_short,
        pub st_gid: c_short,
        pub st_rdev: dev_t,
        pub st_size: i64,
        pub st_atime: time64_t,
        pub st_mtime: time64_t,
        pub st_ctime: time64_t,
    }

    // note that this is called utimbuf64 in Windows
    pub struct utimbuf {
        pub actime: time64_t,
        pub modtime: time64_t,
    }

    pub struct tm {
        pub tm_sec: c_int,
        pub tm_min: c_int,
        pub tm_hour: c_int,
        pub tm_mday: c_int,
        pub tm_mon: c_int,
        pub tm_year: c_int,
        pub tm_wday: c_int,
        pub tm_yday: c_int,
        pub tm_isdst: c_int,
    }

    pub struct timeval {
        pub tv_sec: c_long,
        pub tv_usec: c_long,
    }

    pub struct timespec {
        pub tv_sec: time_t,
        pub tv_nsec: c_long,
    }

    pub struct sockaddr {
        pub sa_family: c_ushort,
        pub sa_data: [c_char; 14],
    }
}

pub const INT_MIN: c_int = -2147483648;
pub const INT_MAX: c_int = 2147483647;

pub const EXIT_FAILURE: c_int = 1;
pub const EXIT_SUCCESS: c_int = 0;
pub const RAND_MAX: c_int = 32767;
pub const EOF: c_int = -1;
pub const SEEK_SET: c_int = 0;
pub const SEEK_CUR: c_int = 1;
pub const SEEK_END: c_int = 2;
pub const _IOFBF: c_int = 0;
pub const _IONBF: c_int = 4;
pub const _IOLBF: c_int = 64;
pub const BUFSIZ: c_uint = 512;
pub const FOPEN_MAX: c_uint = 20;
pub const FILENAME_MAX: c_uint = 260;

// fcntl.h
pub const O_RDONLY: c_int = 0x0000;
pub const O_WRONLY: c_int = 0x0001;
pub const O_RDWR: c_int = 0x0002;
pub const O_APPEND: c_int = 0x0008;
pub const O_CREAT: c_int = 0x0100;
pub const O_TRUNC: c_int = 0x0200;
pub const O_EXCL: c_int = 0x0400;
pub const O_TEXT: c_int = 0x4000;
pub const O_BINARY: c_int = 0x8000;
pub const _O_WTEXT: c_int = 0x10000;
pub const _O_U16TEXT: c_int = 0x20000;
pub const _O_U8TEXT: c_int = 0x40000;
pub const O_RAW: c_int = O_BINARY;
pub const O_NOINHERIT: c_int = 0x0080;
pub const O_TEMPORARY: c_int = 0x0040;
pub const _O_SHORT_LIVED: c_int = 0x1000;
pub const _O_OBTAIN_DIR: c_int = 0x2000;
pub const O_SEQUENTIAL: c_int = 0x0020;
pub const O_RANDOM: c_int = 0x0010;

pub const S_IFCHR: c_int = 0o2_0000;
pub const S_IFDIR: c_int = 0o4_0000;
pub const S_IFREG: c_int = 0o10_0000;
pub const S_IFMT: c_int = 0o17_0000;
pub const S_IEXEC: c_int = 0o0100;
pub const S_IWRITE: c_int = 0o0200;
pub const S_IREAD: c_int = 0o0400;

pub const LC_ALL: c_int = 0;
pub const LC_COLLATE: c_int = 1;
pub const LC_CTYPE: c_int = 2;
pub const LC_MONETARY: c_int = 3;
pub const LC_NUMERIC: c_int = 4;
pub const LC_TIME: c_int = 5;

pub const EPERM: c_int = 1;
pub const ENOENT: c_int = 2;
pub const ESRCH: c_int = 3;
pub const EINTR: c_int = 4;
pub const EIO: c_int = 5;
pub const ENXIO: c_int = 6;
pub const E2BIG: c_int = 7;
pub const ENOEXEC: c_int = 8;
pub const EBADF: c_int = 9;
pub const ECHILD: c_int = 10;
pub const EAGAIN: c_int = 11;
pub const ENOMEM: c_int = 12;
pub const EACCES: c_int = 13;
pub const EFAULT: c_int = 14;
pub const EBUSY: c_int = 16;
pub const EEXIST: c_int = 17;
pub const EXDEV: c_int = 18;
pub const ENODEV: c_int = 19;
pub const ENOTDIR: c_int = 20;
pub const EISDIR: c_int = 21;
pub const EINVAL: c_int = 22;
pub const ENFILE: c_int = 23;
pub const EMFILE: c_int = 24;
pub const ENOTTY: c_int = 25;
pub const EFBIG: c_int = 27;
pub const ENOSPC: c_int = 28;
pub const ESPIPE: c_int = 29;
pub const EROFS: c_int = 30;
pub const EMLINK: c_int = 31;
pub const EPIPE: c_int = 32;
pub const EDOM: c_int = 33;
pub const ERANGE: c_int = 34;
pub const EDEADLK: c_int = 36;
pub const EDEADLOCK: c_int = 36;
pub const ENAMETOOLONG: c_int = 38;
pub const ENOLCK: c_int = 39;
pub const ENOSYS: c_int = 40;
pub const ENOTEMPTY: c_int = 41;
pub const EILSEQ: c_int = 42;
pub const STRUNCATE: c_int = 80;

// POSIX Supplement (from errno.h)
pub const EADDRINUSE: c_int = 100;
pub const EADDRNOTAVAIL: c_int = 101;
pub const EAFNOSUPPORT: c_int = 102;
pub const EALREADY: c_int = 103;
pub const EBADMSG: c_int = 104;
pub const ECANCELED: c_int = 105;
pub const ECONNABORTED: c_int = 106;
pub const ECONNREFUSED: c_int = 107;
pub const ECONNRESET: c_int = 108;
pub const EDESTADDRREQ: c_int = 109;
pub const EHOSTUNREACH: c_int = 110;
pub const EIDRM: c_int = 111;
pub const EINPROGRESS: c_int = 112;
pub const EISCONN: c_int = 113;
pub const ELOOP: c_int = 114;
pub const EMSGSIZE: c_int = 115;
pub const ENETDOWN: c_int = 116;
pub const ENETRESET: c_int = 117;
pub const ENETUNREACH: c_int = 118;
pub const ENOBUFS: c_int = 119;
pub const ENODATA: c_int = 120;
pub const ENOLINK: c_int = 121;
pub const ENOMSG: c_int = 122;
pub const ENOPROTOOPT: c_int = 123;
pub const ENOSR: c_int = 124;
pub const ENOSTR: c_int = 125;
pub const ENOTCONN: c_int = 126;
pub const ENOTRECOVERABLE: c_int = 127;
pub const ENOTSOCK: c_int = 128;
pub const ENOTSUP: c_int = 129;
pub const EOPNOTSUPP: c_int = 130;
pub const EOVERFLOW: c_int = 132;
pub const EOWNERDEAD: c_int = 133;
pub const EPROTO: c_int = 134;
pub const EPROTONOSUPPORT: c_int = 135;
pub const EPROTOTYPE: c_int = 136;
pub const ETIME: c_int = 137;
pub const ETIMEDOUT: c_int = 138;
pub const ETXTBSY: c_int = 139;
pub const EWOULDBLOCK: c_int = 140;

// signal codes
pub const SIGINT: c_int = 2;
pub const SIGILL: c_int = 4;
pub const SIGFPE: c_int = 8;
pub const SIGSEGV: c_int = 11;
pub const SIGTERM: c_int = 15;
pub const SIGABRT: c_int = 22;
pub const NSIG: c_int = 23;

pub const SIG_ERR: c_int = -1;
pub const SIG_DFL: crate::sighandler_t = 0;
pub const SIG_IGN: crate::sighandler_t = 1;
pub const SIG_GET: crate::sighandler_t = 2;
pub const SIG_SGE: crate::sighandler_t = 3;
pub const SIG_ACK: crate::sighandler_t = 4;

// DIFF(main): removed in 458c58f409
// FIXME(msrv): done by `std` starting in 1.79.0
// inline comment below appeases style checker
#[cfg(all(target_env = "msvc", feature = "rustc-dep-of-std"))] // " if "
#[link(name = "msvcrt", cfg(not(target_feature = "crt-static")))]
#[link(name = "libcmt", cfg(target_feature = "crt-static"))]
extern "C" {}

#[cfg_attr(feature = "extra_traits", derive(Debug))]
pub enum FILE {}
impl Copy for FILE {}
impl Clone for FILE {
    fn clone(&self) -> FILE {
        *self
    }
}
#[cfg_attr(feature = "extra_traits", derive(Debug))]
pub enum fpos_t {} // FIXME(windows): fill this out with a struct
impl Copy for fpos_t {}
impl Clone for fpos_t {
    fn clone(&self) -> fpos_t {
        *self
    }
}

// Special handling for all print and scan type functions because of https://github.com/rust-lang/libc/issues/2860
cfg_if! {
    if #[cfg(not(feature = "rustc-dep-of-std"))] {
        #[cfg_attr(
            all(windows, target_env = "msvc"),
            link(name = "legacy_stdio_definitions")
        )]
        extern "C" {
            pub fn printf(format: *const c_char, ...) -> c_int;
            pub fn fprintf(stream: *mut FILE, format: *const c_char, ...) -> c_int;
        }
    }
}

extern "C" {
    pub fn isalnum(c: c_int) -> c_int;
    pub fn isalpha(c: c_int) -> c_int;
    pub fn iscntrl(c: c_int) -> c_int;
    pub fn isdigit(c: c_int) -> c_int;
    pub fn isgraph(c: c_int) -> c_int;
    pub fn islower(c: c_int) -> c_int;
    pub fn isprint(c: c_int) -> c_int;
    pub fn ispunct(c: c_int) -> c_int;
    pub fn isspace(c: c_int) -> c_int;
    pub fn isupper(c: c_int) -> c_int;
    pub fn isxdigit(c: c_int) -> c_int;
    pub fn isblank(c: c_int) -> c_int;
    pub fn tolower(c: c_int) -> c_int;
    pub fn toupper(c: c_int) -> c_int;
    pub fn fopen(filename: *const c_char, mode: *const c_char) -> *mut FILE;
    pub fn freopen(filename: *const c_char, mode: *const c_char, file: *mut FILE) -> *mut FILE;
    pub fn fflush(file: *mut FILE) -> c_int;
    pub fn fclose(file: *mut FILE) -> c_int;
    pub fn remove(filename: *const c_char) -> c_int;
    pub fn rename(oldname: *const c_char, newname: *const c_char) -> c_int;
    pub fn tmpfile() -> *mut FILE;
    pub fn setvbuf(stream: *mut FILE, buffer: *mut c_char, mode: c_int, size: size_t) -> c_int;
    pub fn setbuf(stream: *mut FILE, buf: *mut c_char);
    pub fn getchar() -> c_int;
    pub fn putchar(c: c_int) -> c_int;
    pub fn fgetc(stream: *mut FILE) -> c_int;
    pub fn fgets(buf: *mut c_char, n: c_int, stream: *mut FILE) -> *mut c_char;
    pub fn fputc(c: c_int, stream: *mut FILE) -> c_int;
    pub fn fputs(s: *const c_char, stream: *mut FILE) -> c_int;
    pub fn puts(s: *const c_char) -> c_int;
    pub fn ungetc(c: c_int, stream: *mut FILE) -> c_int;
    pub fn fread(ptr: *mut c_void, size: size_t, nobj: size_t, stream: *mut FILE) -> size_t;
    pub fn fwrite(ptr: *const c_void, size: size_t, nobj: size_t, stream: *mut FILE) -> size_t;
    pub fn fseek(stream: *mut FILE, offset: c_long, whence: c_int) -> c_int;
    pub fn ftell(stream: *mut FILE) -> c_long;
    pub fn rewind(stream: *mut FILE);
    pub fn fgetpos(stream: *mut FILE, ptr: *mut fpos_t) -> c_int;
    pub fn fsetpos(stream: *mut FILE, ptr: *const fpos_t) -> c_int;
    pub fn feof(stream: *mut FILE) -> c_int;
    pub fn ferror(stream: *mut FILE) -> c_int;
    pub fn perror(s: *const c_char);
    pub fn atof(s: *const c_char) -> c_double;
    pub fn atoi(s: *const c_char) -> c_int;
    pub fn atol(s: *const c_char) -> c_long;
    pub fn atoll(s: *const c_char) -> c_longlong;
    pub fn strtod(s: *const c_char, endp: *mut *mut c_char) -> c_double;
    pub fn strtof(s: *const c_char, endp: *mut *mut c_char) -> c_float;
    pub fn strtol(s: *const c_char, endp: *mut *mut c_char, base: c_int) -> c_long;
    pub fn strtoll(s: *const c_char, endp: *mut *mut c_char, base: c_int) -> c_longlong;
    pub fn strtoul(s: *const c_char, endp: *mut *mut c_char, base: c_int) -> c_ulong;
    pub fn strtoull(s: *const c_char, endp: *mut *mut c_char, base: c_int) -> c_ulonglong;
    pub fn calloc(nobj: size_t, size: size_t) -> *mut c_void;
    pub fn malloc(size: size_t) -> *mut c_void;
    pub fn _msize(p: *mut c_void) -> size_t;
    pub fn realloc(p: *mut c_void, size: size_t) -> *mut c_void;
    pub fn free(p: *mut c_void);
    pub fn abort() -> !;
    pub fn exit(status: c_int) -> !;
    pub fn _exit(status: c_int) -> !;
    pub fn atexit(cb: extern "C" fn()) -> c_int;
    pub fn system(s: *const c_char) -> c_int;
    pub fn getenv(s: *const c_char) -> *mut c_char;

    pub fn strcpy(dst: *mut c_char, src: *const c_char) -> *mut c_char;
    pub fn strncpy(dst: *mut c_char, src: *const c_char, n: size_t) -> *mut c_char;
    pub fn strcat(s: *mut c_char, ct: *const c_char) -> *mut c_char;
    pub fn strncat(s: *mut c_char, ct: *const c_char, n: size_t) -> *mut c_char;
    pub fn strcmp(cs: *const c_char, ct: *const c_char) -> c_int;
    pub fn strncmp(cs: *const c_char, ct: *const c_char, n: size_t) -> c_int;
    pub fn strcoll(cs: *const c_char, ct: *const c_char) -> c_int;
    pub fn strchr(cs: *const c_char, c: c_int) -> *mut c_char;
    pub fn strrchr(cs: *const c_char, c: c_int) -> *mut c_char;
    pub fn strspn(cs: *const c_char, ct: *const c_char) -> size_t;
    pub fn strcspn(cs: *const c_char, ct: *const c_char) -> size_t;
    pub fn strdup(cs: *const c_char) -> *mut c_char;
    pub fn strpbrk(cs: *const c_char, ct: *const c_char) -> *mut c_char;
    pub fn strstr(cs: *const c_char, ct: *const c_char) -> *mut c_char;
    pub fn strlen(cs: *const c_char) -> size_t;
    pub fn strnlen(cs: *const c_char, maxlen: size_t) -> size_t;
    pub fn strerror(n: c_int) -> *mut c_char;
    pub fn strtok(s: *mut c_char, t: *const c_char) -> *mut c_char;
    pub fn strxfrm(s: *mut c_char, ct: *const c_char, n: size_t) -> size_t;
    pub fn wcslen(buf: *const wchar_t) -> size_t;
    pub fn wcstombs(dest: *mut c_char, src: *const wchar_t, n: size_t) -> size_t;

    pub fn memchr(cx: *const c_void, c: c_int, n: size_t) -> *mut c_void;
    pub fn memcmp(cx: *const c_void, ct: *const c_void, n: size_t) -> c_int;
    pub fn memcpy(dest: *mut c_void, src: *const c_void, n: size_t) -> *mut c_void;
    pub fn memmove(dest: *mut c_void, src: *const c_void, n: size_t) -> *mut c_void;
    pub fn memset(dest: *mut c_void, c: c_int, n: size_t) -> *mut c_void;

    pub fn abs(i: c_int) -> c_int;
    pub fn labs(i: c_long) -> c_long;
    pub fn rand() -> c_int;
    pub fn srand(seed: c_uint);

    pub fn signal(signum: c_int, handler: sighandler_t) -> sighandler_t;
    pub fn raise(signum: c_int) -> c_int;

    pub fn clock() -> clock_t;
    pub fn ctime(sourceTime: *const time_t) -> *mut c_char;
    pub fn difftime(timeEnd: time_t, timeStart: time_t) -> c_double;
    #[link_name = "_gmtime64_s"]
    pub fn gmtime_s(destTime: *mut tm, srcTime: *const time_t) -> c_int;
    #[link_name = "_get_daylight"]
    pub fn get_daylight(hours: *mut c_int) -> errno_t;
    #[link_name = "_get_dstbias"]
    pub fn get_dstbias(seconds: *mut c_long) -> errno_t;
    #[link_name = "_get_timezone"]
    pub fn get_timezone(seconds: *mut c_long) -> errno_t;
    #[link_name = "_get_tzname"]
    pub fn get_tzname(
        p_return_value: *mut size_t,
        time_zone_name: *mut c_char,
        size_in_bytes: size_t,
        index: c_int,
    ) -> errno_t;
    #[link_name = "_localtime64_s"]
    pub fn localtime_s(tmDest: *mut tm, sourceTime: *const time_t) -> crate::errno_t;
    #[link_name = "_time64"]
    pub fn time(destTime: *mut time_t) -> time_t;
    #[link_name = "_tzset"]
    pub fn tzset();
    #[link_name = "_chmod"]
    pub fn chmod(path: *const c_char, mode: c_int) -> c_int;
    #[link_name = "_wchmod"]
    pub fn wchmod(path: *const wchar_t, mode: c_int) -> c_int;
    #[link_name = "_mkdir"]
    pub fn mkdir(path: *const c_char) -> c_int;
    #[link_name = "_wrmdir"]
    pub fn wrmdir(path: *const wchar_t) -> c_int;
    #[link_name = "_fstat64"]
    pub fn fstat(fildes: c_int, buf: *mut stat) -> c_int;
    #[link_name = "_stat64"]
    pub fn stat(path: *const c_char, buf: *mut stat) -> c_int;
    #[link_name = "_wstat64"]
    pub fn wstat(path: *const wchar_t, buf: *mut stat) -> c_int;
    #[link_name = "_wutime64"]
    pub fn wutime(file: *const wchar_t, buf: *mut utimbuf) -> c_int;
    #[link_name = "_popen"]
    pub fn popen(command: *const c_char, mode: *const c_char) -> *mut crate::FILE;
    #[link_name = "_pclose"]
    pub fn pclose(stream: *mut crate::FILE) -> c_int;
    #[link_name = "_fdopen"]
    pub fn fdopen(fd: c_int, mode: *const c_char) -> *mut crate::FILE;
    #[link_name = "_fileno"]
    pub fn fileno(stream: *mut crate::FILE) -> c_int;
    #[link_name = "_open"]
    pub fn open(path: *const c_char, oflag: c_int, ...) -> c_int;
    #[link_name = "_wopen"]
    pub fn wopen(path: *const wchar_t, oflag: c_int, ...) -> c_int;
    #[link_name = "_creat"]
    pub fn creat(path: *const c_char, mode: c_int) -> c_int;
    #[link_name = "_access"]
    pub fn access(path: *const c_char, amode: c_int) -> c_int;
    #[link_name = "_chdir"]
    pub fn chdir(dir: *const c_char) -> c_int;
    #[link_name = "_close"]
    pub fn close(fd: c_int) -> c_int;
    #[link_name = "_dup"]
    pub fn dup(fd: c_int) -> c_int;
    #[link_name = "_dup2"]
    pub fn dup2(src: c_int, dst: c_int) -> c_int;
    #[link_name = "_execl"]
    pub fn execl(path: *const c_char, arg0: *const c_char, ...) -> intptr_t;
    #[link_name = "_wexecl"]
    pub fn wexecl(path: *const wchar_t, arg0: *const wchar_t, ...) -> intptr_t;
    #[link_name = "_execle"]
    pub fn execle(path: *const c_char, arg0: *const c_char, ...) -> intptr_t;
    #[link_name = "_wexecle"]
    pub fn wexecle(path: *const wchar_t, arg0: *const wchar_t, ...) -> intptr_t;
    #[link_name = "_execlp"]
    pub fn execlp(path: *const c_char, arg0: *const c_char, ...) -> intptr_t;
    #[link_name = "_wexeclp"]
    pub fn wexeclp(path: *const wchar_t, arg0: *const wchar_t, ...) -> intptr_t;
    #[link_name = "_execlpe"]
    pub fn execlpe(path: *const c_char, arg0: *const c_char, ...) -> intptr_t;
    #[link_name = "_wexeclpe"]
    pub fn wexeclpe(path: *const wchar_t, arg0: *const wchar_t, ...) -> intptr_t;
    #[link_name = "_execv"]
    // DIFF(main): changed to `intptr_t` in e77f551de9
    pub fn execv(prog: *const c_char, argv: *const *const c_char) -> intptr_t;
    #[link_name = "_execve"]
    pub fn execve(
        prog: *const c_char,
        argv: *const *const c_char,
        envp: *const *const c_char,
    ) -> c_int;
    #[link_name = "_execvp"]
    pub fn execvp(c: *const c_char, argv: *const *const c_char) -> c_int;
    #[link_name = "_execvpe"]
    pub fn execvpe(
        c: *const c_char,
        argv: *const *const c_char,
        envp: *const *const c_char,
    ) -> c_int;

    #[link_name = "_wexecv"]
    pub fn wexecv(prog: *const wchar_t, argv: *const *const wchar_t) -> intptr_t;
    #[link_name = "_wexecve"]
    pub fn wexecve(
        prog: *const wchar_t,
        argv: *const *const wchar_t,
        envp: *const *const wchar_t,
    ) -> intptr_t;
    #[link_name = "_wexecvp"]
    pub fn wexecvp(c: *const wchar_t, argv: *const *const wchar_t) -> intptr_t;
    #[link_name = "_wexecvpe"]
    pub fn wexecvpe(
        c: *const wchar_t,
        argv: *const *const wchar_t,
        envp: *const *const wchar_t,
    ) -> intptr_t;
    #[link_name = "_getcwd"]
    pub fn getcwd(buf: *mut c_char, size: c_int) -> *mut c_char;
    #[link_name = "_getpid"]
    pub fn getpid() -> c_int;
    #[link_name = "_isatty"]
    pub fn isatty(fd: c_int) -> c_int;
    #[link_name = "_lseek"]
    pub fn lseek(fd: c_int, offset: c_long, origin: c_int) -> c_long;
    #[link_name = "_lseeki64"]
    pub fn lseek64(fd: c_int, offset: c_longlong, origin: c_int) -> c_longlong;
    #[link_name = "_pipe"]
    pub fn pipe(fds: *mut c_int, psize: c_uint, textmode: c_int) -> c_int;
    #[link_name = "_read"]
    pub fn read(fd: c_int, buf: *mut c_void, count: c_uint) -> c_int;
    #[link_name = "_rmdir"]
    pub fn rmdir(path: *const c_char) -> c_int;
    #[link_name = "_unlink"]
    pub fn unlink(c: *const c_char) -> c_int;
    #[link_name = "_write"]
    pub fn write(fd: c_int, buf: *const c_void, count: c_uint) -> c_int;
    #[link_name = "_commit"]
    pub fn commit(fd: c_int) -> c_int;
    #[link_name = "_get_osfhandle"]
    pub fn get_osfhandle(fd: c_int) -> intptr_t;
    #[link_name = "_open_osfhandle"]
    pub fn open_osfhandle(osfhandle: intptr_t, flags: c_int) -> c_int;
    pub fn setlocale(category: c_int, locale: *const c_char) -> *mut c_char;
    #[link_name = "_wsetlocale"]
    pub fn wsetlocale(category: c_int, locale: *const wchar_t) -> *mut wchar_t;
    #[link_name = "_aligned_malloc"]
    pub fn aligned_malloc(size: size_t, alignment: size_t) -> *mut c_void;
    #[link_name = "_aligned_free"]
    pub fn aligned_free(ptr: *mut c_void);
    #[link_name = "_aligned_realloc"]
    pub fn aligned_realloc(memblock: *mut c_void, size: size_t, alignment: size_t) -> *mut c_void;
    #[link_name = "_putenv"]
    pub fn putenv(envstring: *const c_char) -> c_int;
    #[link_name = "_wputenv"]
    pub fn wputenv(envstring: *const crate::wchar_t) -> c_int;
    #[link_name = "_putenv_s"]
    pub fn putenv_s(envstring: *const c_char, value_string: *const c_char) -> crate::errno_t;
    #[link_name = "_wputenv_s"]
    pub fn wputenv_s(
        envstring: *const crate::wchar_t,
        value_string: *const crate::wchar_t,
    ) -> crate::errno_t;
}

extern "system" {
    pub fn listen(s: SOCKET, backlog: c_int) -> c_int;
    pub fn accept(s: SOCKET, addr: *mut crate::sockaddr, addrlen: *mut c_int) -> SOCKET;
    pub fn bind(s: SOCKET, name: *const crate::sockaddr, namelen: c_int) -> c_int;
    pub fn connect(s: SOCKET, name: *const crate::sockaddr, namelen: c_int) -> c_int;
    pub fn getpeername(s: SOCKET, name: *mut crate::sockaddr, nameln: *mut c_int) -> c_int;
    pub fn getsockname(s: SOCKET, name: *mut crate::sockaddr, nameln: *mut c_int) -> c_int;
    pub fn getsockopt(
        s: SOCKET,
        level: c_int,
        optname: c_int,
        optval: *mut c_char,
        optlen: *mut c_int,
    ) -> c_int;
    pub fn recvfrom(
        s: SOCKET,
        buf: *mut c_char,
        len: c_int,
        flags: c_int,
        from: *mut crate::sockaddr,
        fromlen: *mut c_int,
    ) -> c_int;
    pub fn sendto(
        s: SOCKET,
        buf: *const c_char,
        len: c_int,
        flags: c_int,
        to: *const crate::sockaddr,
        tolen: c_int,
    ) -> c_int;
    pub fn setsockopt(
        s: SOCKET,
        level: c_int,
        optname: c_int,
        optval: *const c_char,
        optlen: c_int,
    ) -> c_int;
    pub fn socket(af: c_int, socket_type: c_int, protocol: c_int) -> SOCKET;
}

cfg_if! {
    if #[cfg(all(target_env = "gnu"))] {
        mod gnu;
        pub use self::gnu::*;
    } else if #[cfg(all(target_env = "msvc"))] {
        mod msvc;
        pub use self::msvc::*;
    } else {
        // Unknown target_env
    }
}
