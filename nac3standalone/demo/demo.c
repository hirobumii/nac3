#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#if __SIZEOF_POINTER__ == 8
  #define usize uint64_t
#elif __SIZEOF_POINTER__ == 4
  #define usize uint32_t
#else
  #error "Unsupported platform - Platform is not 32-bit or 64-bit"
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

void output_float64(const double x) {
  printf("%f\n", x);
}
void output_asciiart(const int32_t x) {
  const char* chars = " .,-:;i+hHM$*#@  ";
  if (x < 0) {
    putchar('\n');
  } else {
    putchar(chars[x]);
  }
}

struct cslice {
  const void* data;
  usize len;
};

void output_int32_list(struct cslice* slice) {
  const int32_t* data = (const int32_t*) slice->data;

  putchar('[');
  for (usize i = 0; i < slice->len; ++i) {
    if (i == slice->len - 1) {
      printf("%d", data[i]);
    } else {
      printf("%d, ", data[i]);
    }
  }
  putchar(']');
  putchar('\n');
}

void output_str(struct cslice* slice) {
  const char* data = (const char*) slice->data;

  for (usize i = 0; i < slice->len; ++i) {
    putchar(data[i]);
  }
  putchar('\n');
}

uint32_t __nac3_personality(uint32_t state, uint32_t exception_object, uint32_t context) {
  printf("__nac3_personality(state: %u, exception_object: %u, context: %u\n", state, exception_object, context);
  exit(101);
  __builtin_unreachable();
}

uint32_t __nac3_raise(uint32_t state, uint32_t exception_object, uint32_t context) {
  printf("__nac3_raise(state: %u, exception_object: %u, context: %u\n", state, exception_object, context);
  exit(101);
  __builtin_unreachable();
}

void __nac3_end_catch(void) {}

extern int32_t run(void);

int main(void) {
  run();
}
