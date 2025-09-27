import os
import sys
import time
import select
import ctypes


def main() -> int:
    # Fork the child that will exit shortly.
    pid = os.fork()
    if pid == 0:
        time.sleep(0.2)
        sys.exit(0)

    # Query process group via sysctl MIB: {CTL_KERN, KERN_PROC, KERN_PROC_PGRP, pgid}.
    libSystem = ctypes.CDLL("/usr/lib/libSystem.B.dylib")
    sysctl = libSystem.sysctl
    sysctl.argtypes = [
        ctypes.POINTER(ctypes.c_int),
        ctypes.c_uint,
        ctypes.c_void_p,
        ctypes.POINTER(ctypes.c_size_t),
        ctypes.c_void_p,
        ctypes.c_size_t,
    ]
    sysctl.restype = ctypes.c_int

    CTL_KERN = 1
    KERN_PROC = 14
    KERN_PROC_PGRP = 2

    pgid = os.getpgid(0)
    mib = (ctypes.c_int * 4)(CTL_KERN, KERN_PROC, KERN_PROC_PGRP, pgid)
    sz = ctypes.c_size_t(0)
    rc = sysctl(mib, 4, None, ctypes.byref(sz), None, 0)
    if rc != 0 or sz.value == 0:
        return 1
    buf = (ctypes.c_char * sz.value)()
    rc2 = sysctl(mib, 4, buf, ctypes.byref(sz), None, 0)
    if rc2 != 0 or sz.value == 0:
        return 1

    # Register kqueue EVFILT_PROC NOTE_EXIT for child pid and wait.
    kq = select.kqueue()
    kev = select.kevent(
        pid,
        filter=select.KQ_FILTER_PROC,
        flags=select.KQ_EV_ADD | select.KQ_EV_ENABLE,
        fflags=select.KQ_NOTE_EXIT,
    )
    kq.control([kev], 0, 0)
    events = kq.control(None, 1, None)
    ok_ev = len(events) == 1 and (events[0].fflags & select.KQ_NOTE_EXIT) != 0

    try:
        os.waitpid(pid, 0)
    except Exception:
        pass

    return 0 if ok_ev else 1


if __name__ == "__main__":
    sys.exit(main())

