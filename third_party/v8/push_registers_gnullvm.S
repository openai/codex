.text
.globl PushAllRegistersAndIterateStack
.seh_proc PushAllRegistersAndIterateStack
PushAllRegistersAndIterateStack:
  push %rbp
  .seh_pushreg %rbp
  mov %rsp, %rbp
  .seh_setframe %rbp, 0
  push $0xCDCDCD
  .seh_stackalloc 8
  push %rsi
  .seh_pushreg %rsi
  push %rdi
  .seh_pushreg %rdi
  push %rbx
  .seh_pushreg %rbx
  push %r12
  .seh_pushreg %r12
  push %r13
  .seh_pushreg %r13
  push %r14
  .seh_pushreg %r14
  push %r15
  .seh_pushreg %r15
  sub $160, %rsp
  .seh_stackalloc 160
  # Preserve Windows unwind metadata even though this is now GNU syntax.
  # Without the .seh_* directives, SEH and stack walking can mis-handle
  # unwinding across this callback frame.
  .seh_endprologue
  movdqa %xmm6, 144(%rsp)
  movdqa %xmm7, 128(%rsp)
  movdqa %xmm8, 112(%rsp)
  movdqa %xmm9, 96(%rsp)
  movdqa %xmm10, 80(%rsp)
  movdqa %xmm11, 64(%rsp)
  movdqa %xmm12, 48(%rsp)
  movdqa %xmm13, 32(%rsp)
  movdqa %xmm14, 16(%rsp)
  movdqa %xmm15, 0(%rsp)
  mov %r8, %r9
  mov %rsp, %r8
  call *%r9
  add $224, %rsp
  pop %rbp
  ret
.seh_endproc
