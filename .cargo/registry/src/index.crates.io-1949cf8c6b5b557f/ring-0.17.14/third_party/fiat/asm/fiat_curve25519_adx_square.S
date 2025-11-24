#include <ring-core/asm_base.h>

#if !defined(OPENSSL_NO_ASM) && defined(OPENSSL_X86_64) && \
    (defined(__APPLE__) || defined(__ELF__))

.intel_syntax noprefix
.text
#if defined(__APPLE__)
.private_extern _fiat_curve25519_adx_square
.global _fiat_curve25519_adx_square
_fiat_curve25519_adx_square:
#else
.type fiat_curve25519_adx_square, @function
.hidden fiat_curve25519_adx_square
.global fiat_curve25519_adx_square
fiat_curve25519_adx_square:
#endif

.cfi_startproc
_CET_ENDBR
push rbp
.cfi_adjust_cfa_offset 8
.cfi_offset rbp, -16
mov rbp, rsp

mov rdx, [ rsi + 0x0 ]
mulx r10, rax, [ rsi + 0x8 ]
mov rdx, [ rsi + 0x0 ]
mulx rcx, r11, [ rsi + 0x10 ]
xor rdx, rdx
adox r11, r10
mov rdx, [ rsi + 0x0 ]
mulx r9, r8, [ rsi + 0x18 ]
mov rdx, [ rsi + 0x8 ]
mov [ rsp - 0x80 ], rbx
.cfi_offset rbx, -16-0x80
mulx rbx, r10, [ rsi + 0x18 ]
adox r8, rcx
mov [rsp - 0x48 ], rdi
adox r10, r9
adcx rax, rax
mov rdx, [ rsi + 0x10 ]
mulx r9, rcx, [ rsi + 0x18 ]
adox rcx, rbx
mov rdx, [ rsi + 0x10 ]
mulx rdi, rbx, [ rsi + 0x8 ]
mov rdx, 0x0
adox r9, rdx
mov [ rsp - 0x70 ], r12
.cfi_offset r12, -16-0x70
mov r12, -0x3
inc r12
adox rbx, r8
adox rdi, r10
adcx r11, r11
mov r8, rdx
adox r8, rcx
mov r10, rdx
adox r10, r9
adcx rbx, rbx
mov rdx, [ rsi + 0x0 ]
mulx r9, rcx, rdx
mov rdx, [ rsi + 0x8 ]
mov [ rsp - 0x68 ], r13
.cfi_offset r13, -16-0x68
mov [ rsp - 0x60 ], r14
.cfi_offset r14, -16-0x60
mulx r14, r13, rdx
seto dl
inc r12
adox r9, rax
adox r13, r11
adox r14, rbx
adcx rdi, rdi
mov al, dl
mov rdx, [ rsi + 0x10 ]
mulx rbx, r11, rdx
adox r11, rdi
adcx r8, r8
adox rbx, r8
adcx r10, r10
movzx rdx, al
mov rdi, 0x0
adcx rdx, rdi
movzx r8, al
lea r8, [ r8 + rdx ]
mov rdx, [ rsi + 0x18 ]
mulx rdi, rax, rdx
adox rax, r10
mov rdx, 0x26
mov [ rsp - 0x58 ], r15
.cfi_offset r15, -16-0x58
mulx r15, r10, r11
clc
adcx r10, rcx
mulx r11, rcx, rbx
adox r8, rdi
mulx rdi, rbx, r8
inc r12
adox rcx, r9
mulx r8, r9, rax
adcx r15, rcx
adox r9, r13
adcx r11, r9
adox rbx, r14
adox rdi, r12
adcx r8, rbx
adc rdi, 0x0
mulx r14, r13, rdi
test al, al
mov rdi, [ rsp - 0x48 ]
adox r13, r10
mov r14, r12
adox r14, r15
mov [ rdi + 0x8 ], r14
mov rax, r12
adox rax, r11
mov r10, r12
adox r10, r8
mov [ rdi + 0x10 ], rax
mov rcx, r12
cmovo rcx, rdx
adcx r13, rcx
mov [ rdi + 0x0 ], r13
mov [ rdi + 0x18 ], r10
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
.size fiat_curve25519_adx_square, .-fiat_curve25519_adx_square
#endif

#endif
