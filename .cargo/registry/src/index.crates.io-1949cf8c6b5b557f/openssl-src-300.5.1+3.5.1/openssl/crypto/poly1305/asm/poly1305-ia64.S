// ====================================================================
// Written by Andy Polyakov, @dot-asm, initially for use in the OpenSSL
// project.
// ====================================================================
//
// Poly1305 for Itanium.
//
// January 2019
//
// Performance was reported to be ~2.1 cycles per byte on Itanium 2.
// With exception for processors in 95xx family, which have higher
// floating-point instructions' latencies and deliver ~2.6 cpb.
// Comparison to compiler-generated code is not exactly fair, because
// of different radixes. But just for reference, it was observed to be
// >3x faster. Originally it was argued that floating-point base 2^32
// implementation would be optimal. Upon closer look estimate for below
// integer base 2^64 implementation turned to be approximately same on
// Itanium 2. But floating-point code would be larger, and have higher
// overhead, which would negatively affect small-block performance...

#if defined(_HPUX_SOURCE)
# if !defined(_LP64)
#  define ADDP  addp4
# else
#  define ADDP  add
# endif
# define RUM    rum
# define SUM    sum
#else
# define ADDP   add
# define RUM    nop
# define SUM    nop
#endif

.text
.explicit

.global	poly1305_init#
.proc	poly1305_init#
.align	64
poly1305_init:
	.prologue
	.save		ar.pfs,r2
{ .mmi;	alloc		r2=ar.pfs,2,0,0,0
	cmp.eq		p6,p7=0,r33		}	// key == NULL?
{ .mmi;	ADDP		r9=8,r32
	ADDP		r10=16,r32
	ADDP		r32=0,r32		};;
	.body
{ .mmi;	st8		[r32]=r0,24			// ctx->h0 = 0
	st8		[r9]=r0				// ctx->h1 = 0
(p7)	ADDP		r8=0,r33		}
{ .mib;	st8		[r10]=r0			// ctx->h2 = 0
(p6)	mov		r8=0
(p6)	br.ret.spnt	b0			};;

{ .mmi;	ADDP		r9=1,r33
	ADDP		r10=2,r33
	ADDP		r11=3,r33		};;
{ .mmi;	ld1		r16=[r8],4			// load key, little-endian
	ld1		r17=[r9],4		}
{ .mmi;	ld1		r18=[r10],4
	ld1		r19=[r11],4		};;
{ .mmi;	ld1		r20=[r8],4
	ld1		r21=[r9],4		}
{ .mmi;	ld1		r22=[r10],4
	ld1		r23=[r11],4
	and		r19=15,r19		};;
{ .mmi;	ld1		r24=[r8],4
	ld1		r25=[r9],4
	and		r20=-4,r20		}
{ .mmi;	ld1		r26=[r10],4
	ld1		r27=[r11],4
	and		r23=15,r23		};;
{ .mmi;	ld1		r28=[r8],4
	ld1		r29=[r9],4
	and		r24=-4,r24		}
{ .mmi;	ld1		r30=[r10],4
	ld1		r31=[r11],4
	and		r27=15,r27		};;

{ .mii;	and		r28=-4,r28
	dep		r16=r17,r16,8,8
	dep		r18=r19,r18,8,8		};;
{ .mii;	and		r31=15,r31
	dep		r16=r18,r16,16,16
	dep		r20=r21,r20,8,8		};;
{ .mii;	dep		r16=r20,r16,32,16
	dep		r22=r23,r22,8,8		};;
{ .mii;	dep		r16=r22,r16,48,16
	dep		r24=r25,r24,8,8		};;
{ .mii;	dep		r26=r27,r26,8,8
	dep		r28=r29,r28,8,8		};;
{ .mii;	dep		r24=r26,r24,16,16
	dep		r30=r31,r30,8,8		};;
{ .mii;	st8		[r32]=r16,8			// ctx->r0
	dep		r24=r28,r24,32,16;;
	dep		r24=r30,r24,48,16	};;
{ .mii;	st8		[r32]=r24,8			// ctx->r1
	shr.u		r25=r24,2;;
	add		r25=r25,r24		};;
{ .mib; st8		[r32]=r25			// ctx->s1
	mov		r8=0
	br.ret.sptk	b0			};;
.endp	poly1305_init#

h0=r17;  h1=r18;  h2=r19;
i0=r20;  i1=r21;
HF0=f8;  HF1=f9;  HF2=f10;
RF0=f11; RF1=f12; SF1=f13;

.global	poly1305_blocks#
.proc	poly1305_blocks#
.align	64
poly1305_blocks:
	.prologue
	.save		ar.pfs,r2
{ .mii;	alloc		r2=ar.pfs,4,1,0,0
	.save		ar.lc,r3
	mov		r3=ar.lc
	.save		pr,r36
	mov		r36=pr			}

	.body
{ .mmi;	ADDP		r8=0,r32
	ADDP		r9=8,r32
	and		r29=7,r33		};;
{ .mmi;	ld8		h0=[r8],16
	ld8		h1=[r9],16
	and		r33=-8,r33		};;
{ .mmi;	ld8		h2=[r8],16
	ldf8		RF0=[r9],16
	shr.u		r34=r34,4		};;
{ .mmi;	ldf8		RF1=[r8],-32
	ldf8		SF1=[r9],-32
	cmp.ltu		p16,p17=1,r34		};;
{ .mmi;
(p16)	add		r34=-2,r34
(p17)	mov		r34=0
	ADDP		r10=0,r33		}
{ .mii;	ADDP		r11=8,r33
(p16)	mov		ar.ec=2
(p17)	mov		ar.ec=1			};;
{ .mib;	RUM		1<<1				// go little-endian
	mov		ar.lc=r34
	brp.loop.imp	.Loop,.Lcend-16		}

{ .mmi;	cmp.eq		p8,p7=0,r29
	cmp.eq		p9,p0=1,r29
	cmp.eq		p10,p0=2,r29		}
{ .mmi;	cmp.eq		p11,p0=3,r29
	cmp.eq		p12,p0=4,r29
	cmp.eq		p13,p0=5,r29		}
{ .mmi;	cmp.eq		p14,p0=6,r29
	cmp.eq		p15,p0=7,r29
	add		r16=16,r10		};;

{ .mmb;
(p8)	ld8		i0=[r10],16			// aligned input
(p8)	ld8		i1=[r11],16
(p8)	br.cond.sptk	.Loop			};;

	// align first block
	.pred.rel	"mutex",p8,p9,p10,p11,p12,p13,p14,p15
{ .mmi;	(p7)	ld8		r14=[r10],24
	(p7)	ld8		r15=[r11],24		}

{ .mii;	(p7)	ld8		r16=[r16]
		nop.i		0;;
	(p15)	shrp		i0=r15,r14,56		}
{ .mii;	(p15)	shrp		i1=r16,r15,56
	(p14)	shrp		i0=r15,r14,48		}
{ .mii;	(p14)	shrp		i1=r16,r15,48
	(p13)	shrp		i0=r15,r14,40		}
{ .mii;	(p13)	shrp		i1=r16,r15,40
	(p12)	shrp		i0=r15,r14,32		}
{ .mii;	(p12)	shrp		i1=r16,r15,32
	(p11)	shrp		i0=r15,r14,24		}
{ .mii;	(p11)	shrp		i1=r16,r15,24
	(p10)	shrp		i0=r15,r14,16		}
{ .mii;	(p10)	shrp		i1=r16,r15,16
	(p9)	shrp		i0=r15,r14,8		}
{ .mii;	(p9)	shrp		i1=r16,r15,8
		mov		r14=r16			};;

.Loop:
		.pred.rel	"mutex",p8,p9,p10,p11,p12,p13,p14,p15
{ .mmi;		add		h0=h0,i0
		add		h1=h1,i1
		add		h2=h2,r35		};;
{ .mmi;		setf.sig	HF0=h0
		cmp.ltu		p6,p0=h0,i0
		cmp.ltu		p7,p0=h1,i1		};;
{ .mmi;	(p6)	add		h1=1,h1;;
		setf.sig	HF1=h1
	(p6)	cmp.eq.or	p7,p0=0,h1		};;
{ .mmi;	(p7)	add		h2=1,h2;;
		setf.sig	HF2=h2			};;

{ .mfi;	(p16)	ld8		r15=[r10],16
		xmpy.lu		f32=HF0,RF0		}
{ .mfi;	(p16)	ld8		r16=[r11],16
		xmpy.hu		f33=HF0,RF0		}
{ .mfi;		xmpy.lu		f36=HF0,RF1		}
{ .mfi;		xmpy.hu		f37=HF0,RF1		};;
{ .mfi;		xmpy.lu		f34=HF1,SF1
	(p15)	shrp		i0=r15,r14,56		}
{ .mfi;		xmpy.hu		f35=HF1,SF1		}
{ .mfi;		xmpy.lu		f38=HF1,RF0
	(p15)	shrp		i1=r16,r15,56		}
{ .mfi;		xmpy.hu		f39=HF1,RF0		}
{ .mfi;		xmpy.lu		f40=HF2,SF1
	(p14)	shrp		i0=r15,r14,48		}
{ .mfi;		xmpy.lu		f41=HF2,RF0		};;

{ .mmi;		getf.sig	r22=f32
		getf.sig	r23=f33
	(p14)	shrp		i1=r16,r15,48		}
{ .mmi;		getf.sig	r24=f34
		getf.sig	r25=f35
	(p13)	shrp		i0=r15,r14,40		}
{ .mmi;		getf.sig	r26=f36
		getf.sig	r27=f37
	(p13)	shrp		i1=r16,r15,40		}
{ .mmi;		getf.sig	r28=f38
		getf.sig	r29=f39
	(p12)	shrp		i0=r15,r14,32		}
{ .mmi;		getf.sig	r30=f40
		getf.sig	r31=f41			};;

{ .mmi;		add		h0=r22,r24
		add		r23=r23,r25
	(p12)	shrp		i1=r16,r15,32		}
{ .mmi;		add		h1=r26,r28
		add		r27=r27,r29
	(p11)	shrp		i0=r15,r14,24		};;
{ .mmi;		cmp.ltu		p6,p0=h0,r24
		cmp.ltu		p7,p0=h1,r28
		add		r23=r23,r30		};;
{ .mmi;	(p6)	add		r23=1,r23
	(p7)	add		r27=1,r27
	(p11)	shrp		i1=r16,r15,24		};;
{ .mmi;		add		h1=h1,r23;;
		cmp.ltu		p6,p7=h1,r23
	(p10)	shrp		i0=r15,r14,16		};;
{ .mmi;	(p6)	add		h2=r31,r27,1
	(p7)	add		h2=r31,r27
	(p10)	shrp		i1=r16,r15,16		};;

{ .mmi;	(p8)	mov		i0=r15
		and		r22=-4,h2
		shr.u		r23=h2,2		};;
{ .mmi;		add		r22=r22,r23
		and		h2=3,h2
	(p9)	shrp		i0=r15,r14,8		};;

{ .mmi;		add		h0=h0,r22;;
		cmp.ltu		p6,p0=h0,r22
	(p9)	shrp		i1=r16,r15,8		};;
{ .mmi;	(p8)	mov		i1=r16
	(p6)	cmp.eq.unc	p7,p0=-1,h1
	(p6)	add		h1=1,h1			};;
{ .mmb;	(p7)	add		h2=1,h2
		mov		r14=r16
		br.ctop.sptk	.Loop			};;
.Lcend:

{ .mii;	SUM		1<<1				// back to big-endian
	mov		ar.lc=r3		};;

{ .mmi;	st8		[r8]=h0,16
	st8		[r9]=h1
	mov		pr=r36,0x1ffff		};;
{ .mmb;	st8		[r8]=h2
	rum		1<<5
	br.ret.sptk	b0			};;
.endp	poly1305_blocks#

.global	poly1305_emit#
.proc	poly1305_emit#
.align	64
poly1305_emit:
	.prologue
	.save		ar.pfs,r2
{ .mmi;	alloc		r2=ar.pfs,3,0,0,0
	ADDP		r8=0,r32
	ADDP		r9=8,r32		};;

	.body
{ .mmi;	ld8		r16=[r8],16			// load hash
	ld8		r17=[r9]
	ADDP		r10=0,r34		};;
{ .mmi;	ld8		r18=[r8]
	ld4		r24=[r10],8			// load nonce
	ADDP		r11=4,r34		};;

{ .mmi;	ld4		r25=[r11],8
	ld4		r26=[r10]
	add		r20=5,r16		};;

{ .mmi;	ld4		r27=[r11]
	cmp.ltu		p6,p7=r20,r16
	shl		r25=r25,32		};;
{ .mmi;
(p6)	add		r21=1,r17
(p7)	add		r21=0,r17
(p6)	cmp.eq.or.andcm	p6,p7=-1,r17		};;
{ .mmi;
(p6)	add		r22=1,r18
(p7)	add		r22=0,r18
	shl		r27=r27,32		};;
{ .mmi;	or		r24=r24,r25
	or		r26=r26,r27
	cmp.leu		p6,p7=4,r22		};;
{ .mmi;
(p6)	add		r16=r20,r24
(p7)	add		r16=r16,r24
(p6)	add		r17=r21,r26		};;
{ .mii;
(p7)	add		r17=r17,r26
	cmp.ltu		p6,p7=r16,r24;;
(p6)	add		r17=1,r17		};;

{ .mmi;	ADDP		r8=0,r33
	ADDP		r9=4,r33
	shr.u		r20=r16,32		}
{ .mmi;	ADDP		r10=8,r33
	ADDP		r11=12,r33
	shr.u		r21=r17,32		};;

{ .mmi;	st1		[r8]=r16,1			// write mac, little-endian
	st1		[r9]=r20,1
	shr.u		r16=r16,8		}
{ .mii;	st1		[r10]=r17,1
	shr.u		r20=r20,8
	shr.u		r17=r17,8		}
{ .mmi;	st1		[r11]=r21,1
	shr.u		r21=r21,8		};;

{ .mmi;	st1		[r8]=r16,1
	st1		[r9]=r20,1
	shr.u		r16=r16,8		}
{ .mii;	st1		[r10]=r17,1
	shr.u		r20=r20,8
	shr.u		r17=r17,8		}
{ .mmi;	st1		[r11]=r21,1
	shr.u		r21=r21,8		};;

{ .mmi;	st1		[r8]=r16,1
	st1		[r9]=r20,1
	shr.u		r16=r16,8		}
{ .mii;	st1		[r10]=r17,1
	shr.u		r20=r20,8
	shr.u		r17=r17,8		}
{ .mmi;	st1		[r11]=r21,1
	shr.u		r21=r21,8		};;

{ .mmi;	st1		[r8]=r16
	st1		[r9]=r20		}
{ .mmb;	st1		[r10]=r17
	st1		[r11]=r21
	br.ret.sptk	b0			};;
.endp	poly1305_emit#

stringz	"Poly1305 for IA64, CRYPTOGAMS by \@dot-asm"
