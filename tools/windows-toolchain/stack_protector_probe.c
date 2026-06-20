#if !defined(_WIN32)
#error "this probe must be compiled for Windows"
#endif

__attribute__((noinline)) static int protected_value(int value) {
  volatile char buffer[32] = {0};
  buffer[0] = (char)value;
  return buffer[0];
}

int main(void) { return protected_value(7) == 7 ? 0 : 1; }
