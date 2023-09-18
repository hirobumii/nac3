#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define min(a, b) \
  ({ \
    __typeof__(a) _a = (a); \
    __typeof__(b) _b = (b); \
    _a < _b ? _a : _b; \
  })

#if __SIZEOF_POINTER__ == 8
  #define usize uint64_t
#elif __SIZEOF_POINTER__ == 4
  #define usize uint32_t
#elif __SIZEOF_POINTER__ == 2
  #define usize uint16_t
#endif

void output_int32(const int32_t x) {
  printf("%d\n", x);
}

void output_int64(const int64_t x) {
  printf("%ld\n", x);
}

void output_uint32(const uint32_t x) {
  printf("%d\n", x);
}

void output_uint64(const uint64_t x) {
  printf("%ld\n", x);
}

void output_asciiart(const int32_t x) {
  const char* chars = " .,-:;i+hHM$*#@  ";
  if (x < 0) {
    fputc('\n', stdout);
  } else {
    fputc(chars[x], stdout);
  }
}

struct cslice_int32 {
  const int32_t* data;
  usize len;
};

void output_int32_list(struct cslice_int32* slice) {
  fputc('[', stdout);
  for (usize i = 0; i < slice->len; ++i) {
    if (i == slice->len - 1) {
      printf("%d", slice->data[i]);
    } else {
      printf("%d, ", slice->data[i]);
    }
  }
  puts("]");
}

uint32_t __nac3_personality(
    __attribute__((unused)) uint32_t state,
    __attribute__((unused)) uint32_t exception_object,
    __attribute__((unused)) uint32_t context) {
  assert(false && "__nac3_personality not implemented");
  exit(101);
  __builtin_unreachable();
}

uint32_t __nac3_raise(uint32_t state, uint32_t exception_object, uint32_t context) {
  printf("__nac3_raise(state: %x, exception_object: %x, context: %x\n", state, exception_object, context);
  exit(101);
  __builtin_unreachable();
}

extern int32_t run();

int main() {
  run();
}
