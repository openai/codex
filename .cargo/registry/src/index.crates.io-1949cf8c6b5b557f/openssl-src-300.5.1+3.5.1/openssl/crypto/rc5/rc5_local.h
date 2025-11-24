/*
 * Copyright 1995-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdlib.h>

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

#if (defined(OPENSSL_SYS_WIN32) && defined(_MSC_VER))
# define ROTATE_l32(a,n)     _lrotl(a,n)
# define ROTATE_r32(a,n)     _lrotr(a,n)
#elif defined(__ICC)
# define ROTATE_l32(a,n)     _rotl(a,n)
# define ROTATE_r32(a,n)     _rotr(a,n)
#elif defined(__GNUC__) && __GNUC__>=2 && !defined(__STRICT_ANSI__) && !defined(OPENSSL_NO_ASM) && !defined(OPENSSL_NO_INLINE_ASM) && !defined(PEDANTIC)
# if defined(__i386) || defined(__i386__) || defined(__x86_64) || defined(__x86_64__)
#  define ROTATE_l32(a,n)       ({ register unsigned int ret;   \
                                        asm ("roll %%cl,%0"     \
                                                : "=r"(ret)     \
                                                : "c"(n),"0"((unsigned int)(a)) \
                                                : "cc");        \
                                        ret;                    \
                                })
#  define ROTATE_r32(a,n)       ({ register unsigned int ret;   \
                                        asm ("rorl %%cl,%0"     \
                                                : "=r"(ret)     \
                                                : "c"(n),"0"((unsigned int)(a)) \
                                                : "cc");        \
                                        ret;                    \
                                })
# endif
#endif
#ifndef ROTATE_l32
# define ROTATE_l32(a,n)     (((a)<<(n&0x1f))|(((a)&0xffffffff)>>((32-n)&0x1f)))
#endif
#ifndef ROTATE_r32
# define ROTATE_r32(a,n)     (((a)<<((32-n)&0x1f))|(((a)&0xffffffff)>>(n&0x1f)))
#endif

#define RC5_32_MASK     0xffffffffL

#define RC5_32_P        0xB7E15163L
#define RC5_32_Q        0x9E3779B9L

#define E_RC5_32(a,b,s,n) \
        a^=b; \
        a=ROTATE_l32(a,b); \
        a+=s[n]; \
        a&=RC5_32_MASK; \
        b^=a; \
        b=ROTATE_l32(b,a); \
        b+=s[n+1]; \
        b&=RC5_32_MASK;

#define D_RC5_32(a,b,s,n) \
        b-=s[n+1]; \
        b&=RC5_32_MASK; \
        b=ROTATE_r32(b,a); \
        b^=a; \
        a-=s[n]; \
        a&=RC5_32_MASK; \
        a=ROTATE_r32(a,b); \
        a^=b;
