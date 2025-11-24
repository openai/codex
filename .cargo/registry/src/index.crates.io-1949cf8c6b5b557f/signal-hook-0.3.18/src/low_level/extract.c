/*
 * Low-level extraction code to overcome rust's libc not having the best access
 * to siginfo_t details.
 */
#include <stdbool.h>
#include <signal.h>
#include <stdint.h>

struct Const {
    int native;
    // The signal this applies to, or -1 if it applies to anything.
    int signal;
    uint8_t translated;
};

// Warning: must be in sync with the rust source code
struct Const consts[] = {
#ifdef SI_KERNEL
    { SI_KERNEL, -1, 1 },
#endif
    { SI_USER, -1, 2 },
#ifdef SI_TKILL
    { SI_TKILL, -1, 3 },
#endif
    { SI_QUEUE, -1, 4 },
#ifdef SI_MESGQ
    { SI_MESGQ, -1, 5 },
#endif
    { CLD_EXITED, SIGCHLD, 6 },
    { CLD_KILLED, SIGCHLD, 7 },
    { CLD_DUMPED, SIGCHLD, 8 },
    { CLD_TRAPPED, SIGCHLD, 9 },
    { CLD_STOPPED, SIGCHLD, 10 },
    { CLD_CONTINUED, SIGCHLD, 11 },
};

uint8_t sighook_signal_cause(const siginfo_t *info) {
    const size_t const_len = sizeof consts / sizeof *consts;
    size_t i;
    for (i = 0; i < const_len; i ++) {
        if (
            consts[i].native == info->si_code &&
            (consts[i].signal == -1 || consts[i].signal == info->si_signo)
        ) {
            return consts[i].translated;
        }
    }
    return 0; // The "Unknown" variant
}

pid_t sighook_signal_pid(const siginfo_t *info) {
    return info->si_pid;
}

uid_t sighook_signal_uid(const siginfo_t *info) {
    return info->si_uid;
}
