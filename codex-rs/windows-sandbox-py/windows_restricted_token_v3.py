# Run a command inside a Windows restricted-token sandbox (no admin).
# Python re-implementation of the Rust file you shared.

import argparse
import ctypes as c
import ctypes.wintypes as wt
import json
import os
import sys
from dataclasses import dataclass
from typing import Dict, Iterable, List, Optional, Tuple
from env_utils import normalize_null_device_env
from network_sandbox import NoNetConfig, apply_no_network_to_env


LOG_FILE_PATH = os.path.join(os.path.dirname(__file__), "sandbox_commands.log")
LOG_COMMAND_PREVIEW_LIMIT = 200


def _append_command_log(message: str) -> None:
    try:
        with open(LOG_FILE_PATH, "a", encoding="utf-8") as log_file:
            log_file.write(message + "\n")
    except Exception:
        pass


def _format_command_for_log(command: Iterable[str]) -> str:
    command_str = " ".join(command)
    return command_str[:LOG_COMMAND_PREVIEW_LIMIT]


def _log_command_start(command_preview: str) -> None:
    _append_command_log(f"START: {command_preview}")


def _log_command_success(command_preview: str) -> None:
    _append_command_log(f"SUCCESS: {command_preview}")


def _log_command_failure(command_preview: str, detail: Optional[str] = None) -> None:
    if detail:
        truncated_detail = detail[:LOG_COMMAND_PREVIEW_LIMIT]
        _append_command_log(f"FAILURE: {command_preview} ({truncated_detail})")
    else:
        _append_command_log(f"FAILURE: {command_preview}")

# ---- Minimal SandboxPolicy model (compatible with the CLI) -------------------

@dataclass
class WritableRoot:
    root: str

class SandboxPolicy:
    # Modes: "read-only", "workspace-write", "danger-full-access"
    def __init__(self, mode: str, workspace_roots: Optional[List[str]] = None):
        self.mode = mode
        self.workspace_roots = workspace_roots or []

    @staticmethod
    def new_read_only_policy():
        return SandboxPolicy("read-only")

    @staticmethod
    def new_workspace_write_policy(workspace_roots: Optional[List[str]] = None):
        return SandboxPolicy("workspace-write", workspace_roots or [])

    @staticmethod
    def danger_full_access():
        return SandboxPolicy("danger-full-access")

    def has_full_network_access(self) -> bool:
        # Match Rust behavior: only "danger" gets network by default.
        return self.mode == "danger-full-access"

    # In Rust this resolves policy-relative roots; we replicate the effective list.
    def get_writable_roots_with_cwd(self, policy_cwd: str) -> List[WritableRoot]:
        if self.mode != "workspace-write":
            return []
        roots: List[WritableRoot] = []
        for p in self.workspace_roots:
            pth = p
            if not os.path.isabs(pth):
                pth = os.path.abspath(os.path.join(policy_cwd, pth))
            roots.append(WritableRoot(root=pth))
        return roots


# ---- Win32 helpers -----------------------------------------------------------

advapi32 = c.WinDLL("advapi32", use_last_error=True)
kernel32 = c.WinDLL("kernel32", use_last_error=True)
secur32  = c.WinDLL("secur32", use_last_error=True)
try:
    aclapi   = c.WinDLL("aclapi", use_last_error=True)
except OSError:
    aclapi   = c.WinDLL("advapi32", use_last_error=True)

# Constants (selected)
ERROR_SUCCESS = 0
INVALID_HANDLE_VALUE = wt.HANDLE(-1).value

TOKEN_DUPLICATE        = 0x0002
TOKEN_QUERY            = 0x0008
TOKEN_ASSIGN_PRIMARY   = 0x0001
TOKEN_ADJUST_DEFAULT   = 0x0080
TOKEN_ADJUST_SESSIONID = 0x0100
TOKEN_ADJUST_PRIVILEGES= 0x0020

CREATE_UNICODE_ENVIRONMENT = 0x00000400
STARTF_USESTDHANDLES       = 0x00000100
INFINITE = 0xFFFFFFFF
WAIT_OBJECT_0 = 0x00000000

# Correct 32-bit mask for Logon SID attribute
SE_GROUP_LOGON_ID = 0xC0000000

# Job object
JobObjectExtendedLimitInformation = 9
JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE = 0x00002000

# DACL/security constants
SE_FILE_OBJECT = 1
DACL_SECURITY_INFORMATION = 0x00000004
CONTAINER_INHERIT_ACE = 0x2
OBJECT_INHERIT_ACE = 0x1
SE_KERNEL_OBJECT = 6
READ_CONTROL = 0x00020000
WRITE_DAC = 0x00040000
GENERIC_READ = 0x80000000
GENERIC_WRITE = 0x40000000
FILE_SHARE_READ = 0x00000001
FILE_SHARE_WRITE = 0x00000002
OPEN_EXISTING = 3
FILE_ATTRIBUTE_NORMAL = 0x00000080
WinWorldSid = 1

# ACCESS_MODE
NOT_USED_ACCESS   = 0
GRANT_ACCESS      = 1
SET_ACCESS        = 2
DENY_ACCESS       = 3
REVOKE_ACCESS     = 4
SET_AUDIT_SUCCESS = 5
SET_AUDIT_FAILURE = 6

TRUSTEE_IS_SID = 0
TRUSTEE_IS_UNKNOWN = 0

FILE_GENERIC_READ    = 0x120089
FILE_GENERIC_WRITE   = 0x120116
FILE_GENERIC_EXECUTE = 0x1200A0

# structures
class LUID(c.Structure):
    _fields_ = [("LowPart", wt.DWORD),
                ("HighPart", wt.LONG)]

class LUID_AND_ATTRIBUTES(c.Structure):
    _fields_ = [("Luid", LUID),
                ("Attributes", wt.DWORD)]

class SID_AND_ATTRIBUTES(c.Structure):
    _fields_ = [("Sid", wt.LPVOID),
                ("Attributes", wt.DWORD)]

class TOKEN_GROUPS(c.Structure):
    _fields_ = [("GroupCount", wt.DWORD),
                ("Groups", SID_AND_ATTRIBUTES * 1)]  # variable length; manual handling below

class STARTUPINFOW(c.Structure):
    _fields_ = [
        ("cb", wt.DWORD),
        ("lpReserved", wt.LPWSTR),
        ("lpDesktop", wt.LPWSTR),
        ("lpTitle", wt.LPWSTR),
        ("dwX", wt.DWORD),
        ("dwY", wt.DWORD),
        ("dwXSize", wt.DWORD),
        ("dwYSize", wt.DWORD),
        ("dwXCountChars", wt.DWORD),
        ("dwYCountChars", wt.DWORD),
        ("dwFillAttribute", wt.DWORD),
        ("dwFlags", wt.DWORD),
        ("wShowWindow", wt.WORD),
        ("cbReserved2", wt.WORD),
        ("lpReserved2", wt.LPBYTE),
        ("hStdInput", wt.HANDLE),
        ("hStdOutput", wt.HANDLE),
        ("hStdError", wt.HANDLE),
    ]

class PROCESS_INFORMATION(c.Structure):
    _fields_ = [
        ("hProcess", wt.HANDLE),
        ("hThread", wt.HANDLE),
        ("dwProcessId", wt.DWORD),
        ("dwThreadId", wt.DWORD),
    ]

class SECURITY_DESCRIPTOR(c.Structure):
    _fields_ = [("_", wt.BYTE * 1)]  # opaque

class TRUSTEE_W(c.Structure):
    _fields_ = [
        ("pMultipleTrustee", wt.LPVOID),
        ("MultipleTrusteeOperation", wt.DWORD),
        ("TrusteeForm", wt.DWORD),
        ("TrusteeType", wt.DWORD),
        ("ptstrName", wt.LPWSTR),
    ]

class EXPLICIT_ACCESS_W(c.Structure):
    _fields_ = [
        ("grfAccessPermissions", wt.DWORD),
        ("grfAccessMode", wt.DWORD),
        ("grfInheritance", wt.DWORD),
        ("Trustee", TRUSTEE_W),
    ]

class LARGE_INTEGER(c.Union):
    _fields_ = [("QuadPart", c.c_longlong)]

class JOBOBJECT_BASIC_LIMIT_INFORMATION(c.Structure):
    _fields_ = [
        ("PerProcessUserTimeLimit", LARGE_INTEGER),
        ("PerJobUserTimeLimit", LARGE_INTEGER),
        ("LimitFlags", wt.DWORD),
        ("MinimumWorkingSetSize", c.c_size_t),
        ("MaximumWorkingSetSize", c.c_size_t),
        ("ActiveProcessLimit", wt.DWORD),
        ("Affinity", c.c_size_t),
        ("PriorityClass", wt.DWORD),
        ("SchedulingClass", wt.DWORD),
    ]

class IO_COUNTERS(c.Structure):
    _fields_ = [
        ("ReadOperationCount", c.c_ulonglong),
        ("WriteOperationCount", c.c_ulonglong),
        ("OtherOperationCount", c.c_ulonglong),
        ("ReadTransferCount", c.c_ulonglong),
        ("WriteTransferCount", c.c_ulonglong),
        ("OtherTransferCount", c.c_ulonglong),
    ]

class JOBOBJECT_EXTENDED_LIMIT_INFORMATION(c.Structure):
    _fields_ = [
        ("BasicLimitInformation", JOBOBJECT_BASIC_LIMIT_INFORMATION),
        ("IoInfo", IO_COUNTERS),
        ("ProcessMemoryLimit", c.c_size_t),
        ("JobMemoryLimit", c.c_size_t),
        ("PeakProcessMemoryUsed", c.c_size_t),
        ("PeakJobMemoryUsed", c.c_size_t),
    ]

# prototypes
advapi32.OpenProcessToken.argtypes = [wt.HANDLE, wt.DWORD, c.POINTER(wt.HANDLE)]
advapi32.OpenProcessToken.restype  = wt.BOOL

advapi32.GetTokenInformation.argtypes = [wt.HANDLE, wt.DWORD, wt.LPVOID, wt.DWORD, c.POINTER(wt.DWORD)]
advapi32.GetTokenInformation.restype  = wt.BOOL
TokenGroupsClass = 2  # TOKEN_INFORMATION_CLASS::TokenGroups
TokenRestrictedSidsClass = 11  # TOKEN_INFORMATION_CLASS::TokenRestrictedSids

advapi32.CreateRestrictedToken.argtypes = [
    wt.HANDLE, wt.DWORD,
    wt.DWORD, c.POINTER(SID_AND_ATTRIBUTES),
    wt.DWORD, c.POINTER(LUID_AND_ATTRIBUTES),
    wt.DWORD, c.POINTER(SID_AND_ATTRIBUTES),
    c.POINTER(wt.HANDLE)
]
advapi32.CreateRestrictedToken.restype  = wt.BOOL
DISABLE_MAX_PRIVILEGE = 0x01
LUA_TOKEN             = 0x04
WRITE_RESTRICTED      = 0x08

advapi32.GetLengthSid.argtypes = [wt.LPVOID]
advapi32.GetLengthSid.restype  = wt.DWORD

advapi32.CopySid.argtypes = [wt.DWORD, wt.LPVOID, wt.LPVOID]
advapi32.CopySid.restype  = wt.BOOL

# Privilege APIs
advapi32.LookupPrivilegeValueW.argtypes = [wt.LPCWSTR, wt.LPCWSTR, c.POINTER(LUID)]
advapi32.LookupPrivilegeValueW.restype  = wt.BOOL

class TOKEN_PRIVILEGES(c.Structure):
    _fields_ = [
        ("PrivilegeCount", wt.DWORD),
        ("Privileges", LUID_AND_ATTRIBUTES * 1),
    ]

advapi32.AdjustTokenPrivileges.argtypes = [
    wt.HANDLE, wt.BOOL,
    c.POINTER(TOKEN_PRIVILEGES), wt.DWORD,
    wt.LPVOID, c.POINTER(wt.DWORD),
]
advapi32.AdjustTokenPrivileges.restype  = wt.BOOL

SE_PRIVILEGE_ENABLED      = 0x00000002
SE_CHANGE_NOTIFY_NAME     = "SeChangeNotifyPrivilege"  # directory traverse / bypass traverse checking

# Kernel object security (e.g., \Device\Null)
advapi32.GetSecurityInfo.argtypes = [
    wt.HANDLE, wt.DWORD, wt.DWORD,
    c.POINTER(wt.LPVOID), c.POINTER(wt.LPVOID),
    c.POINTER(wt.LPVOID), c.POINTER(wt.LPVOID),
    c.POINTER(wt.LPVOID),
]
advapi32.GetSecurityInfo.restype = wt.DWORD

advapi32.SetSecurityInfo.argtypes = [
    wt.HANDLE, wt.DWORD, wt.DWORD,
    wt.LPVOID, wt.LPVOID, wt.LPVOID, wt.LPVOID
]
advapi32.SetSecurityInfo.restype  = wt.DWORD

kernel32.GetCurrentProcess.argtypes = []
kernel32.GetCurrentProcess.restype  = wt.HANDLE

kernel32.GetStdHandle.argtypes = [wt.DWORD]
kernel32.GetStdHandle.restype  = wt.HANDLE
STD_INPUT_HANDLE  = wt.DWORD(-10)
STD_OUTPUT_HANDLE = wt.DWORD(-11)
STD_ERROR_HANDLE  = wt.DWORD(-12)

kernel32.SetHandleInformation.argtypes = [wt.HANDLE, wt.DWORD, wt.DWORD]
kernel32.SetHandleInformation.restype  = wt.BOOL
HANDLE_FLAG_INHERIT = 0x00000001

kernel32.WaitForSingleObject.argtypes = [wt.HANDLE, wt.DWORD]
kernel32.WaitForSingleObject.restype  = wt.DWORD

kernel32.GetExitCodeProcess.argtypes = [wt.HANDLE, c.POINTER(wt.DWORD)]
kernel32.GetExitCodeProcess.restype  = wt.BOOL

kernel32.CloseHandle.argtypes = [wt.HANDLE]
kernel32.CloseHandle.restype  = wt.BOOL

kernel32.CreateJobObjectW.argtypes = [wt.LPVOID, wt.LPCWSTR]
kernel32.CreateJobObjectW.restype  = wt.HANDLE

kernel32.SetInformationJobObject.argtypes = [wt.HANDLE, wt.INT, wt.LPVOID, wt.DWORD]
kernel32.SetInformationJobObject.restype  = wt.BOOL

kernel32.AssignProcessToJobObject.argtypes = [wt.HANDLE, wt.HANDLE]
kernel32.AssignProcessToJobObject.restype  = wt.BOOL

# Effective rights from ACL (for diagnostics)
advapi32.GetEffectiveRightsFromAclW.argtypes = [wt.LPVOID, c.POINTER(TRUSTEE_W), c.POINTER(wt.DWORD)]
advapi32.GetEffectiveRightsFromAclW.restype  = wt.DWORD

advapi32.CreateProcessAsUserW.argtypes = [
    wt.HANDLE, wt.LPCWSTR, wt.LPWSTR,
    wt.LPVOID, wt.LPVOID, wt.BOOL, wt.DWORD,
    wt.LPVOID, wt.LPCWSTR,
    c.POINTER(STARTUPINFOW), c.POINTER(PROCESS_INFORMATION)
]
advapi32.CreateProcessAsUserW.restype  = wt.BOOL

advapi32.CreateWellKnownSid.argtypes = [wt.DWORD, wt.LPVOID, wt.LPVOID, c.POINTER(wt.DWORD)]
advapi32.CreateWellKnownSid.restype  = wt.BOOL

aclapi.GetNamedSecurityInfoW.argtypes = [
    wt.LPCWSTR, wt.DWORD, wt.DWORD,
    c.POINTER(wt.LPVOID), c.POINTER(wt.LPVOID),
    c.POINTER(wt.LPVOID), c.POINTER(wt.LPVOID),
    c.POINTER(wt.LPVOID)
]
aclapi.GetNamedSecurityInfoW.restype  = wt.DWORD

aclapi.SetEntriesInAclW.argtypes = [
    wt.ULONG, c.POINTER(EXPLICIT_ACCESS_W), wt.LPVOID, c.POINTER(wt.LPVOID)
]
aclapi.SetEntriesInAclW.restype  = wt.DWORD

aclapi.SetNamedSecurityInfoW.argtypes = [
    wt.LPWSTR, wt.DWORD, wt.DWORD,
    wt.LPVOID, wt.LPVOID, wt.LPVOID, wt.LPVOID
]
aclapi.SetNamedSecurityInfoW.restype  = wt.DWORD

kernel32.LocalFree.argtypes = [wt.HLOCAL]
kernel32.LocalFree.restype  = wt.HLOCAL

# --- ACL/ACE enumeration helpers --------------------------------------------

class ACL(c.Structure):
    _fields_ = [
        ("AclRevision", wt.BYTE),
        ("Sbz1", wt.BYTE),
        ("AclSize", wt.WORD),
        ("AceCount", wt.WORD),
        ("Sbz2", wt.WORD),
    ]

class ACL_SIZE_INFORMATION(c.Structure):
    _fields_ = [
        ("AceCount", wt.DWORD),
        ("AclBytesInUse", wt.DWORD),
        ("AclBytesFree", wt.DWORD),
    ]

class ACE_HEADER(c.Structure):
    _fields_ = [
        ("AceType", wt.BYTE),
        ("AceFlags", wt.BYTE),
        ("AceSize", wt.WORD),
    ]

class ACCESS_ALLOWED_ACE(c.Structure):
    _fields_ = [
        ("Header", ACE_HEADER),
        ("Mask", wt.DWORD),
        ("SidStart", wt.DWORD),  # first DWORD of SID; rest follows
    ]

ACCESS_ALLOWED_ACE_TYPE = 0x00

advapi32.GetAclInformation.argtypes = [wt.LPVOID, wt.LPVOID, wt.DWORD, wt.DWORD]
advapi32.GetAclInformation.restype  = wt.BOOL
AclSizeInformation = 2  # ACL_INFORMATION_CLASS

advapi32.GetAce.argtypes = [wt.LPVOID, wt.DWORD, c.POINTER(wt.LPVOID)]
advapi32.GetAce.restype  = wt.BOOL

advapi32.EqualSid.argtypes = [wt.LPVOID, wt.LPVOID]
advapi32.EqualSid.restype  = wt.BOOL

# --- Diagnostics support -----------------------------------------------------

advapi32.IsTokenRestricted.argtypes = [wt.HANDLE]
advapi32.IsTokenRestricted.restype  = wt.BOOL

secur32.ConvertSidToStringSidW = getattr(secur32, "ConvertSidToStringSidW", None)
if secur32.ConvertSidToStringSidW:
    secur32.ConvertSidToStringSidW.argtypes = [wt.LPVOID, c.POINTER(wt.LPWSTR)]
    secur32.ConvertSidToStringSidW.restype  = wt.BOOL

def _sid_str(psid: wt.LPVOID) -> str:
    try:
        if secur32.ConvertSidToStringSidW:
            pw = wt.LPWSTR()
            if secur32.ConvertSidToStringSidW(psid, c.byref(pw)):
                s = pw.value
                if pw:
                    kernel32.LocalFree(pw)
                return s
    except Exception:
        pass
    return "<sid>"

def _dump_restricted_sids(h_token: wt.HANDLE):
    """Debug: list the Restricted SID set on the token."""
    needed = wt.DWORD(0)
    advapi32.GetTokenInformation(h_token, TokenRestrictedSidsClass, None, 0, c.byref(needed))
    if needed.value == 0:
        #print("[sandbox:debug] restricted-sids: (none or query failed)", file=sys.stderr)
        return
    buf = (c.c_ubyte * needed.value)()
    ok = advapi32.GetTokenInformation(h_token, TokenRestrictedSidsClass, buf, needed, c.byref(needed))
    if not ok:
        #print("[sandbox:debug] restricted-sids: (query failed)", file=sys.stderr)
        return
    tgroups = c.cast(buf, c.POINTER(TOKEN_GROUPS)).contents
    arr_t = SID_AND_ATTRIBUTES * tgroups.GroupCount
    groups = c.cast(c.addressof(tgroups.Groups), c.POINTER(arr_t)).contents
    lines = []
    for i in range(tgroups.GroupCount):
        lines.append(_sid_str(groups[i].Sid))
    #print(f"[sandbox:debug] restricted-sids count={tgroups.GroupCount} {lines}", file=sys.stderr)

def _cwd_effective_write_report(path: str, psid_restrict: wt.LPVOID):
    """Debug: report whether CWD grants FILE_GENERIC_WRITE to restricting SID and to Everyone."""
    pSD = wt.LPVOID()
    pDACL = wt.LPVOID()
    code = aclapi.GetNamedSecurityInfoW(
        wt.LPCWSTR(path), SE_FILE_OBJECT, DACL_SECURITY_INFORMATION,
        None, None, c.byref(pDACL), None, c.byref(pSD)
    )
    if code != ERROR_SUCCESS or not pDACL:
        #print(f"[sandbox:debug] cwd dacl: query failed code={code}", file=sys.stderr)
        if pSD: kernel32.LocalFree(pSD)
        return
    try:
        # Build trustee for restricting SID
        mask = wt.DWORD(0)
        tr = TRUSTEE_W(None, 0, TRUSTEE_IS_SID, TRUSTEE_IS_UNKNOWN, c.cast(psid_restrict, wt.LPWSTR))
        code = advapi32.GetEffectiveRightsFromAclW(pDACL, c.byref(tr), c.byref(mask))
        if code == ERROR_SUCCESS:
            wr = bool(mask.value & FILE_GENERIC_WRITE)
            #print(f"[sandbox:debug] cwd write granted to restricting SID={wr} mask=0x{mask.value:08X}", file=sys.stderr)
        else:
            pass#print(f"[sandbox:debug] cwd write check for restricting SID failed code={code}", file=sys.stderr)

        # Build Everyone SID
        size = wt.DWORD(0)
        advapi32.CreateWellKnownSid(WinWorldSid, None, None, c.byref(size))
        everyone_buf = (c.c_ubyte * size.value)()
        if advapi32.CreateWellKnownSid(WinWorldSid, None, everyone_buf, c.byref(size)):
            psid_everyone = c.cast(everyone_buf, wt.LPVOID)
            mask2 = wt.DWORD(0)
            tr2 = TRUSTEE_W(None, 0, TRUSTEE_IS_SID, TRUSTEE_IS_UNKNOWN, c.cast(psid_everyone, wt.LPWSTR))
            code2 = advapi32.GetEffectiveRightsFromAclW(pDACL, c.byref(tr2), c.byref(mask2))
            if code2 == ERROR_SUCCESS:
                wr2 = bool(mask2.value & FILE_GENERIC_WRITE)
                #print(f"[sandbox:debug] cwd write granted to Everyone={wr2} mask=0x{mask2.value:08X}", file=sys.stderr)
            else:
                pass#print(f"[sandbox:debug] cwd write check for Everyone failed code={code2}", file=sys.stderr)
    finally:
        if pSD: kernel32.LocalFree(pSD)

def _check_bool(ok: bool, fn: str):
    if not ok:
        raise c.WinError(c.get_last_error())

def _check_winerr(code: int, fn: str):
    if code != ERROR_SUCCESS:
        raise c.WinError(code)

def _close_handle_safe(h):
    if h and h != 0 and h != INVALID_HANDLE_VALUE:
        kernel32.CloseHandle(h)

def _make_env_block(env: Dict[str, str]) -> bytes:
    # Windows expects sorted, case-insensitive by name, double-NUL terminated UTF-16LE
    items = sorted(env.items(), key=lambda kv: (kv[0].upper(), kv[0]))
    joined = "".join(f"{k}={v}\0" for k, v in items) + "\0"
    return joined.encode("utf-16le")

def _ensure_inheritable_stdio(si: STARTUPINFOW):
    for kind in (STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE):
        h = kernel32.GetStdHandle(kind)
        if h == 0 or h == INVALID_HANDLE_VALUE:
            raise c.WinError(c.get_last_error())
        _check_bool(kernel32.SetHandleInformation(h, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT),
                    "SetHandleInformation")
    si.dwFlags |= STARTF_USESTDHANDLES
    si.hStdInput  = kernel32.GetStdHandle(STD_INPUT_HANDLE)
    si.hStdOutput = kernel32.GetStdHandle(STD_OUTPUT_HANDLE)
    si.hStdError  = kernel32.GetStdHandle(STD_ERROR_HANDLE)

def _get_current_token_for_restriction() -> wt.HANDLE:
    desired = (TOKEN_DUPLICATE | TOKEN_QUERY | TOKEN_ASSIGN_PRIMARY |
               TOKEN_ADJUST_DEFAULT | TOKEN_ADJUST_SESSIONID | TOKEN_ADJUST_PRIVILEGES)
    h = wt.HANDLE()
    _check_bool(advapi32.OpenProcessToken(kernel32.GetCurrentProcess(), desired, c.byref(h)),
                "OpenProcessToken")
    return h

# ---- Privilege enable helper -------------------------------------------------

def _enable_single_privilege(h_token: wt.HANDLE, name: str):
    luid = LUID()
    _check_bool(advapi32.LookupPrivilegeValueW(None, wt.LPCWSTR(name), c.byref(luid)),
                "LookupPrivilegeValueW")
    tp = TOKEN_PRIVILEGES()
    tp.PrivilegeCount = 1
    tp.Privileges[0].Luid = luid
    tp.Privileges[0].Attributes = SE_PRIVILEGE_ENABLED
    _check_bool(advapi32.AdjustTokenPrivileges(h_token, False, c.byref(tp), 0, None, None),
                "AdjustTokenPrivileges")
    err = c.get_last_error()
    if err != 0:  # ERROR_SUCCESS
        raise c.WinError(err)

# ---- Token group helpers -----------------------------------------------------

def _get_logon_sid_bytes(h_token: wt.HANDLE) -> bytes:
    needed = wt.DWORD(0)
    advapi32.GetTokenInformation(h_token, TokenGroupsClass, None, 0, c.byref(needed))
    if not needed.value:
        raise c.WinError(c.get_last_error())
    buf = (c.c_ubyte * needed.value)()
    _check_bool(advapi32.GetTokenInformation(h_token, TokenGroupsClass, buf, needed, c.byref(needed)),
                "GetTokenInformation(TokenGroups)")
    tgroups = c.cast(buf, c.POINTER(TOKEN_GROUPS)).contents
    arr_t = SID_AND_ATTRIBUTES * tgroups.GroupCount
    groups = c.cast(c.addressof(tgroups.Groups), c.POINTER(arr_t)).contents
    for i in range(tgroups.GroupCount):
        if (groups[i].Attributes & SE_GROUP_LOGON_ID) == SE_GROUP_LOGON_ID:
            sid = groups[i].Sid
            sid_len = advapi32.GetLengthSid(sid)
            tmp = (c.c_ubyte * sid_len)()
            _check_bool(advapi32.CopySid(sid_len, tmp, sid), "CopySid")
            return bytes(tmp)
    raise RuntimeError("Logon SID not present on token")

# ---- Restricted token creators: strict & compat ------------------------------

def _create_write_restricted_token_strict() -> Tuple[wt.HANDLE, wt.LPVOID]:
    r"""
    Strict mode (used for read-only): SidsToRestrict = [ Logon SID ].
    Re-enables SeChangeNotifyPrivilege on the new token so read/exec works.
    """
    base = _get_current_token_for_restriction()
    try:
        logon_sid_bytes = _get_logon_sid_bytes(base)
        sid_buf = c.create_string_buffer(logon_sid_bytes)          # pin PSID memory
        psid_logon = c.cast(sid_buf, wt.LPVOID)
        globals().setdefault("_LIVE_SID_BUFFERS", []).append(sid_buf)

        restrict_entries = (SID_AND_ATTRIBUTES * 1)()
        restrict_entries[0].Sid = psid_logon
        restrict_entries[0].Attributes = 0

        new_token = wt.HANDLE()
        flags = DISABLE_MAX_PRIVILEGE | LUA_TOKEN | WRITE_RESTRICTED
        _check_bool(advapi32.CreateRestrictedToken(
            base, flags,
            0, None,           # SIDs to disable
            0, None,           # Privileges to delete
            1, restrict_entries,
            c.byref(new_token)
        ), "CreateRestrictedToken")

        _enable_single_privilege(new_token, SE_CHANGE_NOTIFY_NAME)

        # --- Diagnostics
        try:
            pass#print("[sandbox:debug] using STRICT token (Logon SID only)", file=sys.stderr)
        except Exception:
            pass

        return new_token, psid_logon
    finally:
        _close_handle_safe(base)

def _create_write_restricted_token_compat() -> Tuple[wt.HANDLE, wt.LPVOID]:
    r"""
    Compat mode (workspace-write/danger): SidsToRestrict = [ Logon SID, Everyone ].
    Keeps Git/PowerShell/Python happy with NUL; still re-enables SeChangeNotifyPrivilege.
    """
    base = _get_current_token_for_restriction()
    try:
        logon_sid_bytes = _get_logon_sid_bytes(base)
        sid_buf = c.create_string_buffer(logon_sid_bytes)
        psid_logon = c.cast(sid_buf, wt.LPVOID)
        globals().setdefault("_LIVE_SID_BUFFERS", []).append(sid_buf)

        # Everyone (WORLD) SID
        everyone_size = wt.DWORD(0)
        advapi32.CreateWellKnownSid(WinWorldSid, None, None, c.byref(everyone_size))
        everyone_buf = (c.c_ubyte * everyone_size.value)()
        _check_bool(advapi32.CreateWellKnownSid(WinWorldSid, None, everyone_buf, c.byref(everyone_size)),
                    "CreateWellKnownSid(WinWorldSid)")
        psid_everyone = c.cast(everyone_buf, wt.LPVOID)

        restrict_entries = (SID_AND_ATTRIBUTES * 2)()
        restrict_entries[0].Sid = psid_logon
        restrict_entries[0].Attributes = 0
        restrict_entries[1].Sid = psid_everyone
        restrict_entries[1].Attributes = 0

        new_token = wt.HANDLE()
        flags = DISABLE_MAX_PRIVILEGE | LUA_TOKEN | WRITE_RESTRICTED
        _check_bool(advapi32.CreateRestrictedToken(
            base, flags,
            0, None,
            0, None,
            2, restrict_entries,
            c.byref(new_token)
        ), "CreateRestrictedToken")

        _enable_single_privilege(new_token, SE_CHANGE_NOTIFY_NAME)

        # --- Diagnostics
        try:
            pass#print("[sandbox:debug] using COMPAT token (Logon SID + Everyone)", file=sys.stderr)
        except Exception:
            pass

        return new_token, psid_logon
    finally:
        _close_handle_safe(base)

# ---- Named security helpers --------------------------------------------------

def _sid_to_string(sid_bytes: bytes) -> str:
    if not secur32.ConvertSidToStringSidW:
        return "<sid>"
    pwstr = wt.LPWSTR()
    sid_ptr = c.cast(c.create_string_buffer(sid_bytes), wt.LPVOID)
    _check_bool(secur32.ConvertSidToStringSidW(sid_ptr, c.byref(pwstr)), "ConvertSidToStringSidW")
    try:
        return pwstr.value
    finally:
        if pwstr:
            kernel32.LocalFree(pwstr)

def _dacl_has_write_allow_for_sid(pDACL: wt.LPVOID, psid: wt.LPVOID) -> bool:
    """Return True if DACL already contains an ACCESS_ALLOWED ACE for psid granting write."""
    if not pDACL:
        return False
    size = ACL_SIZE_INFORMATION()
    ok = advapi32.GetAclInformation(pDACL, c.byref(size), wt.DWORD(c.sizeof(ACL_SIZE_INFORMATION)), wt.DWORD(AclSizeInformation))
    if not ok:
        return False
    count = int(size.AceCount)
    for i in range(count):
        pAce = wt.LPVOID()
        if not advapi32.GetAce(pDACL, wt.DWORD(i), c.byref(pAce)):
            continue
        hdr = c.cast(pAce, c.POINTER(ACE_HEADER)).contents
        if hdr.AceType != ACCESS_ALLOWED_ACE_TYPE:
            continue
        aa_ptr = c.cast(pAce, c.POINTER(ACCESS_ALLOWED_ACE))
        # Access mask
        mask = aa_ptr.contents.Mask
        # Compute pointer to SID: immediately after header + mask
        base_addr = c.cast(pAce, c.c_void_p).value or 0
        sid_addr = base_addr + c.sizeof(ACE_HEADER) + c.sizeof(wt.DWORD)
        sid_ptr = c.c_void_p(sid_addr)
        try:
            if advapi32.EqualSid(sid_ptr, psid) and (mask & FILE_GENERIC_WRITE):
                return True
        except Exception:
            # If EqualSid fails for any reason, be conservative and continue.
            continue
    return False

def _add_allow_ace(path: str, psid: wt.LPVOID) -> bool:
    """Ensure DACL grants read/write/execute to psid. Returns True if an ACE was added."""
    # DACL += ACE granting (GENERIC_READ | GENERIC_WRITE | GENERIC_EXECUTE), CI | OI
    pSD = wt.LPVOID()
    pDACL = wt.LPVOID()
    _check_winerr(aclapi.GetNamedSecurityInfoW(
        wt.LPCWSTR(path), SE_FILE_OBJECT, DACL_SECURITY_INFORMATION,
        None, None, c.byref(pDACL), None, c.byref(pSD)
    ), "GetNamedSecurityInfoW")

    try:
        # If an allow ACE with write already exists for this SID, skip
        try:
            if _dacl_has_write_allow_for_sid(pDACL, psid):
                return False
        except Exception:
            # If the check fails, proceed to set (original behavior).
            pass

        explicit = EXPLICIT_ACCESS_W()
        explicit.grfAccessPermissions = (FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE)
        explicit.grfAccessMode = SET_ACCESS
        explicit.grfInheritance = (CONTAINER_INHERIT_ACE | OBJECT_INHERIT_ACE)
        explicit.Trustee = TRUSTEE_W(
            pMultipleTrustee=None,
            MultipleTrusteeOperation=0,
            TrusteeForm=TRUSTEE_IS_SID,
            TrusteeType=TRUSTEE_IS_UNKNOWN,
            ptstrName=c.cast(psid, wt.LPWSTR)  # TRUSTEE_IS_SID: this field is a PSID
        )

        pNewDacl = wt.LPVOID()
        _check_winerr(aclapi.SetEntriesInAclW(1, c.byref(explicit), pDACL, c.byref(pNewDacl)),
                      "SetEntriesInAclW")

        try:
            _check_winerr(aclapi.SetNamedSecurityInfoW(
                wt.LPWSTR(path), SE_FILE_OBJECT, DACL_SECURITY_INFORMATION,
                None, None, pNewDacl, None
            ), "SetNamedSecurityInfoW")
            return True
        finally:
            if pNewDacl:
                kernel32.LocalFree(pNewDacl)
    finally:
        if pSD:
            kernel32.LocalFree(pSD)

def _revoke_ace(path: str, psid: wt.LPVOID):
    # Best effort removal (REVOKE_ACCESS)
    pSD = wt.LPVOID()
    pDACL = wt.LPVOID()
    code = aclapi.GetNamedSecurityInfoW(
        wt.LPCWSTR(path), SE_FILE_OBJECT, DACL_SECURITY_INFORMATION,
        None, None, c.byref(pDACL), None, c.byref(pSD)
    )
    if code != ERROR_SUCCESS:
        if pSD:
            kernel32.LocalFree(pSD)
        return
    try:
        explicit = EXPLICIT_ACCESS_W()
        explicit.grfAccessPermissions = 0
        explicit.grfAccessMode = REVOKE_ACCESS
        explicit.grfInheritance = (CONTAINER_INHERIT_ACE | OBJECT_INHERIT_ACE)
        explicit.Trustee = TRUSTEE_W(
            pMultipleTrustee=None,
            MultipleTrusteeOperation=0,
            TrusteeForm=TRUSTEE_IS_SID,
            TrusteeType=TRUSTEE_IS_UNKNOWN,
            ptstrName=c.cast(psid, wt.LPWSTR)
        )
        pNewDacl = wt.LPVOID()
        code = aclapi.SetEntriesInAclW(1, c.byref(explicit), pDACL, c.byref(pNewDacl))
        if code != ERROR_SUCCESS:
            if pNewDacl:
                kernel32.LocalFree(pNewDacl)
            return
        try:
            aclapi.SetNamedSecurityInfoW(
                wt.LPWSTR(path), SE_FILE_OBJECT, DACL_SECURITY_INFORMATION,
                None, None, pNewDacl, None
            )
        finally:
            if pNewDacl:
                kernel32.LocalFree(pNewDacl)
    finally:
        if pSD:
            kernel32.LocalFree(pSD)

def _allow_null_device(psid: wt.LPVOID):
    r"""
    Best-effort: grant FILE_GENERIC_READ|WRITE|EXECUTE to the current Logon SID
    on the kernel device object backing NUL (\\.\NUL). Harmless if it fails.
    """
    desired = READ_CONTROL | WRITE_DAC
    h = kernel32.CreateFileW(
        wt.LPCWSTR(r"\\.\NUL"),
        wt.DWORD(desired),
        wt.DWORD(FILE_SHARE_READ | FILE_SHARE_WRITE),
        None,
        wt.DWORD(OPEN_EXISTING),
        wt.DWORD(FILE_ATTRIBUTE_NORMAL),
        None,
    )
    if h == 0 or h == INVALID_HANDLE_VALUE:
        return
    try:
        pSD = wt.LPVOID()
        pDACL = wt.LPVOID()

        code = advapi32.GetSecurityInfo(
            h, wt.DWORD(SE_KERNEL_OBJECT), wt.DWORD(DACL_SECURITY_INFORMATION),
            None, None, c.byref(pDACL), None, c.byref(pSD)
        )
        if code != ERROR_SUCCESS:
            return
        try:
            explicit = EXPLICIT_ACCESS_W()
            explicit.grfAccessPermissions = (FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE)
            explicit.grfAccessMode = SET_ACCESS
            explicit.grfInheritance = 0
            explicit.Trustee = TRUSTEE_W(
                pMultipleTrustee=None,
                MultipleTrusteeOperation=0,
                TrusteeForm=TRUSTEE_IS_SID,
                TrusteeType=TRUSTEE_IS_UNKNOWN,
                ptstrName=c.cast(psid, wt.LPWSTR),
            )

            pNewDacl = wt.LPVOID()
            code = aclapi.SetEntriesInAclW(1, c.byref(explicit), pDACL, c.byref(pNewDacl))
            if code != ERROR_SUCCESS:
                return
            try:
                code = advapi32.SetSecurityInfo(
                    h, wt.DWORD(SE_KERNEL_OBJECT), wt.DWORD(DACL_SECURITY_INFORMATION),
                    None, None, pNewDacl, None
                )
                if code != ERROR_SUCCESS:
                    return
            finally:
                if pNewDacl:
                    kernel32.LocalFree(pNewDacl)
        finally:
            if pSD:
                kernel32.LocalFree(pSD)
    finally:
        kernel32.CloseHandle(h)

# ---- RAII DACL guard & policy mapping ---------------------------------------

class _AclGuard:
    def __init__(self, path: str, psid: wt.LPVOID):
        self.path = path
        self.psid = psid
        self.active = True

    def close(self):
        if self.active:
            try:
                _revoke_ace(self.path, self.psid)
            except Exception:
                pass
            self.active = False

    def __del__(self):
        self.close()

def _configure_paths(policy: SandboxPolicy, policy_cwd: str, command_cwd: str,
                     psid: wt.LPVOID, env_map: Dict[str, str]) -> List[_AclGuard]:
    allow: List[str] = []
    seen = set()

    def add_once(p: str):
        p2 = os.path.abspath(p)
        if p2 not in seen and os.path.exists(p2):
            seen.add(p2)
            allow.append(p2)

    if policy.mode == "read-only":
        pass
    elif policy.mode == "danger-full-access":
        add_once(command_cwd)
    elif policy.mode == "workspace-write":
        for w in policy.get_writable_roots_with_cwd(policy_cwd):
            add_once(w.root)
        # ensure CWD if it's not covered
        if not any(command_cwd.startswith(x + os.sep) or command_cwd == x for x in allow):
            add_once(command_cwd)

    # TEMP/TMP pass-through (only if not read-only)
    if policy.mode != "read-only":
        for key in ("TEMP", "TMP"):
            val = env_map.get(key) or os.environ.get(key)
            if val:
                add_once(val)

    guards: List[_AclGuard] = []
    for p in allow:
        try:
            added = _add_allow_ace(p, psid)
            if added:
                guards.append(_AclGuard(p, psid))
        except Exception as e:
            # best effort — continue
            print(f"[sandbox] failed to allow write on {p}: {e}", file=sys.stderr)

    # Best-effort NUL permission for strict mode; harmless elsewhere
    _allow_null_device(psid)
    return guards

# ---- Job object --------------------------------------------------------------

def _create_job_kill_on_close() -> wt.HANDLE:
    h = kernel32.CreateJobObjectW(None, None)
    if not h:
        raise c.WinError(c.get_last_error())
    limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION()
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
    ok = kernel32.SetInformationJobObject(
        h, JobObjectExtendedLimitInformation,
        c.byref(limits), wt.DWORD(c.sizeof(JOBOBJECT_EXTENDED_LIMIT_INFORMATION))
    )
    if not ok:
        err = c.get_last_error()
        kernel32.CloseHandle(h)
        raise c.WinError(err)
    return h

# ---- Pipe helpers for strict read-only stdio --------------------------------

def _make_inheritable_pipe() -> Tuple[wt.HANDLE, wt.HANDLE]:
    """Return (read_handle, write_handle), both inheritable."""
    h_read = wt.HANDLE()
    h_write = wt.HANDLE()
    _check_bool(kernel32.CreatePipe(c.byref(h_read), c.byref(h_write), None, 0), "CreatePipe")
    _check_bool(kernel32.SetHandleInformation(h_read,  HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT), "SetHandleInformation")
    _check_bool(kernel32.SetHandleInformation(h_write, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT), "SetHandleInformation")
    return h_read, h_write

def _drain_handle_to_stream(h_read: wt.HANDLE, py_stream):
    """Blocking drain: read all bytes from h_read and write to given Python stream."""
    buf = (c.c_char * 8192)()
    nread = wt.DWORD()
    while True:
        ok = kernel32.ReadFile(h_read, buf, len(buf), c.byref(nread), None)
        if not ok or nread.value == 0:
            break
        try:
            py_stream.buffer.write(buf[:nread.value])
        except Exception:
            py_stream.write(buf[:nread.value].decode(errors="replace"))
        py_stream.flush()

# ---- Process spawn -----------------------------------------------------------

def _create_process_as_user(
    h_token: wt.HANDLE,
    argv: List[str],
    cwd: str,
    env: Dict[str, str],
    *,
    std_handles: Optional[Tuple[wt.HANDLE, wt.HANDLE, wt.HANDLE]] = None  # (stdin, stdout, stderr)
) -> Tuple[PROCESS_INFORMATION, STARTUPINFOW]:
    # Build command line
    def quote_arg(a: str) -> str:
        if not a or any(ch in a for ch in ' \t\n\r\v"'):
            # windows quoting
            bs = 0
            out = '"'
            for ch in a:
                if ch == '\\':
                    bs += 1
                elif ch == '"':
                    out += '\\' * (bs * 2 + 1)
                    out += '"'
                    bs = 0
                else:
                    if bs:
                        out += '\\' * (bs * 2)
                        bs = 0
                    out += ch
            if bs:
                out += '\\' * (bs * 2)
            out += '"'
            return out
        return a

    cmdline_str = " ".join(quote_arg(a) for a in argv)
    cmdline_buf = c.create_unicode_buffer(cmdline_str)

    env_block = _make_env_block(env)
    env_buf = None
    lp_env = None
    if env_block:
        env_buf = c.create_string_buffer(env_block)
        lp_env = c.cast(env_buf, wt.LPVOID)

    si = STARTUPINFOW()
    si.cb = c.sizeof(STARTUPINFOW)

    if std_handles is None:
        _ensure_inheritable_stdio(si)
    else:
        si.dwFlags |= STARTF_USESTDHANDLES
        si.hStdInput, si.hStdOutput, si.hStdError = std_handles

    pi = PROCESS_INFORMATION()

    ok = advapi32.CreateProcessAsUserW(
        h_token,
        None,
        cmdline_buf,
        None, None,
        True,
        CREATE_UNICODE_ENVIRONMENT,
        lp_env,
        cwd if cwd else None,
        c.byref(si),
        c.byref(pi)
    )
    if not ok:
        raise c.WinError(c.get_last_error())
    return pi, si

def _wait_process_and_exitcode(pi: PROCESS_INFORMATION) -> int:
    res = kernel32.WaitForSingleObject(pi.hProcess, INFINITE)
    if res != WAIT_OBJECT_0:
        raise c.WinError(c.get_last_error())
    exit_code = wt.DWORD()
    _check_bool(kernel32.GetExitCodeProcess(pi.hProcess, c.byref(exit_code)), "GetExitCodeProcess")
    return int(exit_code.value)

# ---- Public entrypoint-like main (mirrors the Rust CLI) ----------------------

def ensure_non_interactive_pager(env_map: Dict[str, str]):
    env_map.setdefault("GIT_PAGER", "more.com")
    env_map.setdefault("PAGER", "more.com")
    env_map.setdefault("LESS", "")

def apply_best_effort_network_block(env_map: Dict[str, str]):
    sink = "http://127.0.0.1:9"
    env_map.setdefault("HTTP_PROXY", sink)
    env_map.setdefault("HTTPS_PROXY", sink)
    env_map.setdefault("ALL_PROXY", sink)
    env_map.setdefault("NO_PROXY", "localhost,127.0.0.1,::1")
    env_map.setdefault("PIP_NO_INDEX", "1")
    env_map.setdefault("PIP_DISABLE_PIP_VERSION_CHECK", "1")
    env_map.setdefault("NPM_CONFIG_OFFLINE", "true")
    env_map.setdefault("CARGO_NET_OFFLINE", "true")

def parse_policy_arg(value: str) -> SandboxPolicy:
    if value == "read-only":
        return SandboxPolicy.new_read_only_policy()
    if value == "workspace-write":
        # Let JSON override to pass explicit roots; plain preset means "derive from cwd".
        return SandboxPolicy.new_workspace_write_policy()
    if value == "danger-full-access":
        return SandboxPolicy.danger_full_access()
    # JSON path: { "mode": "...", "workspace_roots": ["..."] }
    try:
        obj = json.loads(value)
        mode = obj.get("mode")
        if not mode:
            raise ValueError("policy JSON missing 'mode'")
        roots = obj.get("workspace_roots", [])
        return SandboxPolicy(mode, roots)
    except Exception as e:
        raise argparse.ArgumentTypeError(f"failed to parse sandbox policy: {e}")

def main():
    if os.name != "nt":
        print("codex-windows-sandbox is only supported on Windows", file=sys.stderr)
        sys.exit(2)

    parser = argparse.ArgumentParser(
        prog="codex-windows-sandbox",
        description="Run a command inside a Windows restricted-token sandbox (no admin)",
    )
    parser.add_argument("--sandbox-policy-cwd", default=None, help="Base dir for policy-relative paths")
    parser.add_argument("sandbox_policy", type=parse_policy_arg,
                        help="Preset ('workspace-write' | 'read-only' | 'danger-full-access') or JSON")
    parser.add_argument("command", nargs=argparse.REMAINDER,
                        help="Command and args (everything after policy)")

    ns = parser.parse_args()
    command: List[str] = [arg for arg in ns.command if arg != "--"] if ns.command else []
    if not command:
        print("No command specified to execute.", file=sys.stderr)
        sys.exit(2)

    policy: SandboxPolicy = ns.sandbox_policy
    try:
        current_dir = os.getcwd()
    except Exception as e:
        print(f"failed to get current dir: {e}", file=sys.stderr)
        sys.exit(1)

    policy_cwd = ns.sandbox_policy_cwd or current_dir
    env_map: Dict[str, str] = dict(os.environ)

    normalize_null_device_env(env_map)
    #if not policy.has_full_network_access():
    #    apply_best_effort_network_block(env_map)
    ensure_non_interactive_pager(env_map)
    #if os.environ.get("SANDBOX_NO_NET") == "1":
    apply_no_network_to_env(env_map, NoNetConfig())

    # Create restricted token (WRITE_RESTRICTED + LUA), choose strict/compat
    try:
        if policy.mode == "read-only":
            h_token, psid = _create_write_restricted_token_strict()
        else:
            # workspace-write and danger-full-access
            h_token, psid = _create_write_restricted_token_compat()
    except Exception as e:
        print(f"failed to create restricted token: {e}", file=sys.stderr)
        sys.exit(1)

    # --- Diagnostics: show token restricted flag, SID, and CWD + dump restricted SID set
    try:
        is_restricted = bool(advapi32.IsTokenRestricted(h_token))
        #print(f"[sandbox:debug] policy={policy.mode} restricted={is_restricted} "
        #      f"logonSID={_sid_str(psid)} cwd={current_dir}", file=sys.stderr)
        _dump_restricted_sids(h_token)
        _cwd_effective_write_report(current_dir, psid)
    except Exception:
        pass

    # Configure writable directories by appending allow ACEs for the Logon SID
    acl_guards: List[_AclGuard] = []
    try:
        acl_guards = _configure_paths(policy, policy_cwd, current_dir, psid, env_map)
        # Spawn process with inherited stdio; attach to a kill-on-close job
        command_preview = _format_command_for_log(command)
        _log_command_start(command_preview)
        try:
            pi, si = _create_process_as_user(h_token, command, current_dir, env_map)
        except Exception as e:
            _log_command_failure(command_preview, f"spawn failed: {e}")
            print(f"failed to spawn process: {e}", file=sys.stderr)
            sys.exit(1)

        code = 1
        h_job = None
        try:
            h_job = _create_job_kill_on_close()
            _check_bool(kernel32.AssignProcessToJobObject(h_job, pi.hProcess),
                        "AssignProcessToJobObject")
            code = _wait_process_and_exitcode(pi)
        except Exception as e:
            _log_command_failure(command_preview, str(e))
            raise
        else:
            if code == 0:
                _log_command_success(command_preview)
            else:
                _log_command_failure(command_preview, f"exit code {code}")
        finally:
            if pi.hThread:
                kernel32.CloseHandle(pi.hThread)
            if pi.hProcess:
                kernel32.CloseHandle(pi.hProcess)
            if h_job:
                kernel32.CloseHandle(h_job)
        sys.exit(code)
    finally:
        # Best-effort cleanup: revoke ACEs we added
        for g in acl_guards:
            g.close()
        _close_handle_safe(h_token)

if __name__ == "__main__":
    main()
