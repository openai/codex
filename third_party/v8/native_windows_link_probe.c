// __powidf2 is in compiler-rt's generic builtins on both x64 and arm64.
// MSVC must resolve it from the staged V8 archive without any LLVM search path.
extern double __powidf2(double, int);

int main(void) {
  return __powidf2(3.0, 4) == 81.0 ? 0 : 1;
}
