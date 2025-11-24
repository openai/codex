/*
 * Copyright 2010-2016 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#if defined( __VMS) && !defined( OPENSSL_NO_DECC_INIT) && \
 defined( __DECC) && !defined( __VAX) && (__CRTL_VER >= 70301000)
# define USE_DECC_INIT 1
#endif

#ifdef USE_DECC_INIT

/*
 * ----------------------------------------------------------------------
 * decc_init() On non-VAX systems, uses LIB$INITIALIZE to set a collection
 * of C RTL features without using the DECC$* logical name method.
 * ----------------------------------------------------------------------
 */

# include <stdio.h>
# include <stdlib.h>
# include <unixlib.h>

/* Global storage. */

/* Flag to sense if decc_init() was called. */

int decc_init_done = -1;

/* Structure to hold a DECC$* feature name and its desired value. */

typedef struct {
    char *name;
    int value;
} decc_feat_t;

/*
 * Array of DECC$* feature names and their desired values. Note:
 * DECC$ARGV_PARSE_STYLE is the urgent one.
 */

decc_feat_t decc_feat_array[] = {
    /* Preserve command-line case with SET PROCESS/PARSE_STYLE=EXTENDED */
    {"DECC$ARGV_PARSE_STYLE", 1},

    /* Preserve case for file names on ODS5 disks. */
    {"DECC$EFS_CASE_PRESERVE", 1},

    /*
     * Enable multiple dots (and most characters) in ODS5 file names, while
     * preserving VMS-ness of ";version".
     */
    {"DECC$EFS_CHARSET", 1},

    /* List terminator. */
    {(char *)NULL, 0}
};


/* LIB$INITIALIZE initialization function. */

static void decc_init(void)
{
    char *openssl_debug_decc_init;
    int verbose = 0;
    int feat_index;
    int feat_value;
    int feat_value_max;
    int feat_value_min;
    int i;
    int sts;

    /* Get debug option. */
    openssl_debug_decc_init = getenv("OPENSSL_DEBUG_DECC_INIT");
    if (openssl_debug_decc_init != NULL) {
        verbose = strtol(openssl_debug_decc_init, NULL, 10);
        if (verbose <= 0) {
            verbose = 1;
        }
    }

    /* Set the global flag to indicate that LIB$INITIALIZE worked. */
    decc_init_done = 1;

    /* Loop through all items in the decc_feat_array[]. */

    for (i = 0; decc_feat_array[i].name != NULL; i++) {
        /* Get the feature index. */
        feat_index = decc$feature_get_index(decc_feat_array[i].name);
        if (feat_index >= 0) {
            /* Valid item.  Collect its properties. */
            feat_value = decc$feature_get_value(feat_index, 1);
            feat_value_min = decc$feature_get_value(feat_index, 2);
            feat_value_max = decc$feature_get_value(feat_index, 3);

            /* Check the validity of our desired value. */
            if ((decc_feat_array[i].value >= feat_value_min) &&
                (decc_feat_array[i].value <= feat_value_max)) {
                /* Valid value.  Set it if necessary. */
                if (feat_value != decc_feat_array[i].value) {
                    sts = decc$feature_set_value(feat_index,
                                                 1, decc_feat_array[i].value);

                    if (verbose > 1) {
                        fprintf(stderr, " %s = %d, sts = %d.\n",
                                decc_feat_array[i].name,
                                decc_feat_array[i].value, sts);
                    }
                }
            } else {
                /* Invalid DECC feature value. */
                fprintf(stderr,
                        " INVALID DECC$FEATURE VALUE, %d: %d <= %s <= %d.\n",
                        feat_value,
                        feat_value_min, decc_feat_array[i].name,
                        feat_value_max);
            }
        } else {
            /* Invalid DECC feature name. */
            fprintf(stderr,
                    " UNKNOWN DECC$FEATURE: %s.\n", decc_feat_array[i].name);
        }
    }

    if (verbose > 0) {
        fprintf(stderr, " DECC_INIT complete.\n");
    }
}

/* Get "decc_init()" into a valid, loaded LIB$INITIALIZE PSECT. */

# pragma nostandard

/*
 * Establish the LIB$INITIALIZE PSECTs, with proper alignment and other
 * attributes.  Note that "nopic" is significant only on VAX.
 */
# pragma extern_model save

# if __INITIAL_POINTER_SIZE == 64
#  define PSECT_ALIGN 3
# else
#  define PSECT_ALIGN 2
# endif

# pragma extern_model strict_refdef "LIB$INITIALIZ" PSECT_ALIGN, nopic, nowrt
const int spare[8] = { 0 };

# pragma extern_model strict_refdef "LIB$INITIALIZE" PSECT_ALIGN, nopic, nowrt
void (*const x_decc_init) () = decc_init;

# pragma extern_model restore

/* Fake reference to ensure loading the LIB$INITIALIZE PSECT. */

# pragma extern_model save

int LIB$INITIALIZE(void);

# pragma extern_model strict_refdef
int dmy_lib$initialize = (int)LIB$INITIALIZE;

# pragma extern_model restore

# pragma standard

#else                           /* def USE_DECC_INIT */

/* Dummy code to avoid a %CC-W-EMPTYFILE complaint. */
int decc_init_dummy(void);

#endif                          /* def USE_DECC_INIT */
