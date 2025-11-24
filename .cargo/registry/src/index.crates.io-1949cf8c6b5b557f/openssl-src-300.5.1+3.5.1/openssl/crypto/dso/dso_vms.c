/*
 * Copyright 2000-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "dso_local.h"

#ifdef OPENSSL_SYS_VMS

# pragma message disable DOLLARID
# include <errno.h>
# include <rms.h>
# include <lib$routines.h>
# include <libfisdef.h>
# include <stsdef.h>
# include <descrip.h>
# include <starlet.h>
# include "../vms_rms.h"

/* Some compiler options may mask the declaration of "_malloc32". */
# define DSO_MALLOC OPENSSL_malloc
# if __INITIAL_POINTER_SIZE && defined _ANSI_C_SOURCE
#  if __INITIAL_POINTER_SIZE == 64
#   pragma pointer_size save
#   pragma pointer_size 32
void *_malloc32(__size_t);
static void *dso_malloc(__size_t num, const char *file, int line)
{
    void *ret = _malloc32(num);
    if (ret == NULL && (file != NULL || line != 0)) {
        ERR_new();
        ERR_set_debug(file, line, NULL);
        ERR_set_error(ERR_LIB_DSO, ERR_R_MALLOC_FAILURE, NULL);
    }
    return ret;
}
#   undef DSO_MALLOC
#   define DSO_MALLOC(num) dso_malloc((num), OPENSSL_FILE, OPENSSL_LINE)
#   pragma pointer_size restore
#  endif                        /* __INITIAL_POINTER_SIZE == 64 [else] */
# endif                         /* __INITIAL_POINTER_SIZE && defined
                                 * _ANSI_C_SOURCE */

# pragma message disable DOLLARID

static int vms_load(DSO *dso);
static int vms_unload(DSO *dso);
static DSO_FUNC_TYPE vms_bind_func(DSO *dso, const char *symname);
static char *vms_name_converter(DSO *dso, const char *filename);
static char *vms_merger(DSO *dso, const char *filespec1,
                        const char *filespec2);

static DSO_METHOD dso_meth_vms = {
    "OpenSSL 'VMS' shared library method",
    vms_load,
    NULL,                       /* unload */
    vms_bind_func,
    NULL,                       /* ctrl */
    vms_name_converter,
    vms_merger,
    NULL,                       /* init */
    NULL,                       /* finish */
    NULL,                       /* pathbyaddr */
    NULL                        /* globallookup */
};

/*
 * On VMS, the only "handle" is the file name.  LIB$FIND_IMAGE_SYMBOL depends
 * on the reference to the file name being the same for all calls regarding
 * one shared image, so we'll just store it in an instance of the following
 * structure and put a pointer to that instance in the meth_data stack.
 */
typedef struct dso_internal_st {
    /*
     * This should contain the name only, no directory, no extension, nothing
     * but a name.
     */
    struct dsc$descriptor_s filename_dsc;
    char filename[NAMX_MAXRSS + 1];
    /*
     * This contains whatever is not in filename, if needed. Normally not
     * defined.
     */
    struct dsc$descriptor_s imagename_dsc;
    char imagename[NAMX_MAXRSS + 1];
} DSO_VMS_INTERNAL;

DSO_METHOD *DSO_METHOD_openssl(void)
{
    return &dso_meth_vms;
}

static int vms_load(DSO *dso)
{
    void *ptr = NULL;
    /* See applicable comments in dso_dl.c */
    char *filename = DSO_convert_filename(dso, NULL);

/* Ensure 32-bit pointer for "p", and appropriate malloc() function. */
# if __INITIAL_POINTER_SIZE && defined _ANSI_C_SOURCE
#  if __INITIAL_POINTER_SIZE == 64
#   pragma pointer_size save
#   pragma pointer_size 32
#  endif                        /* __INITIAL_POINTER_SIZE == 64 */
# endif                         /* __INITIAL_POINTER_SIZE && defined
                                 * _ANSI_C_SOURCE */

    DSO_VMS_INTERNAL *p = NULL;

# if __INITIAL_POINTER_SIZE && defined _ANSI_C_SOURCE
#  if __INITIAL_POINTER_SIZE == 64
#   pragma pointer_size restore
#  endif                        /* __INITIAL_POINTER_SIZE == 64 */
# endif                         /* __INITIAL_POINTER_SIZE && defined
                                 * _ANSI_C_SOURCE */

    const char *sp1, *sp2;      /* Search result */
    const char *ext = NULL;     /* possible extension to add */

    if (filename == NULL) {
        ERR_raise(ERR_LIB_DSO, DSO_R_NO_FILENAME);
        goto err;
    }

    /*-
     * A file specification may look like this:
     *
     *      node::dev:[dir-spec]name.type;ver
     *
     * or (for compatibility with TOPS-20):
     *
     *      node::dev:<dir-spec>name.type;ver
     *
     * and the dir-spec uses '.' as separator.  Also, a dir-spec
     * may consist of several parts, with mixed use of [] and <>:
     *
     *      [dir1.]<dir2>
     *
     * We need to split the file specification into the name and
     * the rest (both before and after the name itself).
     */
    /*
     * Start with trying to find the end of a dir-spec, and save the position
     * of the byte after in sp1
     */
    sp1 = strrchr(filename, ']');
    sp2 = strrchr(filename, '>');
    if (sp1 == NULL)
        sp1 = sp2;
    if (sp2 != NULL && sp2 > sp1)
        sp1 = sp2;
    if (sp1 == NULL)
        sp1 = strrchr(filename, ':');
    if (sp1 == NULL)
        sp1 = filename;
    else
        sp1++;                  /* The byte after the found character */
    /* Now, let's see if there's a type, and save the position in sp2 */
    sp2 = strchr(sp1, '.');
    /*
     * If there is a period and the next character is a semi-colon,
     * we need to add an extension
     */
    if (sp2 != NULL && sp2[1] == ';')
        ext = ".EXE";
    /*
     * If we found it, that's where we'll cut.  Otherwise, look for a version
     * number and save the position in sp2
     */
    if (sp2 == NULL) {
        sp2 = strchr(sp1, ';');
        ext = ".EXE";
    }
    /*
     * If there was still nothing to find, set sp2 to point at the end of the
     * string
     */
    if (sp2 == NULL)
        sp2 = sp1 + strlen(sp1);

    /* Check that we won't get buffer overflows */
    if (sp2 - sp1 > FILENAME_MAX
        || (sp1 - filename) + strlen(sp2) > FILENAME_MAX) {
        ERR_raise(ERR_LIB_DSO, DSO_R_FILENAME_TOO_BIG);
        goto err;
    }

    p = DSO_MALLOC(sizeof(*p));
    if (p == NULL)
        goto err;

    strncpy(p->filename, sp1, sp2 - sp1);
    p->filename[sp2 - sp1] = '\0';

    strncpy(p->imagename, filename, sp1 - filename);
    p->imagename[sp1 - filename] = '\0';
    if (ext) {
        strcat(p->imagename, ext);
        if (*sp2 == '.')
            sp2++;
    }
    strcat(p->imagename, sp2);

    p->filename_dsc.dsc$w_length = strlen(p->filename);
    p->filename_dsc.dsc$b_dtype = DSC$K_DTYPE_T;
    p->filename_dsc.dsc$b_class = DSC$K_CLASS_S;
    p->filename_dsc.dsc$a_pointer = p->filename;
    p->imagename_dsc.dsc$w_length = strlen(p->imagename);
    p->imagename_dsc.dsc$b_dtype = DSC$K_DTYPE_T;
    p->imagename_dsc.dsc$b_class = DSC$K_CLASS_S;
    p->imagename_dsc.dsc$a_pointer = p->imagename;

    if (!sk_void_push(dso->meth_data, (char *)p)) {
        ERR_raise(ERR_LIB_DSO, DSO_R_STACK_ERROR);
        goto err;
    }

    /* Success (for now, we lie.  We actually do not know...) */
    dso->loaded_filename = filename;
    return 1;
 err:
    /* Cleanup! */
    OPENSSL_free(p);
    OPENSSL_free(filename);
    return 0;
}

/*
 * Note that this doesn't actually unload the shared image, as there is no
 * such thing in VMS.  Next time it get loaded again, a new copy will
 * actually be loaded.
 */
static int vms_unload(DSO *dso)
{
    DSO_VMS_INTERNAL *p;
    if (dso == NULL) {
        ERR_raise(ERR_LIB_DSO, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if (sk_void_num(dso->meth_data) < 1)
        return 1;
    p = (DSO_VMS_INTERNAL *)sk_void_pop(dso->meth_data);
    if (p == NULL) {
        ERR_raise(ERR_LIB_DSO, DSO_R_NULL_HANDLE);
        return 0;
    }
    /* Cleanup */
    OPENSSL_free(p);
    return 1;
}

/*
 * We must do this in a separate function because of the way the exception
 * handler works (it makes this function return
 */
static int do_find_symbol(DSO_VMS_INTERNAL *ptr,
                          struct dsc$descriptor_s *symname_dsc, void **sym,
                          unsigned long flags)
{
    /*
     * Make sure that signals are caught and returned instead of aborting the
     * program.  The exception handler gets unestablished automatically on
     * return from this function.
     */
    lib$establish(lib$sig_to_ret);

    if (ptr->imagename_dsc.dsc$w_length)
        return lib$find_image_symbol(&ptr->filename_dsc,
                                     symname_dsc, sym,
                                     &ptr->imagename_dsc, flags);
    else
        return lib$find_image_symbol(&ptr->filename_dsc,
                                     symname_dsc, sym, 0, flags);
}

# ifndef LIB$M_FIS_MIXEDCASE
#  define LIB$M_FIS_MIXEDCASE (1 << 4);
# endif
void vms_bind_sym(DSO *dso, const char *symname, void **sym)
{
    DSO_VMS_INTERNAL *ptr;
    int status = 0;
    struct dsc$descriptor_s symname_dsc;

/* Arrange 32-bit pointer to (copied) string storage, if needed. */
# if __INITIAL_POINTER_SIZE == 64
#  define SYMNAME symname_32p
#  pragma pointer_size save
#  pragma pointer_size 32
    char *symname_32p;
#  pragma pointer_size restore
    char symname_32[NAMX_MAXRSS + 1];
# else                          /* __INITIAL_POINTER_SIZE == 64 */
#  define SYMNAME ((char *) symname)
# endif                         /* __INITIAL_POINTER_SIZE == 64 [else] */

    *sym = NULL;

    if ((dso == NULL) || (symname == NULL)) {
        ERR_raise(ERR_LIB_DSO, ERR_R_PASSED_NULL_PARAMETER);
        return;
    }
# if __INITIAL_POINTER_SIZE == 64
    /* Copy the symbol name to storage with a 32-bit pointer. */
    symname_32p = symname_32;
    strcpy(symname_32p, symname);
# endif                         /* __INITIAL_POINTER_SIZE == 64 [else] */

    symname_dsc.dsc$w_length = strlen(SYMNAME);
    symname_dsc.dsc$b_dtype = DSC$K_DTYPE_T;
    symname_dsc.dsc$b_class = DSC$K_CLASS_S;
    symname_dsc.dsc$a_pointer = SYMNAME;

    if (sk_void_num(dso->meth_data) < 1) {
        ERR_raise(ERR_LIB_DSO, DSO_R_STACK_ERROR);
        return;
    }
    ptr = (DSO_VMS_INTERNAL *)sk_void_value(dso->meth_data,
                                            sk_void_num(dso->meth_data) - 1);
    if (ptr == NULL) {
        ERR_raise(ERR_LIB_DSO, DSO_R_NULL_HANDLE);
        return;
    }

    status = do_find_symbol(ptr, &symname_dsc, sym, LIB$M_FIS_MIXEDCASE);

    if (!$VMS_STATUS_SUCCESS(status))
        status = do_find_symbol(ptr, &symname_dsc, sym, 0);

    if (!$VMS_STATUS_SUCCESS(status)) {
        unsigned short length;
        char errstring[257];
        struct dsc$descriptor_s errstring_dsc;

        errstring_dsc.dsc$w_length = sizeof(errstring);
        errstring_dsc.dsc$b_dtype = DSC$K_DTYPE_T;
        errstring_dsc.dsc$b_class = DSC$K_CLASS_S;
        errstring_dsc.dsc$a_pointer = errstring;

        *sym = NULL;

        status = sys$getmsg(status, &length, &errstring_dsc, 1, 0);

        if (!$VMS_STATUS_SUCCESS(status))
            lib$signal(status); /* This is really bad.  Abort! */
        else {
            errstring[length] = '\0';

            if (ptr->imagename_dsc.dsc$w_length)
                ERR_raise_data(ERR_LIB_DSO, DSO_R_SYM_FAILURE,
                               "Symbol %s in %s (%s): %s",
                               symname, ptr->filename, ptr->imagename,
                               errstring);
            else
                ERR_raise_data(ERR_LIB_DSO, DSO_R_SYM_FAILURE,
                               "Symbol %s in %s: %s",
                               symname, ptr->filename, errstring);
        }
        return;
    }
    return;
}

static DSO_FUNC_TYPE vms_bind_func(DSO *dso, const char *symname)
{
    DSO_FUNC_TYPE sym = 0;
    vms_bind_sym(dso, symname, (void **)&sym);
    return sym;
}

static char *vms_merger(DSO *dso, const char *filespec1,
                        const char *filespec2)
{
    int status;
    int filespec1len, filespec2len;
    struct FAB fab;
    struct NAMX_STRUCT nam;
    char esa[NAMX_MAXRSS + 1];
    char *merged;

/* Arrange 32-bit pointer to (copied) string storage, if needed. */
# if __INITIAL_POINTER_SIZE == 64
#  define FILESPEC1 filespec1_32p;
#  define FILESPEC2 filespec2_32p;
#  pragma pointer_size save
#  pragma pointer_size 32
    char *filespec1_32p;
    char *filespec2_32p;
#  pragma pointer_size restore
    char filespec1_32[NAMX_MAXRSS + 1];
    char filespec2_32[NAMX_MAXRSS + 1];
# else                          /* __INITIAL_POINTER_SIZE == 64 */
#  define FILESPEC1 ((char *) filespec1)
#  define FILESPEC2 ((char *) filespec2)
# endif                         /* __INITIAL_POINTER_SIZE == 64 [else] */

    if (!filespec1)
        filespec1 = "";
    if (!filespec2)
        filespec2 = "";
    filespec1len = strlen(filespec1);
    filespec2len = strlen(filespec2);

# if __INITIAL_POINTER_SIZE == 64
    /* Copy the file names to storage with a 32-bit pointer. */
    filespec1_32p = filespec1_32;
    filespec2_32p = filespec2_32;
    strcpy(filespec1_32p, filespec1);
    strcpy(filespec2_32p, filespec2);
# endif                         /* __INITIAL_POINTER_SIZE == 64 [else] */

    fab = cc$rms_fab;
    nam = CC_RMS_NAMX;

    FAB_OR_NAML(fab, nam).FAB_OR_NAML_FNA = FILESPEC1;
    FAB_OR_NAML(fab, nam).FAB_OR_NAML_FNS = filespec1len;
    FAB_OR_NAML(fab, nam).FAB_OR_NAML_DNA = FILESPEC2;
    FAB_OR_NAML(fab, nam).FAB_OR_NAML_DNS = filespec2len;
    NAMX_DNA_FNA_SET(fab)

        nam.NAMX_ESA = esa;
    nam.NAMX_ESS = NAMX_MAXRSS;
    nam.NAMX_NOP = NAM$M_SYNCHK | NAM$M_PWD;
    SET_NAMX_NO_SHORT_UPCASE(nam);

    fab.FAB_NAMX = &nam;

    status = sys$parse(&fab, 0, 0);

    if (!$VMS_STATUS_SUCCESS(status)) {
        unsigned short length;
        char errstring[257];
        struct dsc$descriptor_s errstring_dsc;

        errstring_dsc.dsc$w_length = sizeof(errstring);
        errstring_dsc.dsc$b_dtype = DSC$K_DTYPE_T;
        errstring_dsc.dsc$b_class = DSC$K_CLASS_S;
        errstring_dsc.dsc$a_pointer = errstring;

        status = sys$getmsg(status, &length, &errstring_dsc, 1, 0);

        if (!$VMS_STATUS_SUCCESS(status))
            lib$signal(status); /* This is really bad.  Abort! */
        else {
            errstring[length] = '\0';

            ERR_raise_data(ERR_LIB_DSO, DSO_R_FAILURE,
                           "filespec \"%s\", default \"%s\": %s",
                           filespec1, filespec2, errstring);
        }
        return NULL;
    }

    merged = OPENSSL_malloc(nam.NAMX_ESL + 1);
    if (merged == NULL)
        return NULL;
    strncpy(merged, nam.NAMX_ESA, nam.NAMX_ESL);
    merged[nam.NAMX_ESL] = '\0';
    return merged;
}

static char *vms_name_converter(DSO *dso, const char *filename)
{
    char *translated;
    int len, transform;
    const char *p;

    len = strlen(filename);

    p = strchr(filename, ':');
    if (p != NULL) {
        transform = 0;
    } else {
        p = filename;
        transform = (strrchr(p, '>') == NULL && strrchr(p, ']') == NULL);
    }

    if (transform) {
        int rsize = len + sizeof(DSO_EXTENSION);

        if ((translated = OPENSSL_malloc(rsize)) != NULL) {
            p = strrchr(filename, ';');
            if (p != NULL)
                len = p - filename;
            strncpy(translated, filename, len);
            translated[len] = '\0';
            strcat(translated, DSO_EXTENSION);
            if (p != NULL)
                strcat(translated, p);
        }
    } else {
        translated = OPENSSL_strdup(filename);
    }
    return translated;
}

#endif                          /* OPENSSL_SYS_VMS */
