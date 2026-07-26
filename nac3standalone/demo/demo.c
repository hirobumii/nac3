#include <inttypes.h>
#include <limits.h>
#include <math.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

extern const uint8_t __nac3_global_begin;

struct cslice {
    void* data;
    size_t len;
};

typedef struct typeinfo_s {
    struct cslice* name;
    uint32_t* refcounted_field_offsets;
} typeinfo;

typedef struct object_header_s {
    uint32_t refcount;
    uint32_t typeinfo_offset;
} object_header;
_Static_assert(sizeof(object_header) == 8, "Unexpected object_header size");

typedef struct list_s {
    object_header header;
    void* data;
    size_t len;
} list;

// Internal Functions
static void* __nac3_list_get_data(const list* const slice) {
    // The element storage of a reference-counted array begins after the object header + count
    // fields, padded up to 8-byte alignment so 8-byte-aligned dtypes (e.g. double/int64) stay
    // aligned on 32-bit targets. This offset must match `RefCountedArrayType`'s layout in the
    // compiler.
    size_t off = (sizeof(object_header) + sizeof(size_t) + 7u) & ~(size_t)7u;
    return (char*)slice->data + off;
}

void output_refcount(const void* obj);

double dbl_nan(void) {
    return NAN;
}

double dbl_inf(void) {
    return INFINITY;
}

void output_bool(bool x) {
    puts(x ? "True" : "False");
}

void output_int32(int32_t x) {
    printf("%" PRId32 "\n", x);
}

void output_int64(int64_t x) {
    printf("%" PRId64 "\n", x);
}

void output_uint32(uint32_t x) {
    printf("%" PRIu32 "\n", x);
}

void output_uint64(uint64_t x) {
    printf("%" PRIu64 "\n", x);
}

void output_float64(double x) {
    if (isnan(x)) {
        puts("nan");
    } else {
        printf("%f\n", x);
    }
}

void output_range(int32_t range[3]) {
    printf("range(");
    printf("%d, %d", range[0], range[1]);
    if (range[2] != 1) {
        printf(", %d", range[2]);
    }
    puts(")");
}

void output_asciiart(int32_t x) {
    static const char* chars = " .,-:;i+hHM$*#@    ";
    if (x < 0) {
        putchar('\n');
    } else {
        putchar(chars[x]);
    }
}

void output_int32_list(list* slice) {
    const int32_t* data = (int32_t*)__nac3_list_get_data(slice);

    putchar('[');
    for (size_t i = 0; i < slice->len; ++i) {
        if (i == slice->len - 1) {
            printf("%d", data[i]);
        } else {
            printf("%d, ", data[i]);
        }
    }
    putchar(']');
    putchar('\n');
}

void output_str(struct cslice slice) {
    const char* data = (const char*)slice.data;

    for (size_t i = 0; i < slice.len; ++i) {
        putchar(data[i]);
    }
}

void output_strln(struct cslice slice) {
    output_str(slice);
    putchar('\n');
}

void output_refcount(const void* obj) {
    const object_header* header = (object_header*)obj;

    printf("refcount: ");

    if (header == NULL) {
        printf("<nil>");
    } else if (header->refcount == 0) {
        printf("<unsupported>");
    } else {
        printf("%" PRIu32, header->refcount - 2);
    }
    putchar('\n');

    //     printf("typeinfo: %s\n", obj == NULL ? "<nil>" : "");

    //     if (obj == NULL) {
    //         return;
    //     }

    //     const int32_t offset = *((int32_t*)(obj + 4));
    //     const typeinfo* ti = (void*)&__nac3_global_begin + offset;
    //     printf("  name: ");
    //     output_strln(*ti->name);
    //     puts("  refcounted_field_offsets:");

    //     const uint32_t* field_offsets = ti->refcounted_field_offsets;
    //     const uint32_t refcounted_field_count = field_offsets[0];
    //     if (refcounted_field_count == UINT32_MAX) {
    //         const size_t size = *((size_t*)(obj + sizeof(object_header)));
    // #if INTPTR_MAX == INT32_MAX
    //         printf("    refcounted_field_count: <ALL> (%" PRIu32 ")\n", size);
    // #elif INTPTR_MAX == INT64_MAX
    //         printf("    refcounted_field_count: <ALL> (%" PRIu64 ")\n", size);
    // #else
    // #error "Unsupported pointer size"
    // #endif
    //     } else {
    //         printf("    refcounted_field_count: %" PRIu32, refcounted_field_count);
    //         if (refcounted_field_count > 0) {
    //             printf(" [");
    //             for (uint32_t i = 0; i < refcounted_field_count; ++i) {
    //                 printf("%" PRIu32 "%s", field_offsets[i + 1], i == refcounted_field_count - 1 ? "" : ", ");
    //             }
    //             putchar(']');
    //         }
    //         putchar('\n');
    //     }
}

uint64_t dbg_stack_address(__attribute__((unused)) struct cslice slice) {
    int i;
    void* ptr = (void*)&i;
    return (uintptr_t)ptr;
}

uint32_t __nac3_personality(uint32_t state, uint32_t exception_object, uint32_t context) {
    printf("__nac3_personality(state: %u, exception_object: %u, context: %u)\n", state, exception_object, context);
    exit(101);
    __builtin_unreachable();
}

// See `struct Exception<'a>` in
// https://github.com/m-labs/artiq/blob/master/artiq/firmware/libeh/eh_artiq.rs
struct Exception {
    uint32_t id;
    struct cslice file;
    uint32_t line;
    uint32_t column;
    struct cslice function;
    struct cslice message;
    // Explicit padding to pin `param` at offset 40 on 32-bit targets, where the i686 ABI would
    // otherwise align `int64_t` to 4 and place it at 36. This must match `Exception` in
    // `nac3core/irrt/irrt/exception.hpp` and `ExceptionStructFields` in the compiler.
    uint8_t _padding[8 - sizeof(size_t)];
    int64_t param[3];
};
_Static_assert(offsetof(struct Exception, param) == (sizeof(size_t) == 4 ? 40 : 64),
               "Unexpected Exception param offset");

uint32_t __nac3_raise(struct Exception* e) {
    printf("__nac3_raise called. Exception details:\n");
    printf("          ID: %" PRIu32 "\n", e->id);
    printf("    Location: %*s:%" PRIu32 ":%" PRIu32 "\n", (int)e->file.len, (const char*)e->file.data, e->line,
           e->column);
    printf("    Function: %*s\n", (int)e->function.len, (const char*)e->function.data);
    printf("     Message: \"%*s\"\n", (int)e->message.len, (const char*)e->message.data);
    printf("      Params: {0}=%" PRId64 ", {1}=%" PRId64 ", {2}=%" PRId64 "\n", e->param[0], e->param[1], e->param[2]);
    exit(101);
    __builtin_unreachable();
}

void __nac3_end_catch(void) {}

void __nac3_resume(void) {
    printf("__nac3_resume called\n");
    exit(101);
    __builtin_unreachable();
}

extern int32_t run(void);

int main(void) {
    run();
}
