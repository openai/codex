#pragma once

#include <linux/capability.h>

// bubblewrap only needs `cap_value_t` and `cap_from_name` from libcap's
// sys/capability.h. Keeping this shim minimal avoids forcing host include
// paths during cross-compilation.
typedef int cap_value_t;
int capget(struct __user_cap_header_struct *hdrp, struct __user_cap_data_struct *datap);
int capset(struct __user_cap_header_struct *hdrp, const struct __user_cap_data_struct *datap);
int cap_from_name(const char *name, cap_value_t *cap_p);
