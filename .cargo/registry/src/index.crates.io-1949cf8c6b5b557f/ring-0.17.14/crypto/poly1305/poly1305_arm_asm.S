#include <ring-core/asm_base.h>

#if !defined(OPENSSL_NO_ASM) && defined(OPENSSL_ARM) && defined(__ELF__)

#pragma GCC diagnostic ignored "-Wlanguage-extension-token"

# This implementation was taken from the public domain, neon2 version in
# SUPERCOP by D. J. Bernstein and Peter Schwabe.

# qhasm: int32 input_0

# qhasm: int32 input_1

# qhasm: int32 input_2

# qhasm: int32 input_3

# qhasm: stack32 input_4

# qhasm: stack32 input_5

# qhasm: stack32 input_6

# qhasm: stack32 input_7

# qhasm: int32 caller_r4

# qhasm: int32 caller_r5

# qhasm: int32 caller_r6

# qhasm: int32 caller_r7

# qhasm: int32 caller_r8

# qhasm: int32 caller_r9

# qhasm: int32 caller_r10

# qhasm: int32 caller_r11

# qhasm: int32 caller_r12

# qhasm: int32 caller_r14

# qhasm: reg128 caller_q4

# qhasm: reg128 caller_q5

# qhasm: reg128 caller_q6

# qhasm: reg128 caller_q7

# qhasm: startcode
.fpu neon
.text

# qhasm: reg128 r0

# qhasm: reg128 r1

# qhasm: reg128 r2

# qhasm: reg128 r3

# qhasm: reg128 r4

# qhasm: reg128 x01

# qhasm: reg128 x23

# qhasm: reg128 x4

# qhasm: reg128 y0

# qhasm: reg128 y12

# qhasm: reg128 y34

# qhasm: reg128 5y12

# qhasm: reg128 5y34

# qhasm: stack128 y0_stack

# qhasm: stack128 y12_stack

# qhasm: stack128 y34_stack

# qhasm: stack128 5y12_stack

# qhasm: stack128 5y34_stack

# qhasm: reg128 z0

# qhasm: reg128 z12

# qhasm: reg128 z34

# qhasm: reg128 5z12

# qhasm: reg128 5z34

# qhasm: stack128 z0_stack

# qhasm: stack128 z12_stack

# qhasm: stack128 z34_stack

# qhasm: stack128 5z12_stack

# qhasm: stack128 5z34_stack

# qhasm: stack128 two24

# qhasm: int32 ptr

# qhasm: reg128 c01

# qhasm: reg128 c23

# qhasm: reg128 d01

# qhasm: reg128 d23

# qhasm: reg128 t0

# qhasm: reg128 t1

# qhasm: reg128 t2

# qhasm: reg128 t3

# qhasm: reg128 t4

# qhasm: reg128 mask

# qhasm: reg128 u0

# qhasm: reg128 u1

# qhasm: reg128 u2

# qhasm: reg128 u3

# qhasm: reg128 u4

# qhasm: reg128 v01

# qhasm: reg128 mid

# qhasm: reg128 v23

# qhasm: reg128 v4

# qhasm: int32 len

# qhasm: qpushenter crypto_onetimeauth_poly1305_neon2_blocks
.align 4
.global openssl_poly1305_neon2_blocks
.hidden openssl_poly1305_neon2_blocks
.type openssl_poly1305_neon2_blocks STT_FUNC
openssl_poly1305_neon2_blocks:
vpush {q4,q5,q6,q7}
mov r12,sp
sub sp,sp,#192
bic sp,sp,#31

# qhasm: len = input_3
# asm 1: mov >len=int32#4,<input_3=int32#4
# asm 2: mov >len=r3,<input_3=r3
mov r3,r3

# qhasm: new y0

# qhasm: y0  = mem64[input_1]y0[1]; input_1 += 8
# asm 1: vld1.8 {<y0=reg128#1%bot},[<input_1=int32#2]!
# asm 2: vld1.8 {<y0=d0},[<input_1=r1]!
vld1.8 {d0},[r1]!

# qhasm: y12 = mem128[input_1]; input_1 += 16
# asm 1: vld1.8 {>y12=reg128#2%bot->y12=reg128#2%top},[<input_1=int32#2]!
# asm 2: vld1.8 {>y12=d2->y12=d3},[<input_1=r1]!
vld1.8 {d2-d3},[r1]!

# qhasm: y34 = mem128[input_1]; input_1 += 16
# asm 1: vld1.8 {>y34=reg128#3%bot->y34=reg128#3%top},[<input_1=int32#2]!
# asm 2: vld1.8 {>y34=d4->y34=d5},[<input_1=r1]!
vld1.8 {d4-d5},[r1]!

# qhasm: input_1 += 8
# asm 1: add >input_1=int32#2,<input_1=int32#2,#8
# asm 2: add >input_1=r1,<input_1=r1,#8
add r1,r1,#8

# qhasm: new z0

# qhasm: z0  = mem64[input_1]z0[1]; input_1 += 8
# asm 1: vld1.8 {<z0=reg128#4%bot},[<input_1=int32#2]!
# asm 2: vld1.8 {<z0=d6},[<input_1=r1]!
vld1.8 {d6},[r1]!

# qhasm: z12 = mem128[input_1]; input_1 += 16
# asm 1: vld1.8 {>z12=reg128#5%bot->z12=reg128#5%top},[<input_1=int32#2]!
# asm 2: vld1.8 {>z12=d8->z12=d9},[<input_1=r1]!
vld1.8 {d8-d9},[r1]!

# qhasm: z34 = mem128[input_1]; input_1 += 16
# asm 1: vld1.8 {>z34=reg128#6%bot->z34=reg128#6%top},[<input_1=int32#2]!
# asm 2: vld1.8 {>z34=d10->z34=d11},[<input_1=r1]!
vld1.8 {d10-d11},[r1]!

# qhasm: 2x mask = 0xffffffff
# asm 1: vmov.i64 >mask=reg128#7,#0xffffffff
# asm 2: vmov.i64 >mask=q6,#0xffffffff
vmov.i64 q6,#0xffffffff

# qhasm: 2x u4 = 0xff
# asm 1: vmov.i64 >u4=reg128#8,#0xff
# asm 2: vmov.i64 >u4=q7,#0xff
vmov.i64 q7,#0xff

# qhasm: x01 aligned= mem128[input_0];input_0+=16
# asm 1: vld1.8 {>x01=reg128#9%bot->x01=reg128#9%top},[<input_0=int32#1,: 128]!
# asm 2: vld1.8 {>x01=d16->x01=d17},[<input_0=r0,: 128]!
vld1.8 {d16-d17},[r0,: 128]!

# qhasm: x23 aligned= mem128[input_0];input_0+=16
# asm 1: vld1.8 {>x23=reg128#10%bot->x23=reg128#10%top},[<input_0=int32#1,: 128]!
# asm 2: vld1.8 {>x23=d18->x23=d19},[<input_0=r0,: 128]!
vld1.8 {d18-d19},[r0,: 128]!

# qhasm: x4  aligned= mem64[input_0]x4[1]
# asm 1: vld1.8 {<x4=reg128#11%bot},[<input_0=int32#1,: 64]
# asm 2: vld1.8 {<x4=d20},[<input_0=r0,: 64]
vld1.8 {d20},[r0,: 64]

# qhasm: input_0 -= 32
# asm 1: sub >input_0=int32#1,<input_0=int32#1,#32
# asm 2: sub >input_0=r0,<input_0=r0,#32
sub r0,r0,#32

# qhasm: 2x mask unsigned>>=6
# asm 1: vshr.u64 >mask=reg128#7,<mask=reg128#7,#6
# asm 2: vshr.u64 >mask=q6,<mask=q6,#6
vshr.u64 q6,q6,#6

# qhasm: 2x u4 unsigned>>= 7
# asm 1: vshr.u64 >u4=reg128#8,<u4=reg128#8,#7
# asm 2: vshr.u64 >u4=q7,<u4=q7,#7
vshr.u64 q7,q7,#7

# qhasm: 4x 5y12 = y12 << 2
# asm 1: vshl.i32 >5y12=reg128#12,<y12=reg128#2,#2
# asm 2: vshl.i32 >5y12=q11,<y12=q1,#2
vshl.i32 q11,q1,#2

# qhasm: 4x 5y34 = y34 << 2
# asm 1: vshl.i32 >5y34=reg128#13,<y34=reg128#3,#2
# asm 2: vshl.i32 >5y34=q12,<y34=q2,#2
vshl.i32 q12,q2,#2

# qhasm: 4x 5y12 += y12
# asm 1: vadd.i32 >5y12=reg128#12,<5y12=reg128#12,<y12=reg128#2
# asm 2: vadd.i32 >5y12=q11,<5y12=q11,<y12=q1
vadd.i32 q11,q11,q1

# qhasm: 4x 5y34 += y34
# asm 1: vadd.i32 >5y34=reg128#13,<5y34=reg128#13,<y34=reg128#3
# asm 2: vadd.i32 >5y34=q12,<5y34=q12,<y34=q2
vadd.i32 q12,q12,q2

# qhasm: 2x u4 <<= 24
# asm 1: vshl.i64 >u4=reg128#8,<u4=reg128#8,#24
# asm 2: vshl.i64 >u4=q7,<u4=q7,#24
vshl.i64 q7,q7,#24

# qhasm: 4x 5z12 = z12 << 2
# asm 1: vshl.i32 >5z12=reg128#14,<z12=reg128#5,#2
# asm 2: vshl.i32 >5z12=q13,<z12=q4,#2
vshl.i32 q13,q4,#2

# qhasm: 4x 5z34 = z34 << 2
# asm 1: vshl.i32 >5z34=reg128#15,<z34=reg128#6,#2
# asm 2: vshl.i32 >5z34=q14,<z34=q5,#2
vshl.i32 q14,q5,#2

# qhasm: 4x 5z12 += z12
# asm 1: vadd.i32 >5z12=reg128#14,<5z12=reg128#14,<z12=reg128#5
# asm 2: vadd.i32 >5z12=q13,<5z12=q13,<z12=q4
vadd.i32 q13,q13,q4

# qhasm: 4x 5z34 += z34
# asm 1: vadd.i32 >5z34=reg128#15,<5z34=reg128#15,<z34=reg128#6
# asm 2: vadd.i32 >5z34=q14,<5z34=q14,<z34=q5
vadd.i32 q14,q14,q5

# qhasm: new two24

# qhasm: new y0_stack

# qhasm: new y12_stack

# qhasm: new y34_stack

# qhasm: new 5y12_stack

# qhasm: new 5y34_stack

# qhasm: new z0_stack

# qhasm: new z12_stack

# qhasm: new z34_stack

# qhasm: new 5z12_stack

# qhasm: new 5z34_stack

# qhasm: ptr = &two24
# asm 1: lea >ptr=int32#2,<two24=stack128#1
# asm 2: lea >ptr=r1,<two24=[sp,#0]
add r1,sp,#0

# qhasm: mem128[ptr] aligned= u4
# asm 1: vst1.8 {<u4=reg128#8%bot-<u4=reg128#8%top},[<ptr=int32#2,: 128]
# asm 2: vst1.8 {<u4=d14-<u4=d15},[<ptr=r1,: 128]
vst1.8 {d14-d15},[r1,: 128]

# qhasm: r4 = u4
# asm 1: vmov >r4=reg128#16,<u4=reg128#8
# asm 2: vmov >r4=q15,<u4=q7
vmov q15,q7

# qhasm: r0 = u4
# asm 1: vmov >r0=reg128#8,<u4=reg128#8
# asm 2: vmov >r0=q7,<u4=q7
vmov q7,q7

# qhasm: ptr = &y0_stack
# asm 1: lea >ptr=int32#2,<y0_stack=stack128#2
# asm 2: lea >ptr=r1,<y0_stack=[sp,#16]
add r1,sp,#16

# qhasm: mem128[ptr] aligned= y0
# asm 1: vst1.8 {<y0=reg128#1%bot-<y0=reg128#1%top},[<ptr=int32#2,: 128]
# asm 2: vst1.8 {<y0=d0-<y0=d1},[<ptr=r1,: 128]
vst1.8 {d0-d1},[r1,: 128]

# qhasm: ptr = &y12_stack
# asm 1: lea >ptr=int32#2,<y12_stack=stack128#3
# asm 2: lea >ptr=r1,<y12_stack=[sp,#32]
add r1,sp,#32

# qhasm: mem128[ptr] aligned= y12
# asm 1: vst1.8 {<y12=reg128#2%bot-<y12=reg128#2%top},[<ptr=int32#2,: 128]
# asm 2: vst1.8 {<y12=d2-<y12=d3},[<ptr=r1,: 128]
vst1.8 {d2-d3},[r1,: 128]

# qhasm: ptr = &y34_stack
# asm 1: lea >ptr=int32#2,<y34_stack=stack128#4
# asm 2: lea >ptr=r1,<y34_stack=[sp,#48]
add r1,sp,#48

# qhasm: mem128[ptr] aligned= y34
# asm 1: vst1.8 {<y34=reg128#3%bot-<y34=reg128#3%top},[<ptr=int32#2,: 128]
# asm 2: vst1.8 {<y34=d4-<y34=d5},[<ptr=r1,: 128]
vst1.8 {d4-d5},[r1,: 128]

# qhasm: ptr = &z0_stack
# asm 1: lea >ptr=int32#2,<z0_stack=stack128#7
# asm 2: lea >ptr=r1,<z0_stack=[sp,#96]
add r1,sp,#96

# qhasm: mem128[ptr] aligned= z0
# asm 1: vst1.8 {<z0=reg128#4%bot-<z0=reg128#4%top},[<ptr=int32#2,: 128]
# asm 2: vst1.8 {<z0=d6-<z0=d7},[<ptr=r1,: 128]
vst1.8 {d6-d7},[r1,: 128]

# qhasm: ptr = &z12_stack
# asm 1: lea >ptr=int32#2,<z12_stack=stack128#8
# asm 2: lea >ptr=r1,<z12_stack=[sp,#112]
add r1,sp,#112

# qhasm: mem128[ptr] aligned= z12
# asm 1: vst1.8 {<z12=reg128#5%bot-<z12=reg128#5%top},[<ptr=int32#2,: 128]
# asm 2: vst1.8 {<z12=d8-<z12=d9},[<ptr=r1,: 128]
vst1.8 {d8-d9},[r1,: 128]

# qhasm: ptr = &z34_stack
# asm 1: lea >ptr=int32#2,<z34_stack=stack128#9
# asm 2: lea >ptr=r1,<z34_stack=[sp,#128]
add r1,sp,#128

# qhasm: mem128[ptr] aligned= z34
# asm 1: vst1.8 {<z34=reg128#6%bot-<z34=reg128#6%top},[<ptr=int32#2,: 128]
# asm 2: vst1.8 {<z34=d10-<z34=d11},[<ptr=r1,: 128]
vst1.8 {d10-d11},[r1,: 128]

# qhasm: ptr = &5y12_stack
# asm 1: lea >ptr=int32#2,<5y12_stack=stack128#5
# asm 2: lea >ptr=r1,<5y12_stack=[sp,#64]
add r1,sp,#64

# qhasm: mem128[ptr] aligned= 5y12
# asm 1: vst1.8 {<5y12=reg128#12%bot-<5y12=reg128#12%top},[<ptr=int32#2,: 128]
# asm 2: vst1.8 {<5y12=d22-<5y12=d23},[<ptr=r1,: 128]
vst1.8 {d22-d23},[r1,: 128]

# qhasm: ptr = &5y34_stack
# asm 1: lea >ptr=int32#2,<5y34_stack=stack128#6
# asm 2: lea >ptr=r1,<5y34_stack=[sp,#80]
add r1,sp,#80

# qhasm: mem128[ptr] aligned= 5y34
# asm 1: vst1.8 {<5y34=reg128#13%bot-<5y34=reg128#13%top},[<ptr=int32#2,: 128]
# asm 2: vst1.8 {<5y34=d24-<5y34=d25},[<ptr=r1,: 128]
vst1.8 {d24-d25},[r1,: 128]

# qhasm: ptr = &5z12_stack
# asm 1: lea >ptr=int32#2,<5z12_stack=stack128#10
# asm 2: lea >ptr=r1,<5z12_stack=[sp,#144]
add r1,sp,#144

# qhasm: mem128[ptr] aligned= 5z12
# asm 1: vst1.8 {<5z12=reg128#14%bot-<5z12=reg128#14%top},[<ptr=int32#2,: 128]
# asm 2: vst1.8 {<5z12=d26-<5z12=d27},[<ptr=r1,: 128]
vst1.8 {d26-d27},[r1,: 128]

# qhasm: ptr = &5z34_stack
# asm 1: lea >ptr=int32#2,<5z34_stack=stack128#11
# asm 2: lea >ptr=r1,<5z34_stack=[sp,#160]
add r1,sp,#160

# qhasm: mem128[ptr] aligned= 5z34
# asm 1: vst1.8 {<5z34=reg128#15%bot-<5z34=reg128#15%top},[<ptr=int32#2,: 128]
# asm 2: vst1.8 {<5z34=d28-<5z34=d29},[<ptr=r1,: 128]
vst1.8 {d28-d29},[r1,: 128]

# qhasm:                       unsigned>? len - 64
# asm 1: cmp <len=int32#4,#64
# asm 2: cmp <len=r3,#64
cmp r3,#64

# qhasm: goto below64bytes if !unsigned>
bls ._below64bytes

# qhasm: input_2 += 32
# asm 1: add >input_2=int32#2,<input_2=int32#3,#32
# asm 2: add >input_2=r1,<input_2=r2,#32
add r1,r2,#32

# qhasm: mainloop2:
._mainloop2:

# qhasm:   c01 = mem128[input_2];input_2+=16 
# asm 1: vld1.8 {>c01=reg128#1%bot->c01=reg128#1%top},[<input_2=int32#2]!
# asm 2: vld1.8 {>c01=d0->c01=d1},[<input_2=r1]!
vld1.8 {d0-d1},[r1]!

# qhasm:   c23 = mem128[input_2];input_2+=16 
# asm 1: vld1.8 {>c23=reg128#2%bot->c23=reg128#2%top},[<input_2=int32#2]!
# asm 2: vld1.8 {>c23=d2->c23=d3},[<input_2=r1]!
vld1.8 {d2-d3},[r1]!

# qhasm: r4[0,1] += x01[0] unsigned*  z34[2];  r4[2,3] += x01[1] unsigned*  z34[3]
# asm 1: vmlal.u32 <r4=reg128#16,<x01=reg128#9%bot,<z34=reg128#6%top
# asm 2: vmlal.u32 <r4=q15,<x01=d16,<z34=d11
vmlal.u32 q15,d16,d11

# qhasm:   ptr = &z12_stack
# asm 1: lea >ptr=int32#3,<z12_stack=stack128#8
# asm 2: lea >ptr=r2,<z12_stack=[sp,#112]
add r2,sp,#112

# qhasm:   z12 aligned= mem128[ptr]
# asm 1: vld1.8 {>z12=reg128#3%bot->z12=reg128#3%top},[<ptr=int32#3,: 128]
# asm 2: vld1.8 {>z12=d4->z12=d5},[<ptr=r2,: 128]
vld1.8 {d4-d5},[r2,: 128]

# qhasm: r4[0,1] += x01[2] unsigned* z34[0];  r4[2,3] += x01[3] unsigned* z34[1]
# asm 1: vmlal.u32 <r4=reg128#16,<x01=reg128#9%top,<z34=reg128#6%bot
# asm 2: vmlal.u32 <r4=q15,<x01=d17,<z34=d10
vmlal.u32 q15,d17,d10

# qhasm:   ptr = &z0_stack
# asm 1: lea >ptr=int32#3,<z0_stack=stack128#7
# asm 2: lea >ptr=r2,<z0_stack=[sp,#96]
add r2,sp,#96

# qhasm:   z0 aligned= mem128[ptr]
# asm 1: vld1.8 {>z0=reg128#4%bot->z0=reg128#4%top},[<ptr=int32#3,: 128]
# asm 2: vld1.8 {>z0=d6->z0=d7},[<ptr=r2,: 128]
vld1.8 {d6-d7},[r2,: 128]

# qhasm: r4[0,1] += x23[0] unsigned* z12[2];  r4[2,3] += x23[1] unsigned* z12[3]
# asm 1: vmlal.u32 <r4=reg128#16,<x23=reg128#10%bot,<z12=reg128#3%top
# asm 2: vmlal.u32 <r4=q15,<x23=d18,<z12=d5
vmlal.u32 q15,d18,d5

# qhasm:   c01 c23 = c01[0]c01[1]c01[2]c23[2]c23[0]c23[1]c01[3]c23[3]
# asm 1: vtrn.32 <c01=reg128#1%top,<c23=reg128#2%top
# asm 2: vtrn.32 <c01=d1,<c23=d3
vtrn.32 d1,d3

# qhasm: r4[0,1] += x23[2] unsigned* z12[0];  r4[2,3] += x23[3] unsigned* z12[1]
# asm 1: vmlal.u32 <r4=reg128#16,<x23=reg128#10%top,<z12=reg128#3%bot
# asm 2: vmlal.u32 <r4=q15,<x23=d19,<z12=d4
vmlal.u32 q15,d19,d4

# qhasm: r4[0,1] +=  x4[0] unsigned* z0[0];  r4[2,3] +=  x4[1] unsigned* z0[1]
# asm 1: vmlal.u32 <r4=reg128#16,<x4=reg128#11%bot,<z0=reg128#4%bot
# asm 2: vmlal.u32 <r4=q15,<x4=d20,<z0=d6
vmlal.u32 q15,d20,d6

# qhasm: r3[0,1] = c23[2]<<18; r3[2,3] = c23[3]<<18 
# asm 1: vshll.u32 >r3=reg128#5,<c23=reg128#2%top,#18
# asm 2: vshll.u32 >r3=q4,<c23=d3,#18
vshll.u32 q4,d3,#18

# qhasm:   c01 c23 = c01[0]c23[0]c01[2]c01[3]c01[1]c23[1]c23[2]c23[3]
# asm 1: vtrn.32 <c01=reg128#1%bot,<c23=reg128#2%bot
# asm 2: vtrn.32 <c01=d0,<c23=d2
vtrn.32 d0,d2

# qhasm: r3[0,1] += x01[0] unsigned* z34[0];   r3[2,3] += x01[1] unsigned* z34[1]
# asm 1: vmlal.u32 <r3=reg128#5,<x01=reg128#9%bot,<z34=reg128#6%bot
# asm 2: vmlal.u32 <r3=q4,<x01=d16,<z34=d10
vmlal.u32 q4,d16,d10

# qhasm: r3[0,1] += x01[2] unsigned* z12[2];   r3[2,3] += x01[3] unsigned* z12[3]
# asm 1: vmlal.u32 <r3=reg128#5,<x01=reg128#9%top,<z12=reg128#3%top
# asm 2: vmlal.u32 <r3=q4,<x01=d17,<z12=d5
vmlal.u32 q4,d17,d5

# qhasm:   r0 = r0[1]c01[0]r0[2,3] 
# asm 1: vext.32 <r0=reg128#8%bot,<r0=reg128#8%bot,<c01=reg128#1%bot,#1
# asm 2: vext.32 <r0=d14,<r0=d14,<c01=d0,#1
vext.32 d14,d14,d0,#1

# qhasm: r3[0,1] += x23[0] unsigned* z12[0];   r3[2,3] += x23[1] unsigned* z12[1]
# asm 1: vmlal.u32 <r3=reg128#5,<x23=reg128#10%bot,<z12=reg128#3%bot
# asm 2: vmlal.u32 <r3=q4,<x23=d18,<z12=d4
vmlal.u32 q4,d18,d4

# qhasm: 								input_2 -= 64
# asm 1: sub >input_2=int32#2,<input_2=int32#2,#64
# asm 2: sub >input_2=r1,<input_2=r1,#64
sub r1,r1,#64

# qhasm: r3[0,1] += x23[2] unsigned* z0[0];   r3[2,3] += x23[3] unsigned* z0[1]
# asm 1: vmlal.u32 <r3=reg128#5,<x23=reg128#10%top,<z0=reg128#4%bot
# asm 2: vmlal.u32 <r3=q4,<x23=d19,<z0=d6
vmlal.u32 q4,d19,d6

# qhasm:   ptr = &5z34_stack
# asm 1: lea >ptr=int32#3,<5z34_stack=stack128#11
# asm 2: lea >ptr=r2,<5z34_stack=[sp,#160]
add r2,sp,#160

# qhasm:   5z34 aligned= mem128[ptr]
# asm 1: vld1.8 {>5z34=reg128#6%bot->5z34=reg128#6%top},[<ptr=int32#3,: 128]
# asm 2: vld1.8 {>5z34=d10->5z34=d11},[<ptr=r2,: 128]
vld1.8 {d10-d11},[r2,: 128]

# qhasm: r3[0,1] +=  x4[0] unsigned*  5z34[2]; r3[2,3] +=  x4[1] unsigned*  5z34[3]
# asm 1: vmlal.u32 <r3=reg128#5,<x4=reg128#11%bot,<5z34=reg128#6%top
# asm 2: vmlal.u32 <r3=q4,<x4=d20,<5z34=d11
vmlal.u32 q4,d20,d11

# qhasm:   r0 = r0[1]r0[0]r0[3]r0[2] 
# asm 1: vrev64.i32 >r0=reg128#8,<r0=reg128#8
# asm 2: vrev64.i32 >r0=q7,<r0=q7
vrev64.i32 q7,q7

# qhasm:   r2[0,1] = c01[2]<<12; r2[2,3] = c01[3]<<12 
# asm 1: vshll.u32 >r2=reg128#14,<c01=reg128#1%top,#12
# asm 2: vshll.u32 >r2=q13,<c01=d1,#12
vshll.u32 q13,d1,#12

# qhasm:   		d01 = mem128[input_2];input_2+=16 
# asm 1: vld1.8 {>d01=reg128#12%bot->d01=reg128#12%top},[<input_2=int32#2]!
# asm 2: vld1.8 {>d01=d22->d01=d23},[<input_2=r1]!
vld1.8 {d22-d23},[r1]!

# qhasm: r2[0,1] += x01[0] unsigned* z12[2];   r2[2,3] += x01[1] unsigned* z12[3]
# asm 1: vmlal.u32 <r2=reg128#14,<x01=reg128#9%bot,<z12=reg128#3%top
# asm 2: vmlal.u32 <r2=q13,<x01=d16,<z12=d5
vmlal.u32 q13,d16,d5

# qhasm: r2[0,1] += x01[2] unsigned* z12[0];   r2[2,3] += x01[3] unsigned* z12[1]
# asm 1: vmlal.u32 <r2=reg128#14,<x01=reg128#9%top,<z12=reg128#3%bot
# asm 2: vmlal.u32 <r2=q13,<x01=d17,<z12=d4
vmlal.u32 q13,d17,d4

# qhasm: r2[0,1] += x23[0] unsigned* z0[0];   r2[2,3] += x23[1] unsigned* z0[1]
# asm 1: vmlal.u32 <r2=reg128#14,<x23=reg128#10%bot,<z0=reg128#4%bot
# asm 2: vmlal.u32 <r2=q13,<x23=d18,<z0=d6
vmlal.u32 q13,d18,d6

# qhasm: r2[0,1] += x23[2] unsigned*  5z34[2]; r2[2,3] += x23[3] unsigned*  5z34[3]
# asm 1: vmlal.u32 <r2=reg128#14,<x23=reg128#10%top,<5z34=reg128#6%top
# asm 2: vmlal.u32 <r2=q13,<x23=d19,<5z34=d11
vmlal.u32 q13,d19,d11

# qhasm: r2[0,1] +=  x4[0] unsigned* 5z34[0]; r2[2,3] +=  x4[1] unsigned* 5z34[1]
# asm 1: vmlal.u32 <r2=reg128#14,<x4=reg128#11%bot,<5z34=reg128#6%bot
# asm 2: vmlal.u32 <r2=q13,<x4=d20,<5z34=d10
vmlal.u32 q13,d20,d10

# qhasm:   r0 = r0[0,1]c01[1]r0[2] 
# asm 1: vext.32 <r0=reg128#8%top,<c01=reg128#1%bot,<r0=reg128#8%top,#1
# asm 2: vext.32 <r0=d15,<c01=d0,<r0=d15,#1
vext.32 d15,d0,d15,#1

# qhasm:   r1[0,1] = c23[0]<<6; r1[2,3] = c23[1]<<6 
# asm 1: vshll.u32 >r1=reg128#15,<c23=reg128#2%bot,#6
# asm 2: vshll.u32 >r1=q14,<c23=d2,#6
vshll.u32 q14,d2,#6

# qhasm: r1[0,1] += x01[0] unsigned* z12[0];   r1[2,3] += x01[1] unsigned* z12[1]
# asm 1: vmlal.u32 <r1=reg128#15,<x01=reg128#9%bot,<z12=reg128#3%bot
# asm 2: vmlal.u32 <r1=q14,<x01=d16,<z12=d4
vmlal.u32 q14,d16,d4

# qhasm: r1[0,1] += x01[2] unsigned* z0[0];   r1[2,3] += x01[3] unsigned* z0[1]
# asm 1: vmlal.u32 <r1=reg128#15,<x01=reg128#9%top,<z0=reg128#4%bot
# asm 2: vmlal.u32 <r1=q14,<x01=d17,<z0=d6
vmlal.u32 q14,d17,d6

# qhasm: r1[0,1] += x23[0] unsigned*  5z34[2]; r1[2,3] += x23[1] unsigned*  5z34[3]
# asm 1: vmlal.u32 <r1=reg128#15,<x23=reg128#10%bot,<5z34=reg128#6%top
# asm 2: vmlal.u32 <r1=q14,<x23=d18,<5z34=d11
vmlal.u32 q14,d18,d11

# qhasm: r1[0,1] += x23[2] unsigned* 5z34[0]; r1[2,3] += x23[3] unsigned* 5z34[1]
# asm 1: vmlal.u32 <r1=reg128#15,<x23=reg128#10%top,<5z34=reg128#6%bot
# asm 2: vmlal.u32 <r1=q14,<x23=d19,<5z34=d10
vmlal.u32 q14,d19,d10

# qhasm: ptr = &5z12_stack
# asm 1: lea >ptr=int32#3,<5z12_stack=stack128#10
# asm 2: lea >ptr=r2,<5z12_stack=[sp,#144]
add r2,sp,#144

# qhasm: 5z12 aligned= mem128[ptr]
# asm 1: vld1.8 {>5z12=reg128#1%bot->5z12=reg128#1%top},[<ptr=int32#3,: 128]
# asm 2: vld1.8 {>5z12=d0->5z12=d1},[<ptr=r2,: 128]
vld1.8 {d0-d1},[r2,: 128]

# qhasm: r1[0,1] +=  x4[0] unsigned* 5z12[2]; r1[2,3] +=  x4[1] unsigned* 5z12[3]
# asm 1: vmlal.u32 <r1=reg128#15,<x4=reg128#11%bot,<5z12=reg128#1%top
# asm 2: vmlal.u32 <r1=q14,<x4=d20,<5z12=d1
vmlal.u32 q14,d20,d1

# qhasm:   		d23 = mem128[input_2];input_2+=16 
# asm 1: vld1.8 {>d23=reg128#2%bot->d23=reg128#2%top},[<input_2=int32#2]!
# asm 2: vld1.8 {>d23=d2->d23=d3},[<input_2=r1]!
vld1.8 {d2-d3},[r1]!

# qhasm:   		input_2 += 32
# asm 1: add >input_2=int32#2,<input_2=int32#2,#32
# asm 2: add >input_2=r1,<input_2=r1,#32
add r1,r1,#32

# qhasm: r0[0,1] +=  x4[0] unsigned* 5z12[0]; r0[2,3] +=  x4[1] unsigned* 5z12[1]
# asm 1: vmlal.u32 <r0=reg128#8,<x4=reg128#11%bot,<5z12=reg128#1%bot
# asm 2: vmlal.u32 <r0=q7,<x4=d20,<5z12=d0
vmlal.u32 q7,d20,d0

# qhasm: r0[0,1] += x23[0] unsigned* 5z34[0]; r0[2,3] += x23[1] unsigned* 5z34[1]
# asm 1: vmlal.u32 <r0=reg128#8,<x23=reg128#10%bot,<5z34=reg128#6%bot
# asm 2: vmlal.u32 <r0=q7,<x23=d18,<5z34=d10
vmlal.u32 q7,d18,d10

# qhasm:   		d01 d23 = d01[0] d23[0] d01[1] d23[1] 
# asm 1: vswp <d23=reg128#2%bot,<d01=reg128#12%top
# asm 2: vswp <d23=d2,<d01=d23
vswp d2,d23

# qhasm: r0[0,1] += x23[2] unsigned* 5z12[2]; r0[2,3] += x23[3] unsigned* 5z12[3]
# asm 1: vmlal.u32 <r0=reg128#8,<x23=reg128#10%top,<5z12=reg128#1%top
# asm 2: vmlal.u32 <r0=q7,<x23=d19,<5z12=d1
vmlal.u32 q7,d19,d1

# qhasm: r0[0,1] += x01[0] unsigned* z0[0];   r0[2,3] += x01[1] unsigned* z0[1]
# asm 1: vmlal.u32 <r0=reg128#8,<x01=reg128#9%bot,<z0=reg128#4%bot
# asm 2: vmlal.u32 <r0=q7,<x01=d16,<z0=d6
vmlal.u32 q7,d16,d6

# qhasm:   		new mid

# qhasm:   		2x v4 = d23 unsigned>> 40  
# asm 1: vshr.u64 >v4=reg128#4,<d23=reg128#2,#40
# asm 2: vshr.u64 >v4=q3,<d23=q1,#40
vshr.u64 q3,q1,#40

# qhasm:   		mid = d01[1]d23[0] mid[2,3] 
# asm 1: vext.32 <mid=reg128#1%bot,<d01=reg128#12%bot,<d23=reg128#2%bot,#1
# asm 2: vext.32 <mid=d0,<d01=d22,<d23=d2,#1
vext.32 d0,d22,d2,#1

# qhasm:   		new v23

# qhasm:   		v23[2] = d23[0,1] unsigned>> 14; v23[3] = d23[2,3] unsigned>> 14
# asm 1: vshrn.u64 <v23=reg128#10%top,<d23=reg128#2,#14
# asm 2: vshrn.u64 <v23=d19,<d23=q1,#14
vshrn.u64 d19,q1,#14

# qhasm:   		mid = mid[0,1] d01[3]d23[2] 
# asm 1: vext.32 <mid=reg128#1%top,<d01=reg128#12%top,<d23=reg128#2%top,#1
# asm 2: vext.32 <mid=d1,<d01=d23,<d23=d3,#1
vext.32 d1,d23,d3,#1

# qhasm:   		new v01

# qhasm:   		v01[2] = d01[0,1] unsigned>> 26; v01[3] = d01[2,3] unsigned>> 26
# asm 1: vshrn.u64 <v01=reg128#11%top,<d01=reg128#12,#26
# asm 2: vshrn.u64 <v01=d21,<d01=q11,#26
vshrn.u64 d21,q11,#26

# qhasm:   		v01 = d01[1]d01[0] v01[2,3] 
# asm 1: vext.32 <v01=reg128#11%bot,<d01=reg128#12%bot,<d01=reg128#12%bot,#1
# asm 2: vext.32 <v01=d20,<d01=d22,<d01=d22,#1
vext.32 d20,d22,d22,#1

# qhasm: r0[0,1] += x01[2] unsigned*  5z34[2]; r0[2,3] += x01[3] unsigned*  5z34[3]
# asm 1: vmlal.u32 <r0=reg128#8,<x01=reg128#9%top,<5z34=reg128#6%top
# asm 2: vmlal.u32 <r0=q7,<x01=d17,<5z34=d11
vmlal.u32 q7,d17,d11

# qhasm:   		v01 = v01[1]d01[2] v01[2,3] 
# asm 1: vext.32 <v01=reg128#11%bot,<v01=reg128#11%bot,<d01=reg128#12%top,#1
# asm 2: vext.32 <v01=d20,<v01=d20,<d01=d23,#1
vext.32 d20,d20,d23,#1

# qhasm:   		v23[0] = mid[0,1] unsigned>> 20; v23[1] = mid[2,3] unsigned>> 20
# asm 1: vshrn.u64 <v23=reg128#10%bot,<mid=reg128#1,#20
# asm 2: vshrn.u64 <v23=d18,<mid=q0,#20
vshrn.u64 d18,q0,#20

# qhasm:   		v4 = v4[0]v4[2]v4[1]v4[3]  
# asm 1: vtrn.32 <v4=reg128#4%bot,<v4=reg128#4%top
# asm 2: vtrn.32 <v4=d6,<v4=d7
vtrn.32 d6,d7

# qhasm:   		4x v01 &= 0x03ffffff
# asm 1: vand.i32 <v01=reg128#11,#0x03ffffff
# asm 2: vand.i32 <v01=q10,#0x03ffffff
vand.i32 q10,#0x03ffffff

# qhasm: ptr = &y34_stack
# asm 1: lea >ptr=int32#3,<y34_stack=stack128#4
# asm 2: lea >ptr=r2,<y34_stack=[sp,#48]
add r2,sp,#48

# qhasm: y34 aligned= mem128[ptr]
# asm 1: vld1.8 {>y34=reg128#3%bot->y34=reg128#3%top},[<ptr=int32#3,: 128]
# asm 2: vld1.8 {>y34=d4->y34=d5},[<ptr=r2,: 128]
vld1.8 {d4-d5},[r2,: 128]

# qhasm:   		4x v23 &= 0x03ffffff
# asm 1: vand.i32 <v23=reg128#10,#0x03ffffff
# asm 2: vand.i32 <v23=q9,#0x03ffffff
vand.i32 q9,#0x03ffffff

# qhasm: ptr = &y12_stack
# asm 1: lea >ptr=int32#3,<y12_stack=stack128#3
# asm 2: lea >ptr=r2,<y12_stack=[sp,#32]
add r2,sp,#32

# qhasm: y12 aligned= mem128[ptr]
# asm 1: vld1.8 {>y12=reg128#2%bot->y12=reg128#2%top},[<ptr=int32#3,: 128]
# asm 2: vld1.8 {>y12=d2->y12=d3},[<ptr=r2,: 128]
vld1.8 {d2-d3},[r2,: 128]

# qhasm:   		4x v4 |= 0x01000000
# asm 1: vorr.i32 <v4=reg128#4,#0x01000000
# asm 2: vorr.i32 <v4=q3,#0x01000000
vorr.i32 q3,#0x01000000

# qhasm: ptr = &y0_stack
# asm 1: lea >ptr=int32#3,<y0_stack=stack128#2
# asm 2: lea >ptr=r2,<y0_stack=[sp,#16]
add r2,sp,#16

# qhasm: y0 aligned= mem128[ptr]
# asm 1: vld1.8 {>y0=reg128#1%bot->y0=reg128#1%top},[<ptr=int32#3,: 128]
# asm 2: vld1.8 {>y0=d0->y0=d1},[<ptr=r2,: 128]
vld1.8 {d0-d1},[r2,: 128]

# qhasm: r4[0,1] += v01[0] unsigned*  y34[2];  r4[2,3] += v01[1] unsigned*  y34[3]
# asm 1: vmlal.u32 <r4=reg128#16,<v01=reg128#11%bot,<y34=reg128#3%top
# asm 2: vmlal.u32 <r4=q15,<v01=d20,<y34=d5
vmlal.u32 q15,d20,d5

# qhasm: r4[0,1] += v01[2] unsigned* y34[0];  r4[2,3] += v01[3] unsigned* y34[1]
# asm 1: vmlal.u32 <r4=reg128#16,<v01=reg128#11%top,<y34=reg128#3%bot
# asm 2: vmlal.u32 <r4=q15,<v01=d21,<y34=d4
vmlal.u32 q15,d21,d4

# qhasm: r4[0,1] += v23[0] unsigned* y12[2];  r4[2,3] += v23[1] unsigned* y12[3]
# asm 1: vmlal.u32 <r4=reg128#16,<v23=reg128#10%bot,<y12=reg128#2%top
# asm 2: vmlal.u32 <r4=q15,<v23=d18,<y12=d3
vmlal.u32 q15,d18,d3

# qhasm: r4[0,1] += v23[2] unsigned* y12[0];  r4[2,3] += v23[3] unsigned* y12[1]
# asm 1: vmlal.u32 <r4=reg128#16,<v23=reg128#10%top,<y12=reg128#2%bot
# asm 2: vmlal.u32 <r4=q15,<v23=d19,<y12=d2
vmlal.u32 q15,d19,d2

# qhasm: r4[0,1] +=  v4[0] unsigned* y0[0];  r4[2,3] +=  v4[1] unsigned* y0[1]
# asm 1: vmlal.u32 <r4=reg128#16,<v4=reg128#4%bot,<y0=reg128#1%bot
# asm 2: vmlal.u32 <r4=q15,<v4=d6,<y0=d0
vmlal.u32 q15,d6,d0

# qhasm: ptr = &5y34_stack
# asm 1: lea >ptr=int32#3,<5y34_stack=stack128#6
# asm 2: lea >ptr=r2,<5y34_stack=[sp,#80]
add r2,sp,#80

# qhasm: 5y34 aligned= mem128[ptr]
# asm 1: vld1.8 {>5y34=reg128#13%bot->5y34=reg128#13%top},[<ptr=int32#3,: 128]
# asm 2: vld1.8 {>5y34=d24->5y34=d25},[<ptr=r2,: 128]
vld1.8 {d24-d25},[r2,: 128]

# qhasm: r3[0,1] += v01[0] unsigned* y34[0];   r3[2,3] += v01[1] unsigned* y34[1]
# asm 1: vmlal.u32 <r3=reg128#5,<v01=reg128#11%bot,<y34=reg128#3%bot
# asm 2: vmlal.u32 <r3=q4,<v01=d20,<y34=d4
vmlal.u32 q4,d20,d4

# qhasm: r3[0,1] += v01[2] unsigned* y12[2];   r3[2,3] += v01[3] unsigned* y12[3]
# asm 1: vmlal.u32 <r3=reg128#5,<v01=reg128#11%top,<y12=reg128#2%top
# asm 2: vmlal.u32 <r3=q4,<v01=d21,<y12=d3
vmlal.u32 q4,d21,d3

# qhasm: r3[0,1] += v23[0] unsigned* y12[0];   r3[2,3] += v23[1] unsigned* y12[1]
# asm 1: vmlal.u32 <r3=reg128#5,<v23=reg128#10%bot,<y12=reg128#2%bot
# asm 2: vmlal.u32 <r3=q4,<v23=d18,<y12=d2
vmlal.u32 q4,d18,d2

# qhasm: r3[0,1] += v23[2] unsigned* y0[0];   r3[2,3] += v23[3] unsigned* y0[1]
# asm 1: vmlal.u32 <r3=reg128#5,<v23=reg128#10%top,<y0=reg128#1%bot
# asm 2: vmlal.u32 <r3=q4,<v23=d19,<y0=d0
vmlal.u32 q4,d19,d0

# qhasm: r3[0,1] +=  v4[0] unsigned*  5y34[2]; r3[2,3] +=  v4[1] unsigned*  5y34[3]
# asm 1: vmlal.u32 <r3=reg128#5,<v4=reg128#4%bot,<5y34=reg128#13%top
# asm 2: vmlal.u32 <r3=q4,<v4=d6,<5y34=d25
vmlal.u32 q4,d6,d25

# qhasm: ptr = &5y12_stack
# asm 1: lea >ptr=int32#3,<5y12_stack=stack128#5
# asm 2: lea >ptr=r2,<5y12_stack=[sp,#64]
add r2,sp,#64

# qhasm: 5y12 aligned= mem128[ptr]
# asm 1: vld1.8 {>5y12=reg128#12%bot->5y12=reg128#12%top},[<ptr=int32#3,: 128]
# asm 2: vld1.8 {>5y12=d22->5y12=d23},[<ptr=r2,: 128]
vld1.8 {d22-d23},[r2,: 128]

# qhasm: r0[0,1] +=  v4[0] unsigned* 5y12[0]; r0[2,3] +=  v4[1] unsigned* 5y12[1]
# asm 1: vmlal.u32 <r0=reg128#8,<v4=reg128#4%bot,<5y12=reg128#12%bot
# asm 2: vmlal.u32 <r0=q7,<v4=d6,<5y12=d22
vmlal.u32 q7,d6,d22

# qhasm: r0[0,1] += v23[0] unsigned* 5y34[0]; r0[2,3] += v23[1] unsigned* 5y34[1]
# asm 1: vmlal.u32 <r0=reg128#8,<v23=reg128#10%bot,<5y34=reg128#13%bot
# asm 2: vmlal.u32 <r0=q7,<v23=d18,<5y34=d24
vmlal.u32 q7,d18,d24

# qhasm: r0[0,1] += v23[2] unsigned* 5y12[2]; r0[2,3] += v23[3] unsigned* 5y12[3]
# asm 1: vmlal.u32 <r0=reg128#8,<v23=reg128#10%top,<5y12=reg128#12%top
# asm 2: vmlal.u32 <r0=q7,<v23=d19,<5y12=d23
vmlal.u32 q7,d19,d23

# qhasm: r0[0,1] += v01[0] unsigned* y0[0];   r0[2,3] += v01[1] unsigned* y0[1]
# asm 1: vmlal.u32 <r0=reg128#8,<v01=reg128#11%bot,<y0=reg128#1%bot
# asm 2: vmlal.u32 <r0=q7,<v01=d20,<y0=d0
vmlal.u32 q7,d20,d0

# qhasm: r0[0,1] += v01[2] unsigned*  5y34[2]; r0[2,3] += v01[3] unsigned*  5y34[3]
# asm 1: vmlal.u32 <r0=reg128#8,<v01=reg128#11%top,<5y34=reg128#13%top
# asm 2: vmlal.u32 <r0=q7,<v01=d21,<5y34=d25
vmlal.u32 q7,d21,d25

# qhasm: r1[0,1] += v01[0] unsigned* y12[0];   r1[2,3] += v01[1] unsigned* y12[1]
# asm 1: vmlal.u32 <r1=reg128#15,<v01=reg128#11%bot,<y12=reg128#2%bot
# asm 2: vmlal.u32 <r1=q14,<v01=d20,<y12=d2
vmlal.u32 q14,d20,d2

# qhasm: r1[0,1] += v01[2] unsigned* y0[0];   r1[2,3] += v01[3] unsigned* y0[1]
# asm 1: vmlal.u32 <r1=reg128#15,<v01=reg128#11%top,<y0=reg128#1%bot
# asm 2: vmlal.u32 <r1=q14,<v01=d21,<y0=d0
vmlal.u32 q14,d21,d0

# qhasm: r1[0,1] += v23[0] unsigned*  5y34[2]; r1[2,3] += v23[1] unsigned*  5y34[3]
# asm 1: vmlal.u32 <r1=reg128#15,<v23=reg128#10%bot,<5y34=reg128#13%top
# asm 2: vmlal.u32 <r1=q14,<v23=d18,<5y34=d25
vmlal.u32 q14,d18,d25

# qhasm: r1[0,1] += v23[2] unsigned* 5y34[0]; r1[2,3] += v23[3] unsigned* 5y34[1]
# asm 1: vmlal.u32 <r1=reg128#15,<v23=reg128#10%top,<5y34=reg128#13%bot
# asm 2: vmlal.u32 <r1=q14,<v23=d19,<5y34=d24
vmlal.u32 q14,d19,d24

# qhasm: r1[0,1] +=  v4[0] unsigned* 5y12[2]; r1[2,3] +=  v4[1] unsigned* 5y12[3]
# asm 1: vmlal.u32 <r1=reg128#15,<v4=reg128#4%bot,<5y12=reg128#12%top
# asm 2: vmlal.u32 <r1=q14,<v4=d6,<5y12=d23
vmlal.u32 q14,d6,d23

# qhasm: r2[0,1] += v01[0] unsigned* y12[2];   r2[2,3] += v01[1] unsigned* y12[3]
# asm 1: vmlal.u32 <r2=reg128#14,<v01=reg128#11%bot,<y12=reg128#2%top
# asm 2: vmlal.u32 <r2=q13,<v01=d20,<y12=d3
vmlal.u32 q13,d20,d3

# qhasm: r2[0,1] += v01[2] unsigned* y12[0];   r2[2,3] += v01[3] unsigned* y12[1]
# asm 1: vmlal.u32 <r2=reg128#14,<v01=reg128#11%top,<y12=reg128#2%bot
# asm 2: vmlal.u32 <r2=q13,<v01=d21,<y12=d2
vmlal.u32 q13,d21,d2

# qhasm: r2[0,1] += v23[0] unsigned* y0[0];   r2[2,3] += v23[1] unsigned* y0[1]
# asm 1: vmlal.u32 <r2=reg128#14,<v23=reg128#10%bot,<y0=reg128#1%bot
# asm 2: vmlal.u32 <r2=q13,<v23=d18,<y0=d0
vmlal.u32 q13,d18,d0

# qhasm: r2[0,1] += v23[2] unsigned*  5y34[2]; r2[2,3] += v23[3] unsigned*  5y34[3]
# asm 1: vmlal.u32 <r2=reg128#14,<v23=reg128#10%top,<5y34=reg128#13%top
# asm 2: vmlal.u32 <r2=q13,<v23=d19,<5y34=d25
vmlal.u32 q13,d19,d25

# qhasm: r2[0,1] +=  v4[0] unsigned* 5y34[0]; r2[2,3] +=  v4[1] unsigned* 5y34[1]
# asm 1: vmlal.u32 <r2=reg128#14,<v4=reg128#4%bot,<5y34=reg128#13%bot
# asm 2: vmlal.u32 <r2=q13,<v4=d6,<5y34=d24
vmlal.u32 q13,d6,d24

# qhasm: 				ptr = &two24
# asm 1: lea >ptr=int32#3,<two24=stack128#1
# asm 2: lea >ptr=r2,<two24=[sp,#0]
add r2,sp,#0

# qhasm: 2x t1 = r0 unsigned>> 26
# asm 1: vshr.u64 >t1=reg128#4,<r0=reg128#8,#26
# asm 2: vshr.u64 >t1=q3,<r0=q7,#26
vshr.u64 q3,q7,#26

# qhasm:   				len -= 64
# asm 1: sub >len=int32#4,<len=int32#4,#64
# asm 2: sub >len=r3,<len=r3,#64
sub r3,r3,#64

# qhasm:    r0 &= mask
# asm 1: vand >r0=reg128#6,<r0=reg128#8,<mask=reg128#7
# asm 2: vand >r0=q5,<r0=q7,<mask=q6
vand q5,q7,q6

# qhasm: 2x r1 += t1
# asm 1: vadd.i64 >r1=reg128#4,<r1=reg128#15,<t1=reg128#4
# asm 2: vadd.i64 >r1=q3,<r1=q14,<t1=q3
vadd.i64 q3,q14,q3

# qhasm: 		2x t4 = r3 unsigned>> 26
# asm 1: vshr.u64 >t4=reg128#8,<r3=reg128#5,#26
# asm 2: vshr.u64 >t4=q7,<r3=q4,#26
vshr.u64 q7,q4,#26

# qhasm: 		   r3 &= mask
# asm 1: vand >r3=reg128#5,<r3=reg128#5,<mask=reg128#7
# asm 2: vand >r3=q4,<r3=q4,<mask=q6
vand q4,q4,q6

# qhasm: 		2x x4 = r4 + t4
# asm 1: vadd.i64 >x4=reg128#8,<r4=reg128#16,<t4=reg128#8
# asm 2: vadd.i64 >x4=q7,<r4=q15,<t4=q7
vadd.i64 q7,q15,q7

# qhasm: 				r4 aligned= mem128[ptr]
# asm 1: vld1.8 {>r4=reg128#16%bot->r4=reg128#16%top},[<ptr=int32#3,: 128]
# asm 2: vld1.8 {>r4=d30->r4=d31},[<ptr=r2,: 128]
vld1.8 {d30-d31},[r2,: 128]

# qhasm: 2x t2 = r1 unsigned>> 26
# asm 1: vshr.u64 >t2=reg128#9,<r1=reg128#4,#26
# asm 2: vshr.u64 >t2=q8,<r1=q3,#26
vshr.u64 q8,q3,#26

# qhasm:    r1 &= mask
# asm 1: vand >r1=reg128#4,<r1=reg128#4,<mask=reg128#7
# asm 2: vand >r1=q3,<r1=q3,<mask=q6
vand q3,q3,q6

# qhasm: 		2x t0 = x4 unsigned>> 26
# asm 1: vshr.u64 >t0=reg128#10,<x4=reg128#8,#26
# asm 2: vshr.u64 >t0=q9,<x4=q7,#26
vshr.u64 q9,q7,#26

# qhasm: 2x r2 += t2
# asm 1: vadd.i64 >r2=reg128#9,<r2=reg128#14,<t2=reg128#9
# asm 2: vadd.i64 >r2=q8,<r2=q13,<t2=q8
vadd.i64 q8,q13,q8

# qhasm: 		   x4 &= mask
# asm 1: vand >x4=reg128#11,<x4=reg128#8,<mask=reg128#7
# asm 2: vand >x4=q10,<x4=q7,<mask=q6
vand q10,q7,q6

# qhasm: 		2x x01 = r0 + t0
# asm 1: vadd.i64 >x01=reg128#6,<r0=reg128#6,<t0=reg128#10
# asm 2: vadd.i64 >x01=q5,<r0=q5,<t0=q9
vadd.i64 q5,q5,q9

# qhasm: 				r0 aligned= mem128[ptr]
# asm 1: vld1.8 {>r0=reg128#8%bot->r0=reg128#8%top},[<ptr=int32#3,: 128]
# asm 2: vld1.8 {>r0=d14->r0=d15},[<ptr=r2,: 128]
vld1.8 {d14-d15},[r2,: 128]

# qhasm: 				ptr = &z34_stack
# asm 1: lea >ptr=int32#3,<z34_stack=stack128#9
# asm 2: lea >ptr=r2,<z34_stack=[sp,#128]
add r2,sp,#128

# qhasm: 		2x t0 <<= 2
# asm 1: vshl.i64 >t0=reg128#10,<t0=reg128#10,#2
# asm 2: vshl.i64 >t0=q9,<t0=q9,#2
vshl.i64 q9,q9,#2

# qhasm: 2x t3 = r2 unsigned>> 26
# asm 1: vshr.u64 >t3=reg128#14,<r2=reg128#9,#26
# asm 2: vshr.u64 >t3=q13,<r2=q8,#26
vshr.u64 q13,q8,#26

# qhasm: 		2x x01 += t0
# asm 1: vadd.i64 >x01=reg128#15,<x01=reg128#6,<t0=reg128#10
# asm 2: vadd.i64 >x01=q14,<x01=q5,<t0=q9
vadd.i64 q14,q5,q9

# qhasm: 				z34 aligned= mem128[ptr]
# asm 1: vld1.8 {>z34=reg128#6%bot->z34=reg128#6%top},[<ptr=int32#3,: 128]
# asm 2: vld1.8 {>z34=d10->z34=d11},[<ptr=r2,: 128]
vld1.8 {d10-d11},[r2,: 128]

# qhasm:    x23 = r2 & mask
# asm 1: vand >x23=reg128#10,<r2=reg128#9,<mask=reg128#7
# asm 2: vand >x23=q9,<r2=q8,<mask=q6
vand q9,q8,q6

# qhasm: 2x r3 += t3
# asm 1: vadd.i64 >r3=reg128#5,<r3=reg128#5,<t3=reg128#14
# asm 2: vadd.i64 >r3=q4,<r3=q4,<t3=q13
vadd.i64 q4,q4,q13

# qhasm: 								input_2 += 32
# asm 1: add >input_2=int32#2,<input_2=int32#2,#32
# asm 2: add >input_2=r1,<input_2=r1,#32
add r1,r1,#32

# qhasm: 		2x t1 = x01 unsigned>> 26
# asm 1: vshr.u64 >t1=reg128#14,<x01=reg128#15,#26
# asm 2: vshr.u64 >t1=q13,<x01=q14,#26
vshr.u64 q13,q14,#26

# qhasm: 						x23 = x23[0,2,1,3]
# asm 1: vtrn.32 <x23=reg128#10%bot,<x23=reg128#10%top
# asm 2: vtrn.32 <x23=d18,<x23=d19
vtrn.32 d18,d19

# qhasm: 		   x01 = x01 & mask
# asm 1: vand >x01=reg128#9,<x01=reg128#15,<mask=reg128#7
# asm 2: vand >x01=q8,<x01=q14,<mask=q6
vand q8,q14,q6

# qhasm: 		2x r1 += t1
# asm 1: vadd.i64 >r1=reg128#4,<r1=reg128#4,<t1=reg128#14
# asm 2: vadd.i64 >r1=q3,<r1=q3,<t1=q13
vadd.i64 q3,q3,q13

# qhasm: 2x t4 = r3 unsigned>> 26
# asm 1: vshr.u64 >t4=reg128#14,<r3=reg128#5,#26
# asm 2: vshr.u64 >t4=q13,<r3=q4,#26
vshr.u64 q13,q4,#26

# qhasm: 						x01 = x01[0,2,1,3]
# asm 1: vtrn.32 <x01=reg128#9%bot,<x01=reg128#9%top
# asm 2: vtrn.32 <x01=d16,<x01=d17
vtrn.32 d16,d17

# qhasm:    r3 &= mask
# asm 1: vand >r3=reg128#5,<r3=reg128#5,<mask=reg128#7
# asm 2: vand >r3=q4,<r3=q4,<mask=q6
vand q4,q4,q6

# qhasm: 						r1 = r1[0,2,1,3]
# asm 1: vtrn.32 <r1=reg128#4%bot,<r1=reg128#4%top
# asm 2: vtrn.32 <r1=d6,<r1=d7
vtrn.32 d6,d7

# qhasm: 2x x4 += t4
# asm 1: vadd.i64 >x4=reg128#11,<x4=reg128#11,<t4=reg128#14
# asm 2: vadd.i64 >x4=q10,<x4=q10,<t4=q13
vadd.i64 q10,q10,q13

# qhasm: 						r3 = r3[0,2,1,3]
# asm 1: vtrn.32 <r3=reg128#5%bot,<r3=reg128#5%top
# asm 2: vtrn.32 <r3=d8,<r3=d9
vtrn.32 d8,d9

# qhasm: 						x01 = x01[0,1] r1[0,1]
# asm 1: vext.32 <x01=reg128#9%top,<r1=reg128#4%bot,<r1=reg128#4%bot,#0
# asm 2: vext.32 <x01=d17,<r1=d6,<r1=d6,#0
vext.32 d17,d6,d6,#0

# qhasm: 						x23 = x23[0,1] r3[0,1]
# asm 1: vext.32 <x23=reg128#10%top,<r3=reg128#5%bot,<r3=reg128#5%bot,#0
# asm 2: vext.32 <x23=d19,<r3=d8,<r3=d8,#0
vext.32 d19,d8,d8,#0

# qhasm: 						x4 = x4[0,2,1,3]
# asm 1: vtrn.32 <x4=reg128#11%bot,<x4=reg128#11%top
# asm 2: vtrn.32 <x4=d20,<x4=d21
vtrn.32 d20,d21

# qhasm:                   unsigned>? len - 64
# asm 1: cmp <len=int32#4,#64
# asm 2: cmp <len=r3,#64
cmp r3,#64

# qhasm: goto mainloop2 if unsigned>
bhi ._mainloop2

# qhasm: input_2 -= 32
# asm 1: sub >input_2=int32#3,<input_2=int32#2,#32
# asm 2: sub >input_2=r2,<input_2=r1,#32
sub r2,r1,#32

# qhasm: below64bytes:
._below64bytes:

# qhasm:              unsigned>? len - 32
# asm 1: cmp <len=int32#4,#32
# asm 2: cmp <len=r3,#32
cmp r3,#32

# qhasm: goto end if !unsigned>
bls ._end

# qhasm: mainloop:
._mainloop:

# qhasm:   new r0

# qhasm: ptr = &two24
# asm 1: lea >ptr=int32#2,<two24=stack128#1
# asm 2: lea >ptr=r1,<two24=[sp,#0]
add r1,sp,#0

# qhasm: r4 aligned= mem128[ptr]
# asm 1: vld1.8 {>r4=reg128#5%bot->r4=reg128#5%top},[<ptr=int32#2,: 128]
# asm 2: vld1.8 {>r4=d8->r4=d9},[<ptr=r1,: 128]
vld1.8 {d8-d9},[r1,: 128]

# qhasm: u4 aligned= mem128[ptr]
# asm 1: vld1.8 {>u4=reg128#6%bot->u4=reg128#6%top},[<ptr=int32#2,: 128]
# asm 2: vld1.8 {>u4=d10->u4=d11},[<ptr=r1,: 128]
vld1.8 {d10-d11},[r1,: 128]

# qhasm:   c01 = mem128[input_2];input_2+=16 
# asm 1: vld1.8 {>c01=reg128#8%bot->c01=reg128#8%top},[<input_2=int32#3]!
# asm 2: vld1.8 {>c01=d14->c01=d15},[<input_2=r2]!
vld1.8 {d14-d15},[r2]!

# qhasm: r4[0,1] += x01[0] unsigned*  y34[2];  r4[2,3] += x01[1] unsigned*  y34[3]
# asm 1: vmlal.u32 <r4=reg128#5,<x01=reg128#9%bot,<y34=reg128#3%top
# asm 2: vmlal.u32 <r4=q4,<x01=d16,<y34=d5
vmlal.u32 q4,d16,d5

# qhasm:   c23 = mem128[input_2];input_2+=16 
# asm 1: vld1.8 {>c23=reg128#14%bot->c23=reg128#14%top},[<input_2=int32#3]!
# asm 2: vld1.8 {>c23=d26->c23=d27},[<input_2=r2]!
vld1.8 {d26-d27},[r2]!

# qhasm: r4[0,1] += x01[2] unsigned* y34[0];  r4[2,3] += x01[3] unsigned* y34[1]
# asm 1: vmlal.u32 <r4=reg128#5,<x01=reg128#9%top,<y34=reg128#3%bot
# asm 2: vmlal.u32 <r4=q4,<x01=d17,<y34=d4
vmlal.u32 q4,d17,d4

# qhasm:   r0 = u4[1]c01[0]r0[2,3] 
# asm 1: vext.32 <r0=reg128#4%bot,<u4=reg128#6%bot,<c01=reg128#8%bot,#1
# asm 2: vext.32 <r0=d6,<u4=d10,<c01=d14,#1
vext.32 d6,d10,d14,#1

# qhasm: r4[0,1] += x23[0] unsigned* y12[2];  r4[2,3] += x23[1] unsigned* y12[3]
# asm 1: vmlal.u32 <r4=reg128#5,<x23=reg128#10%bot,<y12=reg128#2%top
# asm 2: vmlal.u32 <r4=q4,<x23=d18,<y12=d3
vmlal.u32 q4,d18,d3

# qhasm:   r0 = r0[0,1]u4[1]c23[0] 
# asm 1: vext.32 <r0=reg128#4%top,<u4=reg128#6%bot,<c23=reg128#14%bot,#1
# asm 2: vext.32 <r0=d7,<u4=d10,<c23=d26,#1
vext.32 d7,d10,d26,#1

# qhasm: r4[0,1] += x23[2] unsigned* y12[0];  r4[2,3] += x23[3] unsigned* y12[1]
# asm 1: vmlal.u32 <r4=reg128#5,<x23=reg128#10%top,<y12=reg128#2%bot
# asm 2: vmlal.u32 <r4=q4,<x23=d19,<y12=d2
vmlal.u32 q4,d19,d2

# qhasm:   r0 = r0[1]r0[0]r0[3]r0[2] 
# asm 1: vrev64.i32 >r0=reg128#4,<r0=reg128#4
# asm 2: vrev64.i32 >r0=q3,<r0=q3
vrev64.i32 q3,q3

# qhasm: r4[0,1] +=  x4[0] unsigned* y0[0];  r4[2,3] +=  x4[1] unsigned* y0[1]
# asm 1: vmlal.u32 <r4=reg128#5,<x4=reg128#11%bot,<y0=reg128#1%bot
# asm 2: vmlal.u32 <r4=q4,<x4=d20,<y0=d0
vmlal.u32 q4,d20,d0

# qhasm: r0[0,1] +=  x4[0] unsigned* 5y12[0]; r0[2,3] +=  x4[1] unsigned* 5y12[1]
# asm 1: vmlal.u32 <r0=reg128#4,<x4=reg128#11%bot,<5y12=reg128#12%bot
# asm 2: vmlal.u32 <r0=q3,<x4=d20,<5y12=d22
vmlal.u32 q3,d20,d22

# qhasm: r0[0,1] += x23[0] unsigned* 5y34[0]; r0[2,3] += x23[1] unsigned* 5y34[1]
# asm 1: vmlal.u32 <r0=reg128#4,<x23=reg128#10%bot,<5y34=reg128#13%bot
# asm 2: vmlal.u32 <r0=q3,<x23=d18,<5y34=d24
vmlal.u32 q3,d18,d24

# qhasm: r0[0,1] += x23[2] unsigned* 5y12[2]; r0[2,3] += x23[3] unsigned* 5y12[3]
# asm 1: vmlal.u32 <r0=reg128#4,<x23=reg128#10%top,<5y12=reg128#12%top
# asm 2: vmlal.u32 <r0=q3,<x23=d19,<5y12=d23
vmlal.u32 q3,d19,d23

# qhasm:   c01 c23 = c01[0]c23[0]c01[2]c23[2]c01[1]c23[1]c01[3]c23[3] 
# asm 1: vtrn.32 <c01=reg128#8,<c23=reg128#14
# asm 2: vtrn.32 <c01=q7,<c23=q13
vtrn.32 q7,q13

# qhasm: r0[0,1] += x01[0] unsigned* y0[0];   r0[2,3] += x01[1] unsigned* y0[1]
# asm 1: vmlal.u32 <r0=reg128#4,<x01=reg128#9%bot,<y0=reg128#1%bot
# asm 2: vmlal.u32 <r0=q3,<x01=d16,<y0=d0
vmlal.u32 q3,d16,d0

# qhasm:   r3[0,1] = c23[2]<<18; r3[2,3] = c23[3]<<18 
# asm 1: vshll.u32 >r3=reg128#6,<c23=reg128#14%top,#18
# asm 2: vshll.u32 >r3=q5,<c23=d27,#18
vshll.u32 q5,d27,#18

# qhasm: r0[0,1] += x01[2] unsigned*  5y34[2]; r0[2,3] += x01[3] unsigned*  5y34[3]
# asm 1: vmlal.u32 <r0=reg128#4,<x01=reg128#9%top,<5y34=reg128#13%top
# asm 2: vmlal.u32 <r0=q3,<x01=d17,<5y34=d25
vmlal.u32 q3,d17,d25

# qhasm: r3[0,1] += x01[0] unsigned* y34[0];   r3[2,3] += x01[1] unsigned* y34[1]
# asm 1: vmlal.u32 <r3=reg128#6,<x01=reg128#9%bot,<y34=reg128#3%bot
# asm 2: vmlal.u32 <r3=q5,<x01=d16,<y34=d4
vmlal.u32 q5,d16,d4

# qhasm: r3[0,1] += x01[2] unsigned* y12[2];   r3[2,3] += x01[3] unsigned* y12[3]
# asm 1: vmlal.u32 <r3=reg128#6,<x01=reg128#9%top,<y12=reg128#2%top
# asm 2: vmlal.u32 <r3=q5,<x01=d17,<y12=d3
vmlal.u32 q5,d17,d3

# qhasm: r3[0,1] += x23[0] unsigned* y12[0];   r3[2,3] += x23[1] unsigned* y12[1]
# asm 1: vmlal.u32 <r3=reg128#6,<x23=reg128#10%bot,<y12=reg128#2%bot
# asm 2: vmlal.u32 <r3=q5,<x23=d18,<y12=d2
vmlal.u32 q5,d18,d2

# qhasm: r3[0,1] += x23[2] unsigned* y0[0];   r3[2,3] += x23[3] unsigned* y0[1]
# asm 1: vmlal.u32 <r3=reg128#6,<x23=reg128#10%top,<y0=reg128#1%bot
# asm 2: vmlal.u32 <r3=q5,<x23=d19,<y0=d0
vmlal.u32 q5,d19,d0

# qhasm:   r1[0,1] = c23[0]<<6; r1[2,3] = c23[1]<<6 
# asm 1: vshll.u32 >r1=reg128#14,<c23=reg128#14%bot,#6
# asm 2: vshll.u32 >r1=q13,<c23=d26,#6
vshll.u32 q13,d26,#6

# qhasm: r3[0,1] +=  x4[0] unsigned*  5y34[2]; r3[2,3] +=  x4[1] unsigned*  5y34[3]
# asm 1: vmlal.u32 <r3=reg128#6,<x4=reg128#11%bot,<5y34=reg128#13%top
# asm 2: vmlal.u32 <r3=q5,<x4=d20,<5y34=d25
vmlal.u32 q5,d20,d25

# qhasm: r1[0,1] += x01[0] unsigned* y12[0];   r1[2,3] += x01[1] unsigned* y12[1]
# asm 1: vmlal.u32 <r1=reg128#14,<x01=reg128#9%bot,<y12=reg128#2%bot
# asm 2: vmlal.u32 <r1=q13,<x01=d16,<y12=d2
vmlal.u32 q13,d16,d2

# qhasm: r1[0,1] += x01[2] unsigned* y0[0];   r1[2,3] += x01[3] unsigned* y0[1]
# asm 1: vmlal.u32 <r1=reg128#14,<x01=reg128#9%top,<y0=reg128#1%bot
# asm 2: vmlal.u32 <r1=q13,<x01=d17,<y0=d0
vmlal.u32 q13,d17,d0

# qhasm: r1[0,1] += x23[0] unsigned*  5y34[2]; r1[2,3] += x23[1] unsigned*  5y34[3]
# asm 1: vmlal.u32 <r1=reg128#14,<x23=reg128#10%bot,<5y34=reg128#13%top
# asm 2: vmlal.u32 <r1=q13,<x23=d18,<5y34=d25
vmlal.u32 q13,d18,d25

# qhasm: r1[0,1] += x23[2] unsigned* 5y34[0]; r1[2,3] += x23[3] unsigned* 5y34[1]
# asm 1: vmlal.u32 <r1=reg128#14,<x23=reg128#10%top,<5y34=reg128#13%bot
# asm 2: vmlal.u32 <r1=q13,<x23=d19,<5y34=d24
vmlal.u32 q13,d19,d24

# qhasm:   r2[0,1] = c01[2]<<12; r2[2,3] = c01[3]<<12 
# asm 1: vshll.u32 >r2=reg128#8,<c01=reg128#8%top,#12
# asm 2: vshll.u32 >r2=q7,<c01=d15,#12
vshll.u32 q7,d15,#12

# qhasm: r1[0,1] +=  x4[0] unsigned* 5y12[2]; r1[2,3] +=  x4[1] unsigned* 5y12[3]
# asm 1: vmlal.u32 <r1=reg128#14,<x4=reg128#11%bot,<5y12=reg128#12%top
# asm 2: vmlal.u32 <r1=q13,<x4=d20,<5y12=d23
vmlal.u32 q13,d20,d23

# qhasm: r2[0,1] += x01[0] unsigned* y12[2];   r2[2,3] += x01[1] unsigned* y12[3]
# asm 1: vmlal.u32 <r2=reg128#8,<x01=reg128#9%bot,<y12=reg128#2%top
# asm 2: vmlal.u32 <r2=q7,<x01=d16,<y12=d3
vmlal.u32 q7,d16,d3

# qhasm: r2[0,1] += x01[2] unsigned* y12[0];   r2[2,3] += x01[3] unsigned* y12[1]
# asm 1: vmlal.u32 <r2=reg128#8,<x01=reg128#9%top,<y12=reg128#2%bot
# asm 2: vmlal.u32 <r2=q7,<x01=d17,<y12=d2
vmlal.u32 q7,d17,d2

# qhasm: r2[0,1] += x23[0] unsigned* y0[0];   r2[2,3] += x23[1] unsigned* y0[1]
# asm 1: vmlal.u32 <r2=reg128#8,<x23=reg128#10%bot,<y0=reg128#1%bot
# asm 2: vmlal.u32 <r2=q7,<x23=d18,<y0=d0
vmlal.u32 q7,d18,d0

# qhasm: r2[0,1] += x23[2] unsigned*  5y34[2]; r2[2,3] += x23[3] unsigned*  5y34[3]
# asm 1: vmlal.u32 <r2=reg128#8,<x23=reg128#10%top,<5y34=reg128#13%top
# asm 2: vmlal.u32 <r2=q7,<x23=d19,<5y34=d25
vmlal.u32 q7,d19,d25

# qhasm: r2[0,1] +=  x4[0] unsigned* 5y34[0]; r2[2,3] +=  x4[1] unsigned* 5y34[1]
# asm 1: vmlal.u32 <r2=reg128#8,<x4=reg128#11%bot,<5y34=reg128#13%bot
# asm 2: vmlal.u32 <r2=q7,<x4=d20,<5y34=d24
vmlal.u32 q7,d20,d24

# qhasm: 2x t1 = r0 unsigned>> 26
# asm 1: vshr.u64 >t1=reg128#9,<r0=reg128#4,#26
# asm 2: vshr.u64 >t1=q8,<r0=q3,#26
vshr.u64 q8,q3,#26

# qhasm:    r0 &= mask
# asm 1: vand >r0=reg128#4,<r0=reg128#4,<mask=reg128#7
# asm 2: vand >r0=q3,<r0=q3,<mask=q6
vand q3,q3,q6

# qhasm: 2x r1 += t1
# asm 1: vadd.i64 >r1=reg128#9,<r1=reg128#14,<t1=reg128#9
# asm 2: vadd.i64 >r1=q8,<r1=q13,<t1=q8
vadd.i64 q8,q13,q8

# qhasm: 		2x t4 = r3 unsigned>> 26
# asm 1: vshr.u64 >t4=reg128#10,<r3=reg128#6,#26
# asm 2: vshr.u64 >t4=q9,<r3=q5,#26
vshr.u64 q9,q5,#26

# qhasm: 		   r3 &= mask
# asm 1: vand >r3=reg128#6,<r3=reg128#6,<mask=reg128#7
# asm 2: vand >r3=q5,<r3=q5,<mask=q6
vand q5,q5,q6

# qhasm: 		2x r4 += t4
# asm 1: vadd.i64 >r4=reg128#5,<r4=reg128#5,<t4=reg128#10
# asm 2: vadd.i64 >r4=q4,<r4=q4,<t4=q9
vadd.i64 q4,q4,q9

# qhasm: 2x t2 = r1 unsigned>> 26
# asm 1: vshr.u64 >t2=reg128#10,<r1=reg128#9,#26
# asm 2: vshr.u64 >t2=q9,<r1=q8,#26
vshr.u64 q9,q8,#26

# qhasm:    r1 &= mask
# asm 1: vand >r1=reg128#11,<r1=reg128#9,<mask=reg128#7
# asm 2: vand >r1=q10,<r1=q8,<mask=q6
vand q10,q8,q6

# qhasm: 		2x t0 = r4 unsigned>> 26
# asm 1: vshr.u64 >t0=reg128#9,<r4=reg128#5,#26
# asm 2: vshr.u64 >t0=q8,<r4=q4,#26
vshr.u64 q8,q4,#26

# qhasm: 2x r2 += t2
# asm 1: vadd.i64 >r2=reg128#8,<r2=reg128#8,<t2=reg128#10
# asm 2: vadd.i64 >r2=q7,<r2=q7,<t2=q9
vadd.i64 q7,q7,q9

# qhasm: 		   r4 &= mask
# asm 1: vand >r4=reg128#5,<r4=reg128#5,<mask=reg128#7
# asm 2: vand >r4=q4,<r4=q4,<mask=q6
vand q4,q4,q6

# qhasm: 		2x r0 += t0
# asm 1: vadd.i64 >r0=reg128#4,<r0=reg128#4,<t0=reg128#9
# asm 2: vadd.i64 >r0=q3,<r0=q3,<t0=q8
vadd.i64 q3,q3,q8

# qhasm: 		2x t0 <<= 2
# asm 1: vshl.i64 >t0=reg128#9,<t0=reg128#9,#2
# asm 2: vshl.i64 >t0=q8,<t0=q8,#2
vshl.i64 q8,q8,#2

# qhasm: 2x t3 = r2 unsigned>> 26
# asm 1: vshr.u64 >t3=reg128#14,<r2=reg128#8,#26
# asm 2: vshr.u64 >t3=q13,<r2=q7,#26
vshr.u64 q13,q7,#26

# qhasm: 		2x r0 += t0
# asm 1: vadd.i64 >r0=reg128#4,<r0=reg128#4,<t0=reg128#9
# asm 2: vadd.i64 >r0=q3,<r0=q3,<t0=q8
vadd.i64 q3,q3,q8

# qhasm:    x23 = r2 & mask
# asm 1: vand >x23=reg128#10,<r2=reg128#8,<mask=reg128#7
# asm 2: vand >x23=q9,<r2=q7,<mask=q6
vand q9,q7,q6

# qhasm: 2x r3 += t3
# asm 1: vadd.i64 >r3=reg128#6,<r3=reg128#6,<t3=reg128#14
# asm 2: vadd.i64 >r3=q5,<r3=q5,<t3=q13
vadd.i64 q5,q5,q13

# qhasm: 		2x t1 = r0 unsigned>> 26
# asm 1: vshr.u64 >t1=reg128#8,<r0=reg128#4,#26
# asm 2: vshr.u64 >t1=q7,<r0=q3,#26
vshr.u64 q7,q3,#26

# qhasm: 		   x01 = r0 & mask
# asm 1: vand >x01=reg128#9,<r0=reg128#4,<mask=reg128#7
# asm 2: vand >x01=q8,<r0=q3,<mask=q6
vand q8,q3,q6

# qhasm: 		2x r1 += t1
# asm 1: vadd.i64 >r1=reg128#4,<r1=reg128#11,<t1=reg128#8
# asm 2: vadd.i64 >r1=q3,<r1=q10,<t1=q7
vadd.i64 q3,q10,q7

# qhasm: 2x t4 = r3 unsigned>> 26
# asm 1: vshr.u64 >t4=reg128#8,<r3=reg128#6,#26
# asm 2: vshr.u64 >t4=q7,<r3=q5,#26
vshr.u64 q7,q5,#26

# qhasm:    r3 &= mask
# asm 1: vand >r3=reg128#6,<r3=reg128#6,<mask=reg128#7
# asm 2: vand >r3=q5,<r3=q5,<mask=q6
vand q5,q5,q6

# qhasm: 2x x4 = r4 + t4
# asm 1: vadd.i64 >x4=reg128#11,<r4=reg128#5,<t4=reg128#8
# asm 2: vadd.i64 >x4=q10,<r4=q4,<t4=q7
vadd.i64 q10,q4,q7

# qhasm:   len -= 32
# asm 1: sub >len=int32#4,<len=int32#4,#32
# asm 2: sub >len=r3,<len=r3,#32
sub r3,r3,#32

# qhasm: x01 = x01[0,2,1,3]
# asm 1: vtrn.32 <x01=reg128#9%bot,<x01=reg128#9%top
# asm 2: vtrn.32 <x01=d16,<x01=d17
vtrn.32 d16,d17

# qhasm: x23 = x23[0,2,1,3]
# asm 1: vtrn.32 <x23=reg128#10%bot,<x23=reg128#10%top
# asm 2: vtrn.32 <x23=d18,<x23=d19
vtrn.32 d18,d19

# qhasm: r1 = r1[0,2,1,3]
# asm 1: vtrn.32 <r1=reg128#4%bot,<r1=reg128#4%top
# asm 2: vtrn.32 <r1=d6,<r1=d7
vtrn.32 d6,d7

# qhasm: r3 = r3[0,2,1,3]
# asm 1: vtrn.32 <r3=reg128#6%bot,<r3=reg128#6%top
# asm 2: vtrn.32 <r3=d10,<r3=d11
vtrn.32 d10,d11

# qhasm: x4 = x4[0,2,1,3]
# asm 1: vtrn.32 <x4=reg128#11%bot,<x4=reg128#11%top
# asm 2: vtrn.32 <x4=d20,<x4=d21
vtrn.32 d20,d21

# qhasm: x01 = x01[0,1] r1[0,1]
# asm 1: vext.32 <x01=reg128#9%top,<r1=reg128#4%bot,<r1=reg128#4%bot,#0
# asm 2: vext.32 <x01=d17,<r1=d6,<r1=d6,#0
vext.32 d17,d6,d6,#0

# qhasm: x23 = x23[0,1] r3[0,1]
# asm 1: vext.32 <x23=reg128#10%top,<r3=reg128#6%bot,<r3=reg128#6%bot,#0
# asm 2: vext.32 <x23=d19,<r3=d10,<r3=d10,#0
vext.32 d19,d10,d10,#0

# qhasm: unsigned>? len - 32
# asm 1: cmp <len=int32#4,#32
# asm 2: cmp <len=r3,#32
cmp r3,#32

# qhasm: goto mainloop if unsigned>
bhi ._mainloop

# qhasm: end:
._end:

# qhasm: mem128[input_0] = x01;input_0+=16
# asm 1: vst1.8 {<x01=reg128#9%bot-<x01=reg128#9%top},[<input_0=int32#1]!
# asm 2: vst1.8 {<x01=d16-<x01=d17},[<input_0=r0]!
vst1.8 {d16-d17},[r0]!

# qhasm: mem128[input_0] = x23;input_0+=16
# asm 1: vst1.8 {<x23=reg128#10%bot-<x23=reg128#10%top},[<input_0=int32#1]!
# asm 2: vst1.8 {<x23=d18-<x23=d19},[<input_0=r0]!
vst1.8 {d18-d19},[r0]!

# qhasm: mem64[input_0] = x4[0]
# asm 1: vst1.8 <x4=reg128#11%bot,[<input_0=int32#1]
# asm 2: vst1.8 <x4=d20,[<input_0=r0]
vst1.8 d20,[r0]

# qhasm: len = len
# asm 1: mov >len=int32#1,<len=int32#4
# asm 2: mov >len=r0,<len=r3
mov r0,r3

# qhasm: qpopreturn len
mov sp,r12
vpop {q4,q5,q6,q7}
bx lr

# qhasm: int32 input_0

# qhasm: int32 input_1

# qhasm: int32 input_2

# qhasm: int32 input_3

# qhasm: stack32 input_4

# qhasm: stack32 input_5

# qhasm: stack32 input_6

# qhasm: stack32 input_7

# qhasm: int32 caller_r4

# qhasm: int32 caller_r5

# qhasm: int32 caller_r6

# qhasm: int32 caller_r7

# qhasm: int32 caller_r8

# qhasm: int32 caller_r9

# qhasm: int32 caller_r10

# qhasm: int32 caller_r11

# qhasm: int32 caller_r12

# qhasm: int32 caller_r14

# qhasm: reg128 caller_q4

# qhasm: reg128 caller_q5

# qhasm: reg128 caller_q6

# qhasm: reg128 caller_q7

# qhasm: reg128 r0

# qhasm: reg128 r1

# qhasm: reg128 r2

# qhasm: reg128 r3

# qhasm: reg128 r4

# qhasm: reg128 x01

# qhasm: reg128 x23

# qhasm: reg128 x4

# qhasm: reg128 y01

# qhasm: reg128 y23

# qhasm: reg128 y4

# qhasm: reg128 _5y01

# qhasm: reg128 _5y23

# qhasm: reg128 _5y4

# qhasm: reg128 c01

# qhasm: reg128 c23

# qhasm: reg128 c4

# qhasm: reg128 t0

# qhasm: reg128 t1

# qhasm: reg128 t2

# qhasm: reg128 t3

# qhasm: reg128 t4

# qhasm: reg128 mask

# qhasm: enter crypto_onetimeauth_poly1305_neon2_addmulmod
.align 2
.global openssl_poly1305_neon2_addmulmod
.hidden openssl_poly1305_neon2_addmulmod
.type openssl_poly1305_neon2_addmulmod STT_FUNC
openssl_poly1305_neon2_addmulmod:
sub sp,sp,#0

# qhasm: 				2x mask = 0xffffffff
# asm 1: vmov.i64 >mask=reg128#1,#0xffffffff
# asm 2: vmov.i64 >mask=q0,#0xffffffff
vmov.i64 q0,#0xffffffff

# qhasm:   y01 aligned= mem128[input_2];input_2+=16
# asm 1: vld1.8 {>y01=reg128#2%bot->y01=reg128#2%top},[<input_2=int32#3,: 128]!
# asm 2: vld1.8 {>y01=d2->y01=d3},[<input_2=r2,: 128]!
vld1.8 {d2-d3},[r2,: 128]!

# qhasm: 4x _5y01 = y01 << 2
# asm 1: vshl.i32 >_5y01=reg128#3,<y01=reg128#2,#2
# asm 2: vshl.i32 >_5y01=q2,<y01=q1,#2
vshl.i32 q2,q1,#2

# qhasm:   y23 aligned= mem128[input_2];input_2+=16
# asm 1: vld1.8 {>y23=reg128#4%bot->y23=reg128#4%top},[<input_2=int32#3,: 128]!
# asm 2: vld1.8 {>y23=d6->y23=d7},[<input_2=r2,: 128]!
vld1.8 {d6-d7},[r2,: 128]!

# qhasm: 4x _5y23 = y23 << 2
# asm 1: vshl.i32 >_5y23=reg128#9,<y23=reg128#4,#2
# asm 2: vshl.i32 >_5y23=q8,<y23=q3,#2
vshl.i32 q8,q3,#2

# qhasm:   y4  aligned= mem64[input_2]y4[1]
# asm 1: vld1.8 {<y4=reg128#10%bot},[<input_2=int32#3,: 64]
# asm 2: vld1.8 {<y4=d18},[<input_2=r2,: 64]
vld1.8 {d18},[r2,: 64]

# qhasm: 4x _5y4 = y4 << 2
# asm 1: vshl.i32 >_5y4=reg128#11,<y4=reg128#10,#2
# asm 2: vshl.i32 >_5y4=q10,<y4=q9,#2
vshl.i32 q10,q9,#2

# qhasm:   x01 aligned= mem128[input_1];input_1+=16
# asm 1: vld1.8 {>x01=reg128#12%bot->x01=reg128#12%top},[<input_1=int32#2,: 128]!
# asm 2: vld1.8 {>x01=d22->x01=d23},[<input_1=r1,: 128]!
vld1.8 {d22-d23},[r1,: 128]!

# qhasm: 4x _5y01 += y01
# asm 1: vadd.i32 >_5y01=reg128#3,<_5y01=reg128#3,<y01=reg128#2
# asm 2: vadd.i32 >_5y01=q2,<_5y01=q2,<y01=q1
vadd.i32 q2,q2,q1

# qhasm:   x23 aligned= mem128[input_1];input_1+=16
# asm 1: vld1.8 {>x23=reg128#13%bot->x23=reg128#13%top},[<input_1=int32#2,: 128]!
# asm 2: vld1.8 {>x23=d24->x23=d25},[<input_1=r1,: 128]!
vld1.8 {d24-d25},[r1,: 128]!

# qhasm: 4x _5y23 += y23
# asm 1: vadd.i32 >_5y23=reg128#9,<_5y23=reg128#9,<y23=reg128#4
# asm 2: vadd.i32 >_5y23=q8,<_5y23=q8,<y23=q3
vadd.i32 q8,q8,q3

# qhasm: 4x _5y4 += y4
# asm 1: vadd.i32 >_5y4=reg128#11,<_5y4=reg128#11,<y4=reg128#10
# asm 2: vadd.i32 >_5y4=q10,<_5y4=q10,<y4=q9
vadd.i32 q10,q10,q9

# qhasm:   c01 aligned= mem128[input_3];input_3+=16
# asm 1: vld1.8 {>c01=reg128#14%bot->c01=reg128#14%top},[<input_3=int32#4,: 128]!
# asm 2: vld1.8 {>c01=d26->c01=d27},[<input_3=r3,: 128]!
vld1.8 {d26-d27},[r3,: 128]!

# qhasm: 4x x01 += c01
# asm 1: vadd.i32 >x01=reg128#12,<x01=reg128#12,<c01=reg128#14
# asm 2: vadd.i32 >x01=q11,<x01=q11,<c01=q13
vadd.i32 q11,q11,q13

# qhasm:   c23 aligned= mem128[input_3];input_3+=16
# asm 1: vld1.8 {>c23=reg128#14%bot->c23=reg128#14%top},[<input_3=int32#4,: 128]!
# asm 2: vld1.8 {>c23=d26->c23=d27},[<input_3=r3,: 128]!
vld1.8 {d26-d27},[r3,: 128]!

# qhasm: 4x x23 += c23
# asm 1: vadd.i32 >x23=reg128#13,<x23=reg128#13,<c23=reg128#14
# asm 2: vadd.i32 >x23=q12,<x23=q12,<c23=q13
vadd.i32 q12,q12,q13

# qhasm:   x4  aligned= mem64[input_1]x4[1]
# asm 1: vld1.8 {<x4=reg128#14%bot},[<input_1=int32#2,: 64]
# asm 2: vld1.8 {<x4=d26},[<input_1=r1,: 64]
vld1.8 {d26},[r1,: 64]

# qhasm: 				2x mask unsigned>>=6
# asm 1: vshr.u64 >mask=reg128#1,<mask=reg128#1,#6
# asm 2: vshr.u64 >mask=q0,<mask=q0,#6
vshr.u64 q0,q0,#6

# qhasm:   c4  aligned= mem64[input_3]c4[1]
# asm 1: vld1.8 {<c4=reg128#15%bot},[<input_3=int32#4,: 64]
# asm 2: vld1.8 {<c4=d28},[<input_3=r3,: 64]
vld1.8 {d28},[r3,: 64]

# qhasm: 4x x4 += c4
# asm 1: vadd.i32 >x4=reg128#14,<x4=reg128#14,<c4=reg128#15
# asm 2: vadd.i32 >x4=q13,<x4=q13,<c4=q14
vadd.i32 q13,q13,q14

# qhasm: r0[0,1]  = x01[0] unsigned* y01[0];   r0[2,3]  = x01[1] unsigned* y01[1]
# asm 1: vmull.u32 >r0=reg128#15,<x01=reg128#12%bot,<y01=reg128#2%bot
# asm 2: vmull.u32 >r0=q14,<x01=d22,<y01=d2
vmull.u32 q14,d22,d2

# qhasm: r0[0,1] += x01[2] unsigned*  _5y4[0]; r0[2,3] += x01[3] unsigned*  _5y4[1]
# asm 1: vmlal.u32 <r0=reg128#15,<x01=reg128#12%top,<_5y4=reg128#11%bot
# asm 2: vmlal.u32 <r0=q14,<x01=d23,<_5y4=d20
vmlal.u32 q14,d23,d20

# qhasm: r0[0,1] += x23[0] unsigned* _5y23[2]; r0[2,3] += x23[1] unsigned* _5y23[3]
# asm 1: vmlal.u32 <r0=reg128#15,<x23=reg128#13%bot,<_5y23=reg128#9%top
# asm 2: vmlal.u32 <r0=q14,<x23=d24,<_5y23=d17
vmlal.u32 q14,d24,d17

# qhasm: r0[0,1] += x23[2] unsigned* _5y23[0]; r0[2,3] += x23[3] unsigned* _5y23[1]
# asm 1: vmlal.u32 <r0=reg128#15,<x23=reg128#13%top,<_5y23=reg128#9%bot
# asm 2: vmlal.u32 <r0=q14,<x23=d25,<_5y23=d16
vmlal.u32 q14,d25,d16

# qhasm: r0[0,1] +=  x4[0] unsigned* _5y01[2]; r0[2,3] +=  x4[1] unsigned* _5y01[3]
# asm 1: vmlal.u32 <r0=reg128#15,<x4=reg128#14%bot,<_5y01=reg128#3%top
# asm 2: vmlal.u32 <r0=q14,<x4=d26,<_5y01=d5
vmlal.u32 q14,d26,d5

# qhasm: r1[0,1]  = x01[0] unsigned* y01[2];   r1[2,3]  = x01[1] unsigned* y01[3]
# asm 1: vmull.u32 >r1=reg128#3,<x01=reg128#12%bot,<y01=reg128#2%top
# asm 2: vmull.u32 >r1=q2,<x01=d22,<y01=d3
vmull.u32 q2,d22,d3

# qhasm: r1[0,1] += x01[2] unsigned* y01[0];   r1[2,3] += x01[3] unsigned* y01[1]
# asm 1: vmlal.u32 <r1=reg128#3,<x01=reg128#12%top,<y01=reg128#2%bot
# asm 2: vmlal.u32 <r1=q2,<x01=d23,<y01=d2
vmlal.u32 q2,d23,d2

# qhasm: r1[0,1] += x23[0] unsigned*  _5y4[0]; r1[2,3] += x23[1] unsigned*  _5y4[1]
# asm 1: vmlal.u32 <r1=reg128#3,<x23=reg128#13%bot,<_5y4=reg128#11%bot
# asm 2: vmlal.u32 <r1=q2,<x23=d24,<_5y4=d20
vmlal.u32 q2,d24,d20

# qhasm: r1[0,1] += x23[2] unsigned* _5y23[2]; r1[2,3] += x23[3] unsigned* _5y23[3]
# asm 1: vmlal.u32 <r1=reg128#3,<x23=reg128#13%top,<_5y23=reg128#9%top
# asm 2: vmlal.u32 <r1=q2,<x23=d25,<_5y23=d17
vmlal.u32 q2,d25,d17

# qhasm: r1[0,1] +=  x4[0] unsigned* _5y23[0]; r1[2,3] +=  x4[1] unsigned* _5y23[1]
# asm 1: vmlal.u32 <r1=reg128#3,<x4=reg128#14%bot,<_5y23=reg128#9%bot
# asm 2: vmlal.u32 <r1=q2,<x4=d26,<_5y23=d16
vmlal.u32 q2,d26,d16

# qhasm: r2[0,1]  = x01[0] unsigned* y23[0];   r2[2,3]  = x01[1] unsigned* y23[1]
# asm 1: vmull.u32 >r2=reg128#16,<x01=reg128#12%bot,<y23=reg128#4%bot
# asm 2: vmull.u32 >r2=q15,<x01=d22,<y23=d6
vmull.u32 q15,d22,d6

# qhasm: r2[0,1] += x01[2] unsigned* y01[2];   r2[2,3] += x01[3] unsigned* y01[3]
# asm 1: vmlal.u32 <r2=reg128#16,<x01=reg128#12%top,<y01=reg128#2%top
# asm 2: vmlal.u32 <r2=q15,<x01=d23,<y01=d3
vmlal.u32 q15,d23,d3

# qhasm: r2[0,1] += x23[0] unsigned* y01[0];   r2[2,3] += x23[1] unsigned* y01[1]
# asm 1: vmlal.u32 <r2=reg128#16,<x23=reg128#13%bot,<y01=reg128#2%bot
# asm 2: vmlal.u32 <r2=q15,<x23=d24,<y01=d2
vmlal.u32 q15,d24,d2

# qhasm: r2[0,1] += x23[2] unsigned*  _5y4[0]; r2[2,3] += x23[3] unsigned*  _5y4[1]
# asm 1: vmlal.u32 <r2=reg128#16,<x23=reg128#13%top,<_5y4=reg128#11%bot
# asm 2: vmlal.u32 <r2=q15,<x23=d25,<_5y4=d20
vmlal.u32 q15,d25,d20

# qhasm: r2[0,1] +=  x4[0] unsigned* _5y23[2]; r2[2,3] +=  x4[1] unsigned* _5y23[3]
# asm 1: vmlal.u32 <r2=reg128#16,<x4=reg128#14%bot,<_5y23=reg128#9%top
# asm 2: vmlal.u32 <r2=q15,<x4=d26,<_5y23=d17
vmlal.u32 q15,d26,d17

# qhasm: r3[0,1]  = x01[0] unsigned* y23[2];   r3[2,3]  = x01[1] unsigned* y23[3]
# asm 1: vmull.u32 >r3=reg128#9,<x01=reg128#12%bot,<y23=reg128#4%top
# asm 2: vmull.u32 >r3=q8,<x01=d22,<y23=d7
vmull.u32 q8,d22,d7

# qhasm: r3[0,1] += x01[2] unsigned* y23[0];   r3[2,3] += x01[3] unsigned* y23[1]
# asm 1: vmlal.u32 <r3=reg128#9,<x01=reg128#12%top,<y23=reg128#4%bot
# asm 2: vmlal.u32 <r3=q8,<x01=d23,<y23=d6
vmlal.u32 q8,d23,d6

# qhasm: r3[0,1] += x23[0] unsigned* y01[2];   r3[2,3] += x23[1] unsigned* y01[3]
# asm 1: vmlal.u32 <r3=reg128#9,<x23=reg128#13%bot,<y01=reg128#2%top
# asm 2: vmlal.u32 <r3=q8,<x23=d24,<y01=d3
vmlal.u32 q8,d24,d3

# qhasm: r3[0,1] += x23[2] unsigned* y01[0];   r3[2,3] += x23[3] unsigned* y01[1]
# asm 1: vmlal.u32 <r3=reg128#9,<x23=reg128#13%top,<y01=reg128#2%bot
# asm 2: vmlal.u32 <r3=q8,<x23=d25,<y01=d2
vmlal.u32 q8,d25,d2

# qhasm: r3[0,1] +=  x4[0] unsigned*  _5y4[0]; r3[2,3] +=  x4[1] unsigned*  _5y4[1]
# asm 1: vmlal.u32 <r3=reg128#9,<x4=reg128#14%bot,<_5y4=reg128#11%bot
# asm 2: vmlal.u32 <r3=q8,<x4=d26,<_5y4=d20
vmlal.u32 q8,d26,d20

# qhasm: r4[0,1]  = x01[0] unsigned*  y4[0];  r4[2,3]  = x01[1] unsigned*  y4[1]
# asm 1: vmull.u32 >r4=reg128#10,<x01=reg128#12%bot,<y4=reg128#10%bot
# asm 2: vmull.u32 >r4=q9,<x01=d22,<y4=d18
vmull.u32 q9,d22,d18

# qhasm: r4[0,1] += x01[2] unsigned* y23[2];  r4[2,3] += x01[3] unsigned* y23[3]
# asm 1: vmlal.u32 <r4=reg128#10,<x01=reg128#12%top,<y23=reg128#4%top
# asm 2: vmlal.u32 <r4=q9,<x01=d23,<y23=d7
vmlal.u32 q9,d23,d7

# qhasm: r4[0,1] += x23[0] unsigned* y23[0];  r4[2,3] += x23[1] unsigned* y23[1]
# asm 1: vmlal.u32 <r4=reg128#10,<x23=reg128#13%bot,<y23=reg128#4%bot
# asm 2: vmlal.u32 <r4=q9,<x23=d24,<y23=d6
vmlal.u32 q9,d24,d6

# qhasm: r4[0,1] += x23[2] unsigned* y01[2];  r4[2,3] += x23[3] unsigned* y01[3]
# asm 1: vmlal.u32 <r4=reg128#10,<x23=reg128#13%top,<y01=reg128#2%top
# asm 2: vmlal.u32 <r4=q9,<x23=d25,<y01=d3
vmlal.u32 q9,d25,d3

# qhasm: r4[0,1] +=  x4[0] unsigned* y01[0];  r4[2,3] +=  x4[1] unsigned* y01[1]
# asm 1: vmlal.u32 <r4=reg128#10,<x4=reg128#14%bot,<y01=reg128#2%bot
# asm 2: vmlal.u32 <r4=q9,<x4=d26,<y01=d2
vmlal.u32 q9,d26,d2

# qhasm: 2x t1 = r0 unsigned>> 26
# asm 1: vshr.u64 >t1=reg128#2,<r0=reg128#15,#26
# asm 2: vshr.u64 >t1=q1,<r0=q14,#26
vshr.u64 q1,q14,#26

# qhasm:    r0 &= mask
# asm 1: vand >r0=reg128#4,<r0=reg128#15,<mask=reg128#1
# asm 2: vand >r0=q3,<r0=q14,<mask=q0
vand q3,q14,q0

# qhasm: 2x r1 += t1
# asm 1: vadd.i64 >r1=reg128#2,<r1=reg128#3,<t1=reg128#2
# asm 2: vadd.i64 >r1=q1,<r1=q2,<t1=q1
vadd.i64 q1,q2,q1

# qhasm:                 2x t4 = r3 unsigned>> 26
# asm 1: vshr.u64 >t4=reg128#3,<r3=reg128#9,#26
# asm 2: vshr.u64 >t4=q2,<r3=q8,#26
vshr.u64 q2,q8,#26

# qhasm:                    r3 &= mask
# asm 1: vand >r3=reg128#9,<r3=reg128#9,<mask=reg128#1
# asm 2: vand >r3=q8,<r3=q8,<mask=q0
vand q8,q8,q0

# qhasm:                 2x r4 += t4
# asm 1: vadd.i64 >r4=reg128#3,<r4=reg128#10,<t4=reg128#3
# asm 2: vadd.i64 >r4=q2,<r4=q9,<t4=q2
vadd.i64 q2,q9,q2

# qhasm: 2x t2 = r1 unsigned>> 26
# asm 1: vshr.u64 >t2=reg128#10,<r1=reg128#2,#26
# asm 2: vshr.u64 >t2=q9,<r1=q1,#26
vshr.u64 q9,q1,#26

# qhasm:    r1 &= mask
# asm 1: vand >r1=reg128#2,<r1=reg128#2,<mask=reg128#1
# asm 2: vand >r1=q1,<r1=q1,<mask=q0
vand q1,q1,q0

# qhasm:                 2x t0 = r4 unsigned>> 26
# asm 1: vshr.u64 >t0=reg128#11,<r4=reg128#3,#26
# asm 2: vshr.u64 >t0=q10,<r4=q2,#26
vshr.u64 q10,q2,#26

# qhasm: 2x r2 += t2
# asm 1: vadd.i64 >r2=reg128#10,<r2=reg128#16,<t2=reg128#10
# asm 2: vadd.i64 >r2=q9,<r2=q15,<t2=q9
vadd.i64 q9,q15,q9

# qhasm:                    r4 &= mask
# asm 1: vand >r4=reg128#3,<r4=reg128#3,<mask=reg128#1
# asm 2: vand >r4=q2,<r4=q2,<mask=q0
vand q2,q2,q0

# qhasm:                 2x r0 += t0
# asm 1: vadd.i64 >r0=reg128#4,<r0=reg128#4,<t0=reg128#11
# asm 2: vadd.i64 >r0=q3,<r0=q3,<t0=q10
vadd.i64 q3,q3,q10

# qhasm:                 2x t0 <<= 2
# asm 1: vshl.i64 >t0=reg128#11,<t0=reg128#11,#2
# asm 2: vshl.i64 >t0=q10,<t0=q10,#2
vshl.i64 q10,q10,#2

# qhasm: 2x t3 = r2 unsigned>> 26
# asm 1: vshr.u64 >t3=reg128#12,<r2=reg128#10,#26
# asm 2: vshr.u64 >t3=q11,<r2=q9,#26
vshr.u64 q11,q9,#26

# qhasm:                 2x r0 += t0
# asm 1: vadd.i64 >r0=reg128#4,<r0=reg128#4,<t0=reg128#11
# asm 2: vadd.i64 >r0=q3,<r0=q3,<t0=q10
vadd.i64 q3,q3,q10

# qhasm:    x23 = r2 & mask
# asm 1: vand >x23=reg128#10,<r2=reg128#10,<mask=reg128#1
# asm 2: vand >x23=q9,<r2=q9,<mask=q0
vand q9,q9,q0

# qhasm: 2x r3 += t3
# asm 1: vadd.i64 >r3=reg128#9,<r3=reg128#9,<t3=reg128#12
# asm 2: vadd.i64 >r3=q8,<r3=q8,<t3=q11
vadd.i64 q8,q8,q11

# qhasm:                 2x t1 = r0 unsigned>> 26
# asm 1: vshr.u64 >t1=reg128#11,<r0=reg128#4,#26
# asm 2: vshr.u64 >t1=q10,<r0=q3,#26
vshr.u64 q10,q3,#26

# qhasm: 				x23 = x23[0,2,1,3]
# asm 1: vtrn.32 <x23=reg128#10%bot,<x23=reg128#10%top
# asm 2: vtrn.32 <x23=d18,<x23=d19
vtrn.32 d18,d19

# qhasm:                    x01 = r0 & mask
# asm 1: vand >x01=reg128#4,<r0=reg128#4,<mask=reg128#1
# asm 2: vand >x01=q3,<r0=q3,<mask=q0
vand q3,q3,q0

# qhasm:                 2x r1 += t1
# asm 1: vadd.i64 >r1=reg128#2,<r1=reg128#2,<t1=reg128#11
# asm 2: vadd.i64 >r1=q1,<r1=q1,<t1=q10
vadd.i64 q1,q1,q10

# qhasm: 2x t4 = r3 unsigned>> 26
# asm 1: vshr.u64 >t4=reg128#11,<r3=reg128#9,#26
# asm 2: vshr.u64 >t4=q10,<r3=q8,#26
vshr.u64 q10,q8,#26

# qhasm: 				x01 = x01[0,2,1,3]
# asm 1: vtrn.32 <x01=reg128#4%bot,<x01=reg128#4%top
# asm 2: vtrn.32 <x01=d6,<x01=d7
vtrn.32 d6,d7

# qhasm:    r3 &= mask
# asm 1: vand >r3=reg128#1,<r3=reg128#9,<mask=reg128#1
# asm 2: vand >r3=q0,<r3=q8,<mask=q0
vand q0,q8,q0

# qhasm: 				r1 = r1[0,2,1,3]
# asm 1: vtrn.32 <r1=reg128#2%bot,<r1=reg128#2%top
# asm 2: vtrn.32 <r1=d2,<r1=d3
vtrn.32 d2,d3

# qhasm: 2x x4 = r4 + t4
# asm 1: vadd.i64 >x4=reg128#3,<r4=reg128#3,<t4=reg128#11
# asm 2: vadd.i64 >x4=q2,<r4=q2,<t4=q10
vadd.i64 q2,q2,q10

# qhasm: 				r3 = r3[0,2,1,3]
# asm 1: vtrn.32 <r3=reg128#1%bot,<r3=reg128#1%top
# asm 2: vtrn.32 <r3=d0,<r3=d1
vtrn.32 d0,d1

# qhasm: 				x01 = x01[0,1] r1[0,1]
# asm 1: vext.32 <x01=reg128#4%top,<r1=reg128#2%bot,<r1=reg128#2%bot,#0
# asm 2: vext.32 <x01=d7,<r1=d2,<r1=d2,#0
vext.32 d7,d2,d2,#0

# qhasm: 				x23 = x23[0,1] r3[0,1]
# asm 1: vext.32 <x23=reg128#10%top,<r3=reg128#1%bot,<r3=reg128#1%bot,#0
# asm 2: vext.32 <x23=d19,<r3=d0,<r3=d0,#0
vext.32 d19,d0,d0,#0

# qhasm: 				x4 = x4[0,2,1,3]
# asm 1: vtrn.32 <x4=reg128#3%bot,<x4=reg128#3%top
# asm 2: vtrn.32 <x4=d4,<x4=d5
vtrn.32 d4,d5

# qhasm: mem128[input_0] aligned= x01;input_0+=16
# asm 1: vst1.8 {<x01=reg128#4%bot-<x01=reg128#4%top},[<input_0=int32#1,: 128]!
# asm 2: vst1.8 {<x01=d6-<x01=d7},[<input_0=r0,: 128]!
vst1.8 {d6-d7},[r0,: 128]!

# qhasm: mem128[input_0] aligned= x23;input_0+=16
# asm 1: vst1.8 {<x23=reg128#10%bot-<x23=reg128#10%top},[<input_0=int32#1,: 128]!
# asm 2: vst1.8 {<x23=d18-<x23=d19},[<input_0=r0,: 128]!
vst1.8 {d18-d19},[r0,: 128]!

# qhasm: mem64[input_0] aligned= x4[0]
# asm 1: vst1.8 <x4=reg128#3%bot,[<input_0=int32#1,: 64]
# asm 2: vst1.8 <x4=d4,[<input_0=r0,: 64]
vst1.8 d4,[r0,: 64]

# qhasm: return
add sp,sp,#0
bx lr

#endif  /* !OPENSSL_NO_ASM && OPENSSL_ARM && __ELF__ */
