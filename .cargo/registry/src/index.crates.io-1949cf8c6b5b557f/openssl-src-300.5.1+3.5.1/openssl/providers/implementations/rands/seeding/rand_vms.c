/*
 * Copyright 2001-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/e_os.h"

#define __NEW_STARLET 1         /* New starlet definitions since VMS 7.0 */
#include <unistd.h>
#include "internal/cryptlib.h"
#include "internal/nelem.h"
#include <openssl/rand.h>
#include "crypto/rand.h"
#include "crypto/rand_pool.h"
#include "prov/seeding.h"
#include <descrip.h>
#include <dvidef.h>
#include <jpidef.h>
#include <rmidef.h>
#include <syidef.h>
#include <ssdef.h>
#include <starlet.h>
#include <efndef.h>
#include <gen64def.h>
#include <iosbdef.h>
#include <iledef.h>
#include <lib$routines.h>
#ifdef __DECC
# pragma message disable DOLLARID
#endif

#include <dlfcn.h>              /* SYS$GET_ENTROPY presence */

#ifndef OPENSSL_RAND_SEED_OS
# error "Unsupported seeding method configured; must be os"
#endif

/*
 * DATA COLLECTION METHOD
 * ======================
 *
 * This is a method to get low quality entropy.
 * It works by collecting all kinds of statistical data that
 * VMS offers and using them as random seed.
 */

/* We need to make sure we have the right size pointer in some cases */
#if __INITIAL_POINTER_SIZE == 64
# pragma pointer_size save
# pragma pointer_size 32
#endif
typedef uint32_t *uint32_t__ptr32;
#if __INITIAL_POINTER_SIZE == 64
# pragma pointer_size restore
#endif

struct item_st {
    short length, code;         /* length is number of bytes */
};

static const struct item_st DVI_item_data[] = {
    {4,   DVI$_ERRCNT},
    {4,   DVI$_REFCNT},
};

static const struct item_st JPI_item_data[] = {
    {4,   JPI$_BUFIO},
    {4,   JPI$_CPUTIM},
    {4,   JPI$_DIRIO},
    {4,   JPI$_IMAGECOUNT},
    {4,   JPI$_PAGEFLTS},
    {4,   JPI$_PID},
    {4,   JPI$_PPGCNT},
    {4,   JPI$_WSPEAK},
    /*
     * Note: the direct result is just a 32-bit address.  However, it points
     * to a list of 4 32-bit words, so we make extra space for them so we can
     * do in-place replacement of values
     */
    {16,  JPI$_FINALEXC},
};

static const struct item_st JPI_item_data_64bit[] = {
    {8,   JPI$_LAST_LOGIN_I},
    {8,   JPI$_LOGINTIM},
};

static const struct item_st RMI_item_data[] = {
    {4,   RMI$_COLPG},
    {4,   RMI$_MWAIT},
    {4,   RMI$_CEF},
    {4,   RMI$_PFW},
    {4,   RMI$_LEF},
    {4,   RMI$_LEFO},
    {4,   RMI$_HIB},
    {4,   RMI$_HIBO},
    {4,   RMI$_SUSP},
    {4,   RMI$_SUSPO},
    {4,   RMI$_FPG},
    {4,   RMI$_COM},
    {4,   RMI$_COMO},
    {4,   RMI$_CUR},
#if defined __alpha
    {4,   RMI$_FRLIST},
    {4,   RMI$_MODLIST},
#endif
    {4,   RMI$_FAULTS},
    {4,   RMI$_PREADS},
    {4,   RMI$_PWRITES},
    {4,   RMI$_PWRITIO},
    {4,   RMI$_PREADIO},
    {4,   RMI$_GVALFLTS},
    {4,   RMI$_WRTINPROG},
    {4,   RMI$_FREFLTS},
    {4,   RMI$_DZROFLTS},
    {4,   RMI$_SYSFAULTS},
    {4,   RMI$_ISWPCNT},
    {4,   RMI$_DIRIO},
    {4,   RMI$_BUFIO},
    {4,   RMI$_MBREADS},
    {4,   RMI$_MBWRITES},
    {4,   RMI$_LOGNAM},
    {4,   RMI$_FCPCALLS},
    {4,   RMI$_FCPREAD},
    {4,   RMI$_FCPWRITE},
    {4,   RMI$_FCPCACHE},
    {4,   RMI$_FCPCPU},
    {4,   RMI$_FCPHIT},
    {4,   RMI$_FCPSPLIT},
    {4,   RMI$_FCPFAULT},
    {4,   RMI$_ENQNEW},
    {4,   RMI$_ENQCVT},
    {4,   RMI$_DEQ},
    {4,   RMI$_BLKAST},
    {4,   RMI$_ENQWAIT},
    {4,   RMI$_ENQNOTQD},
    {4,   RMI$_DLCKSRCH},
    {4,   RMI$_DLCKFND},
    {4,   RMI$_NUMLOCKS},
    {4,   RMI$_NUMRES},
    {4,   RMI$_ARRLOCPK},
    {4,   RMI$_DEPLOCPK},
    {4,   RMI$_ARRTRAPK},
    {4,   RMI$_TRCNGLOS},
    {4,   RMI$_RCVBUFFL},
    {4,   RMI$_ENQNEWLOC},
    {4,   RMI$_ENQNEWIN},
    {4,   RMI$_ENQNEWOUT},
    {4,   RMI$_ENQCVTLOC},
    {4,   RMI$_ENQCVTIN},
    {4,   RMI$_ENQCVTOUT},
    {4,   RMI$_DEQLOC},
    {4,   RMI$_DEQIN},
    {4,   RMI$_DEQOUT},
    {4,   RMI$_BLKLOC},
    {4,   RMI$_BLKIN},
    {4,   RMI$_BLKOUT},
    {4,   RMI$_DIRIN},
    {4,   RMI$_DIROUT},
    /* We currently get a fault when trying these */
#if 0
    {140, RMI$_MSCP_EVERYTHING},   /* 35 32-bit words */
    {152, RMI$_DDTM_ALL},          /* 38 32-bit words */
    {80,  RMI$_TMSCP_EVERYTHING}   /* 20 32-bit words */
#endif
    {4,   RMI$_LPZ_PAGCNT},
    {4,   RMI$_LPZ_HITS},
    {4,   RMI$_LPZ_MISSES},
    {4,   RMI$_LPZ_EXPCNT},
    {4,   RMI$_LPZ_ALLOCF},
    {4,   RMI$_LPZ_ALLOC2},
    {4,   RMI$_ACCESS},
    {4,   RMI$_ALLOC},
    {4,   RMI$_FCPCREATE},
    {4,   RMI$_VOLWAIT},
    {4,   RMI$_FCPTURN},
    {4,   RMI$_FCPERASE},
    {4,   RMI$_OPENS},
    {4,   RMI$_FIDHIT},
    {4,   RMI$_FIDMISS},
    {4,   RMI$_FILHDR_HIT},
    {4,   RMI$_DIRFCB_HIT},
    {4,   RMI$_DIRFCB_MISS},
    {4,   RMI$_DIRDATA_HIT},
    {4,   RMI$_EXTHIT},
    {4,   RMI$_EXTMISS},
    {4,   RMI$_QUOHIT},
    {4,   RMI$_QUOMISS},
    {4,   RMI$_STORAGMAP_HIT},
    {4,   RMI$_VOLLCK},
    {4,   RMI$_SYNCHLCK},
    {4,   RMI$_SYNCHWAIT},
    {4,   RMI$_ACCLCK},
    {4,   RMI$_XQPCACHEWAIT},
    {4,   RMI$_DIRDATA_MISS},
    {4,   RMI$_FILHDR_MISS},
    {4,   RMI$_STORAGMAP_MISS},
    {4,   RMI$_PROCCNTMAX},
    {4,   RMI$_PROCBATCNT},
    {4,   RMI$_PROCINTCNT},
    {4,   RMI$_PROCNETCNT},
    {4,   RMI$_PROCSWITCHCNT},
    {4,   RMI$_PROCBALSETCNT},
    {4,   RMI$_PROCLOADCNT},
    {4,   RMI$_BADFLTS},
    {4,   RMI$_EXEFAULTS},
    {4,   RMI$_HDRINSWAPS},
    {4,   RMI$_HDROUTSWAPS},
    {4,   RMI$_IOPAGCNT},
    {4,   RMI$_ISWPCNTPG},
    {4,   RMI$_OSWPCNT},
    {4,   RMI$_OSWPCNTPG},
    {4,   RMI$_RDFAULTS},
    {4,   RMI$_TRANSFLTS},
    {4,   RMI$_WRTFAULTS},
#if defined __alpha
    {4,   RMI$_USERPAGES},
#endif
    {4,   RMI$_VMSPAGES},
    {4,   RMI$_TTWRITES},
    {4,   RMI$_BUFOBJPAG},
    {4,   RMI$_BUFOBJPAGPEAK},
    {4,   RMI$_BUFOBJPAGS01},
    {4,   RMI$_BUFOBJPAGS2},
    {4,   RMI$_BUFOBJPAGMAXS01},
    {4,   RMI$_BUFOBJPAGMAXS2},
    {4,   RMI$_BUFOBJPAGPEAKS01},
    {4,   RMI$_BUFOBJPAGPEAKS2},
    {4,   RMI$_BUFOBJPGLTMAXS01},
    {4,   RMI$_BUFOBJPGLTMAXS2},
    {4,   RMI$_DLCK_INCMPLT},
    {4,   RMI$_DLCKMSGS_IN},
    {4,   RMI$_DLCKMSGS_OUT},
    {4,   RMI$_MCHKERRS},
    {4,   RMI$_MEMERRS},
};

static const struct item_st RMI_item_data_64bit[] = {
#if defined __ia64
    {8,   RMI$_FRLIST},
    {8,   RMI$_MODLIST},
#endif
    {8,   RMI$_LCKMGR_REQCNT},
    {8,   RMI$_LCKMGR_REQTIME},
    {8,   RMI$_LCKMGR_SPINCNT},
    {8,   RMI$_LCKMGR_SPINTIME},
    {8,   RMI$_CPUINTSTK},
    {8,   RMI$_CPUMPSYNCH},
    {8,   RMI$_CPUKERNEL},
    {8,   RMI$_CPUEXEC},
    {8,   RMI$_CPUSUPER},
    {8,   RMI$_CPUUSER},
#if defined __ia64
    {8,   RMI$_USERPAGES},
#endif
    {8,   RMI$_TQETOTAL},
    {8,   RMI$_TQESYSUB},
    {8,   RMI$_TQEUSRTIMR},
    {8,   RMI$_TQEUSRWAKE},
};

static const struct item_st SYI_item_data[] = {
    {4,   SYI$_PAGEFILE_FREE},
};

/*
 * Input:
 * items_data           - an array of lengths and codes
 * items_data_num       - number of elements in that array
 *
 * Output:
 * items                - pre-allocated ILE3 array to be filled.
 *                        It's assumed to have items_data_num elements plus
 *                        one extra for the terminating NULL element
 * databuffer           - pre-allocated 32-bit word array.
 *
 * Returns the number of elements used in databuffer
 */
static size_t prepare_item_list(const struct item_st *items_input,
                                size_t items_input_num,
                                ILE3 *items,
                                uint32_t__ptr32 databuffer)
{
    size_t data_sz = 0;

    for (; items_input_num-- > 0; items_input++, items++) {

        items->ile3$w_code = items_input->code;
        /* Special treatment of JPI$_FINALEXC */
        if (items->ile3$w_code == JPI$_FINALEXC)
            items->ile3$w_length = 4;
        else
            items->ile3$w_length = items_input->length;

        items->ile3$ps_bufaddr = databuffer;
        items->ile3$ps_retlen_addr = 0;

        databuffer += items_input->length / sizeof(databuffer[0]);
        data_sz += items_input->length;
    }
    /* Terminating NULL entry */
    items->ile3$w_length = items->ile3$w_code = 0;
    items->ile3$ps_bufaddr = items->ile3$ps_retlen_addr = NULL;

    return data_sz / sizeof(databuffer[0]);
}

static void massage_JPI(ILE3 *items)
{
    /*
     * Special treatment of JPI$_FINALEXC
     * The result of that item's data buffer is a 32-bit address to a list of
     * 4 32-bit words.
     */
    for (; items->ile3$w_length != 0; items++) {
        if (items->ile3$w_code == JPI$_FINALEXC) {
            uint32_t *data = items->ile3$ps_bufaddr;
            uint32_t *ptr = (uint32_t *)*data;
            size_t j;

            /*
             * We know we made space for 4 32-bit words, so we can do in-place
             * replacement.
             */
            for (j = 0; j < 4; j++)
                data[j] = ptr[j];

            break;
        }
    }
}

/*
 * This number expresses how many bits of data contain 1 bit of entropy.
 *
 * For the moment, we assume about 0.05 entropy bits per data bit, or 1
 * bit of entropy per 20 data bits.
 */
#define ENTROPY_FACTOR  20

size_t data_collect_method(RAND_POOL *pool)
{
    ILE3 JPI_items_64bit[OSSL_NELEM(JPI_item_data_64bit) + 1];
    ILE3 RMI_items_64bit[OSSL_NELEM(RMI_item_data_64bit) + 1];
    ILE3 DVI_items[OSSL_NELEM(DVI_item_data) + 1];
    ILE3 JPI_items[OSSL_NELEM(JPI_item_data) + 1];
    ILE3 RMI_items[OSSL_NELEM(RMI_item_data) + 1];
    ILE3 SYI_items[OSSL_NELEM(SYI_item_data) + 1];
    union {
        /* This ensures buffer starts at 64 bit boundary */
        uint64_t dummy;
        uint32_t buffer[OSSL_NELEM(JPI_item_data_64bit) * 2
                        + OSSL_NELEM(RMI_item_data_64bit) * 2
                        + OSSL_NELEM(DVI_item_data)
                        + OSSL_NELEM(JPI_item_data)
                        + OSSL_NELEM(RMI_item_data)
                        + OSSL_NELEM(SYI_item_data)
                        + 4 /* For JPI$_FINALEXC */];
    } data;
    size_t total_elems = 0;
    size_t total_length = 0;
    size_t bytes_needed = ossl_rand_pool_bytes_needed(pool, ENTROPY_FACTOR);
    size_t bytes_remaining = ossl_rand_pool_bytes_remaining(pool);

    /* Take all the 64-bit items first, to ensure proper alignment of data */
    total_elems +=
        prepare_item_list(JPI_item_data_64bit, OSSL_NELEM(JPI_item_data_64bit),
                          JPI_items_64bit, &data.buffer[total_elems]);
    total_elems +=
        prepare_item_list(RMI_item_data_64bit, OSSL_NELEM(RMI_item_data_64bit),
                          RMI_items_64bit, &data.buffer[total_elems]);
    /* Now the 32-bit items */
    total_elems += prepare_item_list(DVI_item_data, OSSL_NELEM(DVI_item_data),
                                     DVI_items, &data.buffer[total_elems]);
    total_elems += prepare_item_list(JPI_item_data, OSSL_NELEM(JPI_item_data),
                                     JPI_items, &data.buffer[total_elems]);
    total_elems += prepare_item_list(RMI_item_data, OSSL_NELEM(RMI_item_data),
                                     RMI_items, &data.buffer[total_elems]);
    total_elems += prepare_item_list(SYI_item_data, OSSL_NELEM(SYI_item_data),
                                     SYI_items, &data.buffer[total_elems]);
    total_length = total_elems * sizeof(data.buffer[0]);

    /* Fill data.buffer with various info bits from this process */
    {
        uint32_t status;
        uint32_t efn;
        IOSB iosb;
        $DESCRIPTOR(SYSDEVICE, "SYS$SYSDEVICE:");

        if ((status = sys$getdviw(EFN$C_ENF, 0, &SYSDEVICE, DVI_items,
                                  0, 0, 0, 0, 0)) != SS$_NORMAL) {
            lib$signal(status);
            return 0;
        }
        if ((status = sys$getjpiw(EFN$C_ENF, 0, 0, JPI_items_64bit, 0, 0, 0))
            != SS$_NORMAL) {
            lib$signal(status);
            return 0;
        }
        if ((status = sys$getjpiw(EFN$C_ENF, 0, 0, JPI_items, 0, 0, 0))
            != SS$_NORMAL) {
            lib$signal(status);
            return 0;
        }
        if ((status = sys$getsyiw(EFN$C_ENF, 0, 0, SYI_items, 0, 0, 0))
            != SS$_NORMAL) {
            lib$signal(status);
            return 0;
        }
        /*
         * The RMI service is a bit special, as there is no synchronous
         * variant, so we MUST create an event flag to synchronise on.
         */
        if ((status = lib$get_ef(&efn)) != SS$_NORMAL) {
            lib$signal(status);
            return 0;
        }
        if ((status = sys$getrmi(efn, 0, 0, RMI_items_64bit, &iosb, 0, 0))
            != SS$_NORMAL) {
            lib$signal(status);
            return 0;
        }
        if ((status = sys$synch(efn, &iosb)) != SS$_NORMAL) {
            lib$signal(status);
            return 0;
        }
        if (iosb.iosb$l_getxxi_status != SS$_NORMAL) {
            lib$signal(iosb.iosb$l_getxxi_status);
            return 0;
        }
        if ((status = sys$getrmi(efn, 0, 0, RMI_items, &iosb, 0, 0))
            != SS$_NORMAL) {
            lib$signal(status);
            return 0;
        }
        if ((status = sys$synch(efn, &iosb)) != SS$_NORMAL) {
            lib$signal(status);
            return 0;
        }
        if (iosb.iosb$l_getxxi_status != SS$_NORMAL) {
            lib$signal(iosb.iosb$l_getxxi_status);
            return 0;
        }
        if ((status = lib$free_ef(&efn)) != SS$_NORMAL) {
            lib$signal(status);
            return 0;
        }
    }

    massage_JPI(JPI_items);

    /*
     * If we can't feed the requirements from the caller, we're in deep trouble.
     */
    if (!ossl_assert(total_length >= bytes_needed)) {
        ERR_raise_data(ERR_LIB_RAND, RAND_R_RANDOM_POOL_UNDERFLOW,
                       "Needed: %zu, Available: %zu",
                       bytes_needed, total_length);
        return 0;
    }

    /*
     * Try not to overfeed the pool
     */
    if (total_length > bytes_remaining)
        total_length = bytes_remaining;

    /* We give the pessimistic value for the amount of entropy */
    ossl_rand_pool_add(pool, (unsigned char *)data.buffer, total_length,
                       8 * total_length / ENTROPY_FACTOR);
    return ossl_rand_pool_entropy_available(pool);
}

/*
 * SYS$GET_ENTROPY METHOD
 * ======================
 *
 * This is a high entropy method based on a new system service that is
 * based on getentropy() from FreeBSD 12.  It's only used if available,
 * and its availability is detected at run-time.
 *
 * We assume that this function provides full entropy random output.
 */
#define PUBLIC_VECTORS "SYS$LIBRARY:SYS$PUBLIC_VECTORS.EXE"
#define GET_ENTROPY "SYS$GET_ENTROPY"

static int get_entropy_address_flag = 0;
static int (*get_entropy_address)(void *buffer, size_t buffer_size) = NULL;
static int init_get_entropy_address(void)
{
    if (get_entropy_address_flag == 0)
        get_entropy_address = dlsym(dlopen(PUBLIC_VECTORS, 0), GET_ENTROPY);
    get_entropy_address_flag = 1;
    return get_entropy_address != NULL;
}

size_t get_entropy_method(RAND_POOL *pool)
{
    /*
     * The documentation says that SYS$GET_ENTROPY will give a maximum of
     * 256 bytes of data.
     */
    unsigned char buffer[256];
    size_t bytes_needed;
    size_t bytes_to_get = 0;
    uint32_t status;

    for (bytes_needed = ossl_rand_pool_bytes_needed(pool, 1);
         bytes_needed > 0;
         bytes_needed -= bytes_to_get) {
        bytes_to_get =
            bytes_needed > sizeof(buffer) ? sizeof(buffer) : bytes_needed;

        status = get_entropy_address(buffer, bytes_to_get);
        if (status == SS$_RETRY) {
            /* Set to zero so the loop doesn't diminish |bytes_needed| */
            bytes_to_get = 0;
            /* Should sleep some amount of time */
            continue;
        }

        if (status != SS$_NORMAL) {
            lib$signal(status);
            return 0;
        }

        ossl_rand_pool_add(pool, buffer, bytes_to_get, 8 * bytes_to_get);
    }

    return ossl_rand_pool_entropy_available(pool);
}

/*
 * MAIN ENTROPY ACQUISITION FUNCTIONS
 * ==================================
 *
 * These functions are called by the RAND / DRBG functions
 */

size_t ossl_pool_acquire_entropy(RAND_POOL *pool)
{
    if (init_get_entropy_address())
        return get_entropy_method(pool);
    return data_collect_method(pool);
}

int ossl_pool_add_nonce_data(RAND_POOL *pool)
{
    /*
     * Two variables to ensure that two nonces won't ever be the same
     */
    static unsigned __int64 last_time = 0;
    static unsigned __int32 last_seq = 0;

    struct {
        pid_t pid;
        CRYPTO_THREAD_ID tid;
        unsigned __int64 time;
        unsigned __int32 seq;
    } data;

    /* Erase the entire structure including any padding */
    memset(&data, 0, sizeof(data));

    /*
     * Add process id, thread id, a timestamp, and a sequence number in case
     * the same time stamp is repeated, to ensure that the nonce is unique
     * with high probability for different process instances.
     *
     * The normal OpenVMS time is specified to be high granularity (100ns),
     * but the time update granularity given by sys$gettim() may be lower.
     *
     * OpenVMS version 8.4 (which is the latest for Alpha and Itanium) and
     * on have sys$gettim_prec() as well, which is supposedly having a better
     * time update granularity, but tests on Itanium (and even Alpha) have
     * shown that compared with sys$gettim(), the difference is marginal,
     * so of very little significance in terms of entropy.
     * Given that, and that it's a high ask to expect everyone to have
     * upgraded to OpenVMS version 8.4, only sys$gettim() is used, and a
     * sequence number is added as well, in case sys$gettim() returns the
     * same time value more than once.
     *
     * This function is assumed to be called under thread lock, and does
     * therefore not take concurrency into account.
     */
    data.pid = getpid();
    data.tid = CRYPTO_THREAD_get_current_id();
    data.seq = 0;
    sys$gettim((void*)&data.time);

    if (data.time == last_time) {
        data.seq = ++last_seq;
    } else {
        last_time = data.time;
        last_seq = 0;
    }

    return ossl_rand_pool_add(pool, (unsigned char *)&data, sizeof(data), 0);
}

int ossl_rand_pool_init(void)
{
    return 1;
}

void ossl_rand_pool_cleanup(void)
{
}

void ossl_rand_pool_keep_random_devices_open(int keep)
{
}
