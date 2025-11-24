/*
 * Copyright 1995-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#undef c2l
#define c2l(c,l)        (l =((unsigned long)(*((c)++)))    , \
                         l|=((unsigned long)(*((c)++)))<< 8L, \
                         l|=((unsigned long)(*((c)++)))<<16L, \
                         l|=((unsigned long)(*((c)++)))<<24L)

/* NOTE - c is not incremented as per c2l */
#undef c2ln
#define c2ln(c,l1,l2,n) { \
                        c+=n; \
                        l1=l2=0; \
                        switch (n) { \
                        case 8: l2 =((unsigned long)(*(--(c))))<<24L; \
                        /* fall through */                               \
                        case 7: l2|=((unsigned long)(*(--(c))))<<16L; \
                        /* fall through */                               \
                        case 6: l2|=((unsigned long)(*(--(c))))<< 8L; \
                        /* fall through */                               \
                        case 5: l2|=((unsigned long)(*(--(c))));      \
                        /* fall through */                               \
                        case 4: l1 =((unsigned long)(*(--(c))))<<24L; \
                        /* fall through */                               \
                        case 3: l1|=((unsigned long)(*(--(c))))<<16L; \
                        /* fall through */                               \
                        case 2: l1|=((unsigned long)(*(--(c))))<< 8L; \
                        /* fall through */                               \
                        case 1: l1|=((unsigned long)(*(--(c))));      \
                                } \
                        }

#undef l2c
#define l2c(l,c)        (*((c)++)=(unsigned char)(((l)     )&0xff), \
                         *((c)++)=(unsigned char)(((l)>> 8L)&0xff), \
                         *((c)++)=(unsigned char)(((l)>>16L)&0xff), \
                         *((c)++)=(unsigned char)(((l)>>24L)&0xff))

/* NOTE - c is not incremented as per l2c */
#undef l2cn
#define l2cn(l1,l2,c,n) { \
                        c+=n; \
                        switch (n) { \
                        case 8: *(--(c))=(unsigned char)(((l2)>>24L)&0xff); \
                        /* fall through */                                     \
                        case 7: *(--(c))=(unsigned char)(((l2)>>16L)&0xff); \
                        /* fall through */                                     \
                        case 6: *(--(c))=(unsigned char)(((l2)>> 8L)&0xff); \
                        /* fall through */                                     \
                        case 5: *(--(c))=(unsigned char)(((l2)     )&0xff); \
                        /* fall through */                                     \
                        case 4: *(--(c))=(unsigned char)(((l1)>>24L)&0xff); \
                        /* fall through */                                     \
                        case 3: *(--(c))=(unsigned char)(((l1)>>16L)&0xff); \
                        /* fall through */                                     \
                        case 2: *(--(c))=(unsigned char)(((l1)>> 8L)&0xff); \
                        /* fall through */                                     \
                        case 1: *(--(c))=(unsigned char)(((l1)     )&0xff); \
                                } \
                        }

