#include <cstdio>

extern "C" bool v8__V8__IsSandboxEnabled();

int main() {
  if (!v8__V8__IsSandboxEnabled()) return 1;
  std::puts("V8 sandbox enabled");
  return 0;
}
