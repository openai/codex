#include <ring-core/asm_base.h>

#if !defined(OPENSSL_NO_ASM) && defined(OPENSSL_X86_64) && \
    (defined(__APPLE__) || defined(__ELF__))

.intel_syntax noprefix
.text
#if defined(__APPLE__)
.private_extern _fiat_curve25519_adx_mul
.global _fiat_curve25519_adx_mul
_fiat_curve25519_adx_mul:
#else
.type fiat_curve25519_adx_mul, @function
.hidden fiat_curve25519_adx_mul
.global fiat_curve25519_adx_mul
fiat_curve25519_adx_mul:
#endif

.cfi_startproc
_CET_ENDBR
push rbp
.cfi_adjust_cfa_offset 8
.cfi_offset rbp, -16
mov rbp, rsp

mov rax, rdx
mov rdx, [ rsi + 0x18 ]
mulx r11, r10, [ rax + 0x8 ]
mov rdx, [ rax + 0x0 ]
mov [ rsp - 0x58 ], r15
.cfi_offset r15, -16-0x58
mulx r8, rcx, [ rsi + 0x18 ]
mov rdx, [ rsi + 0x8 ]
mov [ rsp - 0x80 ], rbx
.cfi_offset rbx, -16-0x80
mulx rbx, r9, [ rax + 0x18 ]
mov rdx, [ rsi + 0x8 ]
mov [ rsp - 0x70 ], r12
.cfi_offset r12, -16-0x70
mulx r15, r12, [ rax + 0x8 ]
mov rdx, [ rsi + 0x0 ]
mov [ rsp - 0x68 ], r13
.cfi_offset r13, -16-0x68
mov [ rsp - 0x60 ], r14
.cfi_offset r14, -16-0x60
mulx r14, r13, [ rax + 0x0 ]
mov rdx, [ rax + 0x10 ]
mov [ rsp - 0x18 ], r15
mov [ rsp - 0x50 ], rdi
mulx rdi, r15, [ rsi + 0x0 ]
mov rdx, [ rax + 0x18 ]
mov [ rsp - 0x48 ], r13
mov [ rsp - 0x40 ], r9
mulx r9, r13, [ rsi + 0x0 ]
test al, al
adox rcx, rdi
mov rdx, [ rsi + 0x10 ]
mov [ rsp - 0x38 ], r13
mulx r13, rdi, [ rax + 0x8 ]
adox r10, r9
mov rdx, 0x0
adox rbx, rdx
adcx rdi, rcx
adcx r8, r10
mov r9, rdx
adcx r9, rbx
mov rdx, [ rsi + 0x10 ]
mulx r10, rcx, [ rax + 0x0 ]
mov rdx, [ rsi + 0x0 ]
mov [ rsp - 0x30 ], r15
mulx r15, rbx, [ rax + 0x8 ]
mov rdx, -0x2
inc rdx
adox rcx, r15
setc r15b
clc
adcx rcx, r12
adox r10, rdi
mov rdx, [ rax + 0x10 ]
mov [ rsp - 0x78 ], rcx
mulx rcx, rdi, [ rsi + 0x10 ]
adox rdi, r8
mov rdx, [ rax + 0x18 ]
mov [ rsp - 0x28 ], rcx
mulx rcx, r8, [ rsi + 0x10 ]
mov rdx, [ rax + 0x10 ]
mov [ rsp - 0x20 ], r8
mulx r12, r8, [ rsi + 0x18 ]
adox r8, r9
mov rdx, [ rsi + 0x8 ]
mov [ rsp - 0x10 ], r12
mulx r12, r9, [ rax + 0x10 ]
movzx rdx, r15b
lea rdx, [ rdx + rcx ]
adcx r9, r10
adcx r13, rdi
mov r15, 0x0
mov r10, r15
adox r10, rdx
mov rdx, [ rax + 0x18 ]
mulx rcx, rdi, [ rsi + 0x18 ]
adox rcx, r15
adcx r11, r8
mov rdx, r15
adcx rdx, r10
adcx rcx, r15
mov r8, rdx
mov rdx, [ rax + 0x0 ]
mulx r15, r10, [ rsi + 0x8 ]
test al, al
adox r10, r14
adcx rbx, r10
adox r15, [ rsp - 0x78 ]
adcx r15, [ rsp - 0x30 ]
adox r9, [ rsp - 0x18 ]
adcx r9, [ rsp - 0x38 ]
adox r13, [ rsp - 0x40 ]
adcx r12, r13
adox r11, [ rsp - 0x20 ]
adcx r11, [ rsp - 0x28 ]
mov rdx, 0x26
mulx rsi, r14, r12
adox rdi, r8
adcx rdi, [ rsp - 0x10 ]
mulx r10, r8, r11
mov r13, 0x0
adox rcx, r13
adcx rcx, r13
mulx r11, r12, rdi
xor rdi, rdi
adox r8, rbx
adox r12, r15
mulx rbx, r13, rcx
adcx r14, [ rsp - 0x48 ]
adox r13, r9
adox rbx, rdi
adcx rsi, r8
adcx r10, r12
adcx r11, r13
adc rbx, 0x0
mulx r9, r15, rbx
xor r9, r9
adox r15, r14
mov rdi, r9
adox rdi, rsi
mov rcx, r9
adox rcx, r10
mov r8, [ rsp - 0x50 ]
mov [ r8 + 0x8 ], rdi
mov r12, r9
adox r12, r11
mov r14, r9
cmovo r14, rdx
mov [ r8 + 0x18 ], r12
adcx r15, r14
mov [ r8 + 0x0 ], r15
mov [ r8 + 0x10 ], rcx
mov rbx, [ rsp - 0x80 ]
.cfi_restore rbx
mov r12, [ rsp - 0x70 ]
.cfi_restore r12
mov r13, [ rsp - 0x68 ]
.cfi_restore r13
mov r14, [ rsp - 0x60 ]
.cfi_restore r14
mov r15, [ rsp - 0x58 ]
.cfi_restore r15

pop rbp
.cfi_restore rbp
.cfi_adjust_cfa_offset -8
ret
.cfi_endproc
#if defined(__ELF__)
.size fiat_curve25519_adx_mul, .-fiat_curve25519_adx_mul
#endif

#endif
