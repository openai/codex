#!/usr/bin/env perl
#
# ====================================================================
# Written by Andy Polyakov, @dot-asm, initially for use with OpenSSL.
# ====================================================================
#
# ChaCha20 for Itanium.
#
# March 2019
#
# Itanium 9xxx, which has pair of shifters, manages to process one byte
# in 9.3 cycles. This aligns perfectly with theoretical estimate.
# On the other hand, pre-9000 CPU has single shifter and each extr/dep
# pairs below takes additional cycle. Then final input->xor->output
# pass runs slower than expected... Overall result is 15.6 cpb, two
# cycles more than theoretical estimate.

$output = pop and open STDOUT, ">$output";

my @k = map("r$_",(16..31));
my @x = map("r$_",(38..53));
my @y = map("r$_",(8..11));
my @z = map("r$_",(15,35..37));
my ($out,$inp,$len,$key,$counter) = map("r$_",(32..36));

$code.=<<___;
#if defined(_HPUX_SOURCE)
# if !defined(_LP64)
#  define ADDP  addp4
# else
#  define ADDP  add
# endif
#else
# define ADDP   add
#endif

.text

.global	ChaCha20_ctr32#
.proc	ChaCha20_ctr32#
.align	32
ChaCha20_ctr32:
	.prologue
	.save		ar.pfs,r2
{ .mmi;	alloc		r2=ar.pfs,5,17,0,0
	ADDP		@k[11]=4,$key
	.save		ar.lc,r3
	mov		r3=ar.lc		}
{ .mmi;	ADDP		$out=0,$out
	ADDP		$inp=0,$inp		}
{ .mmi;	ADDP		$key=0,$key
	ADDP		$counter=0,$counter
	.save		pr,r14
	mov		r14=pr			};;

	.body
{ .mlx;	ld4		@k[4]=[$key],8
	movl		@k[0]=0x61707865	}
{ .mlx;	ld4		@k[5]=[@k[11]],8
	movl		@k[1]=0x3320646e	};;
{ .mlx;	ld4		@k[6]=[$key],8
	movl		@k[2]=0x79622d32	}
{ .mlx;	ld4		@k[7]=[@k[11]],8
	movl		@k[3]=0x6b206574	};;
{ .mmi;	ld4		@k[8]=[$key],8
	ld4		@k[9]=[@k[11]],8
	add		@k[15]=4,$counter	};;
{ .mmi;	ld4		@k[10]=[$key]
	ld4		@k[11]=[@k[11]]
	mov		@x[0]=@k[0]		};;
{ .mmi;	ld4		@k[12]=[$counter],8
	ld4		@k[13]=[@k[15]],8
	mov		@x[1]=@k[1]		};;
{ .mmi;	ld4		@k[14]=[$counter]
	ld4		@k[15]=[@k[15]]
	mov		@x[2]=@k[2]		}
{ .mmi;	mov		@x[3]=@k[3]
	mov		@x[4]=@k[4]
	mov		@x[5]=@k[5]		};;
{ .mmi;	mov		@x[6]=@k[6]
	mov		@x[7]=@k[7]
	mov		@x[8]=@k[8]		}
{ .mmi;	mov		@x[9]=@k[9]
	mov		@x[10]=@k[10]
	mov		@x[11]=@k[11]		}
{ .mmi;	mov		@x[12]=@k[12]
	mov		@x[13]=@k[13]
	mov		@x[14]=@k[14]		};;

.Loop_outer:
{ .mii;	mov		@x[15]=@k[15]
	mov		ar.lc=9
	mov		ar.ec=1			}
{ .mmb;	cmp.geu		p6,p0=64,$len
	sub		@z[1]=64,$len
	brp.loop.imp	.Loop_top,.Loop_end-16	};;

.Loop_top:
___
sub ROUND {
my ($a0,$b0,$c0,$d0)=@_;
my ($a1,$b1,$c1,$d1)=map(($_&~3)+(($_+1)&3),($a0,$b0,$c0,$d0));
my ($a2,$b2,$c2,$d2)=map(($_&~3)+(($_+1)&3),($a1,$b1,$c1,$d1));
my ($a3,$b3,$c3,$d3)=map(($_&~3)+(($_+1)&3),($a2,$b2,$c2,$d2));

$code.=<<___;
{ .mmi;	add		@x[$a0]=@x[$a0],@x[$b0]
	add		@x[$a1]=@x[$a1],@x[$b1]
	add		@x[$a2]=@x[$a2],@x[$b2]		};;
{ .mmi;	add		@x[$a3]=@x[$a3],@x[$b3]
	xor		@x[$d0]=@x[$d0],@x[$a0]
	xor		@x[$d1]=@x[$d1],@x[$a1]		};;
{ .mmi;	xor		@x[$d2]=@x[$d2],@x[$a2]
	xor		@x[$d3]=@x[$d3],@x[$a3]
	extr.u		@y[0]=@x[$d0],16,16		};;
{ .mii;	extr.u		@y[1]=@x[$d1],16,16
	dep		@x[$d0]=@x[$d0],@y[0],16,16	};;
{ .mii;	 add		@x[$c0]=@x[$c0],@x[$d0]
	extr.u		@y[2]=@x[$d2],16,16
	dep		@x[$d1]=@x[$d1],@y[1],16,16	};;
{ .mii;	 add		@x[$c1]=@x[$c1],@x[$d1]
	 xor		@x[$b0]=@x[$b0],@x[$c0]
	extr.u		@y[3]=@x[$d3],16,16		};;
{ .mii;	 xor		@x[$b1]=@x[$b1],@x[$c1]
	dep		@x[$d2]=@x[$d2],@y[2],16,16
	dep		@x[$d3]=@x[$d3],@y[3],16,16	};;
{ .mmi;	 add		@x[$c2]=@x[$c2],@x[$d2]
	 add		@x[$c3]=@x[$c3],@x[$d3]
	 extr.u		@y[0]=@x[$b0],20,12		};;
{ .mmi;	 xor		@x[$b2]=@x[$b2],@x[$c2]
	 xor		@x[$b3]=@x[$b3],@x[$c3]
	 dep.z		@x[$b0]=@x[$b0],12,20		};;
{ .mii;	 or		@x[$b0]=@x[$b0],@y[0]
	 extr.u		@y[1]=@x[$b1],20,12
	 dep.z		@x[$b1]=@x[$b1],12,20		};;
{ .mii;	add		@x[$a0]=@x[$a0],@x[$b0]
	 extr.u		@y[2]=@x[$b2],20,12
	 extr.u		@y[3]=@x[$b3],20,12		}
{ .mii;	 or		@x[$b1]=@x[$b1],@y[1]
	 dep.z		@x[$b2]=@x[$b2],12,20
	 dep.z		@x[$b3]=@x[$b3],12,20		};;
{ .mmi;	 or		@x[$b2]=@x[$b2],@y[2]
	 or		@x[$b3]=@x[$b3],@y[3]
	add		@x[$a1]=@x[$a1],@x[$b1]		};;
{ .mmi;	add		@x[$a2]=@x[$a2],@x[$b2]
	add		@x[$a3]=@x[$a3],@x[$b3]
	xor		@x[$d0]=@x[$d0],@x[$a0]		};;
{ .mii;	xor		@x[$d1]=@x[$d1],@x[$a1]
	extr.u		@y[0]=@x[$d0],24,8
	dep.z		@x[$d0]=@x[$d0],8,24		};;
{ .mii;	or		@x[$d0]=@x[$d0],@y[0]
	extr.u		@y[1]=@x[$d1],24,8
	dep.z		@x[$d1]=@x[$d1],8,24		};;
{ .mmi;	or		@x[$d1]=@x[$d1],@y[1]
	xor		@x[$d2]=@x[$d2],@x[$a2]
	xor		@x[$d3]=@x[$d3],@x[$a3]		};;
{ .mii;	 add		@x[$c0]=@x[$c0],@x[$d0]
	extr.u		@y[2]=@x[$d2],24,8
	dep.z		@x[$d2]=@x[$d2],8,24		};;
{ .mii;	 xor		@x[$b0]=@x[$b0],@x[$c0]
	extr.u		@y[3]=@x[$d3],24,8
	dep.z		@x[$d3]=@x[$d3],8,24		};;
{ .mmi;	or		@x[$d2]=@x[$d2],@y[2]
	or		@x[$d3]=@x[$d3],@y[3]
	 extr.u		@y[0]=@x[$b0],25,7		};;
{ .mmi;	 add		@x[$c1]=@x[$c1],@x[$d1]
	 add		@x[$c2]=@x[$c2],@x[$d2]
	 dep.z		@x[$b0]=@x[$b0],7,25		};;
{ .mmi;	 xor		@x[$b1]=@x[$b1],@x[$c1]
	 xor		@x[$b2]=@x[$b2],@x[$c2]
	 add		@x[$c3]=@x[$c3],@x[$d3]		};;
{ .mii;	 xor		@x[$b3]=@x[$b3],@x[$c3]
	 extr.u		@y[1]=@x[$b1],25,7
	 dep.z		@x[$b1]=@x[$b1],7,25		};;
{ .mii;	 or		@x[$b0]=@x[$b0],@y[0]
	 extr.u		@y[2]=@x[$b2],25,7
	 dep.z		@x[$b2]=@x[$b2],7,25		};;
{ .mii;	 or		@x[$b1]=@x[$b1],@y[1]
	 extr.u		@y[3]=@x[$b3],25,7
	 dep.z		@x[$b3]=@x[$b3],7,25		};;
___
$code.=<<___		if ($d0 == 12);
{ .mmi;	 or		@x[$b2]=@x[$b2],@y[2]
	 or		@x[$b3]=@x[$b3],@y[3]
	mov		@z[0]=-1			};;
___
$code.=<<___		if ($d0 == 15);
{ .mmb;	 or		@x[$b2]=@x[$b2],@y[2]
	 or		@x[$b3]=@x[$b3],@y[3]
	br.ctop.sptk	.Loop_top			};;
___
}
	&ROUND(0, 4, 8, 12);
	&ROUND(0, 5, 10, 15);
$code.=<<___;
.Loop_end:

{ .mmi;	add		@x[0]=@x[0],@k[0]
	add		@x[1]=@x[1],@k[1]
(p6)	shr.u		@z[0]=@z[0],@z[1]		}
{ .mmb;	add		@x[2]=@x[2],@k[2]
	add		@x[3]=@x[3],@k[3]
	clrrrb.pr					};;
{ .mmi;	add		@x[4]=@x[4],@k[4]
	add		@x[5]=@x[5],@k[5]
	add		@x[6]=@x[6],@k[6]		}
{ .mmi;	add		@x[7]=@x[7],@k[7]
	add		@x[8]=@x[8],@k[8]
	add		@x[9]=@x[9],@k[9]		}
{ .mmi;	add		@x[10]=@x[10],@k[10]
	add		@x[11]=@x[11],@k[11]
	add		@x[12]=@x[12],@k[12]		}
{ .mmi;	add		@x[13]=@x[13],@k[13]
	add		@x[14]=@x[14],@k[14]
	add		@x[15]=@x[15],@k[15]		}
{ .mmi;	add		@k[12]=1,@k[12]			// next counter
	mov		pr=@z[0],0x1ffff		};;

//////////////////////////////////////////////////////////////////
// Each predicate bit corresponds to byte to be processed. Note
// that p0 is wired to 1, but it works out, because there always
// is at least one byte to process...
{ .mmi;	(p0)	ld1		@z[0]=[$inp],1
		shr.u		@y[1]=@x[0],8		};;
{ .mmi;	(p1)	ld1		@z[1]=[$inp],1
	(p2)	shr.u		@y[2]=@x[0],16		};;
{ .mmi;	(p2)	ld1		@z[2]=[$inp],1
	(p0)	xor		@z[0]=@z[0],@x[0]
	(p3)	shr.u		@y[3]=@x[0],24		};;
___
for(my $i0=0; $i0<60; $i0+=4) {
my ($i1, $i2, $i3, $i4, $i5, $i6, $i7) = map($i0+$_,(1..7));
my $k = $i0/4+1;

$code.=<<___;
{ .mmi;	(p$i3)	ld1		@z[3]=[$inp],1
	(p$i0)	st1		[$out]=@z[0],1
	(p$i1)	xor		@z[1]=@z[1],@y[1]	};;
{ .mmi;	(p$i4)	ld1		@z[0]=[$inp],1
	(p$i5)	shr.u		@y[1]=@x[$k],8		}
{ .mmi;	(p$i1)	st1		[$out]=@z[1],1
	(p$i2)	xor		@z[2]=@z[2],@y[2]
	(p1)	mov		@x[$k-1]=@k[$k-1]	};;
{ .mfi;	(p$i5)	ld1		@z[1]=[$inp],1
	(p$i6)	shr.u		@y[2]=@x[$k],16		}
{ .mfi;	(p$i2)	st1		[$out]=@z[2],1
	(p$i3)	xor		@z[3]=@z[3],@y[3]	};;
{ .mfi;	(p$i6)	ld1		@z[2]=[$inp],1
	(p$i7)	shr.u		@y[3]=@x[$k],24		}
___
$code.=<<___	if ($i0==0);	# p1,p2 are available for reuse in first round
{ .mmi;	(p$i3)	st1		[$out]=@z[3],1
	(p$i4)	xor		@z[0]=@z[0],@x[$k]
		cmp.ltu		p1,p2=64,$len		};;
___
$code.=<<___	if ($i0>0);
{ .mfi;	(p$i3)	st1		[$out]=@z[3],1
	(p$i4)	xor		@z[0]=@z[0],@x[$k]	};;
___
}
$code.=<<___;
{ .mmi;	(p63)	ld1		@z[3]=[$inp],1
	(p60)	st1		[$out]=@z[0],1
	(p61)	xor		@z[1]=@z[1],@y[1]	};;
{ .mmi;	(p61)	st1		[$out]=@z[1],1
	(p62)	xor		@z[2]=@z[2],@y[2]	};;
{ .mmi;	(p62)	st1		[$out]=@z[2],1
	(p63)	xor		@z[3]=@z[3],@y[3]
	(p2)	mov		ar.lc=r3		};;
{ .mib;	(p63)	st1		[$out]=@z[3],1
	(p1)	add		$len=-64,$len
(p1)	br.dptk.many		.Loop_outer		};;

{ .mmi;	mov			@k[4]=0			// wipe key material
	mov			@k[5]=0
	mov			@k[6]=0			}
{ .mmi;	mov			@k[7]=0
	mov			@k[8]=0
	mov			@k[9]=0			}
{ .mmi;	mov			@k[10]=0
	mov			@k[11]=0
	mov			@k[12]=0		}
{ .mmi;	mov			@k[13]=0
	mov			@k[14]=0
	mov			@k[15]=0		}
{ .mib;	mov			pr=r14,0x1ffff
	br.ret.sptk.many	b0			};;
.endp	ChaCha20_ctr32#
stringz "ChaCha20 for IA64, CRYPTOGAMS by \@dot-asm"
___

print $code;
close STDOUT or die "error closing STDOUT: $!";
