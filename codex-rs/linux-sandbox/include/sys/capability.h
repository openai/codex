#pragma once

#include <linux/capability.h>

// bubblewrap only needs `cap_value_t` and `cap_from_name` from libcap's
// sys/capability.h. Keeping this shim minimal avoids forcing host include
// paths during cross-compilation.
typedef int cap_value_t;
int cap_from_name(const char *name, cap_value_t *cap_p);
