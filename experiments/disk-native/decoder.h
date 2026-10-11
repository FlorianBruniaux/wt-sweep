#ifndef SWEEP_NATIVE_DECODER_H
#define SWEEP_NATIVE_DECODER_H
#include <sys/attr.h>
#include <sys/types.h>
#include <stddef.h>
#include <stdint.h>
enum decoded_status { DECODE_OK, DECODE_INVALID, DECODE_MISSING, DECODE_ENTRY_ERROR };
struct decoded {
    const unsigned char *name;
    size_t name_len, used;
    dev_t device;
    uint64_t inode;
    fsobj_type_t type;
    uint32_t flags, error;
    off_t logical, allocated;
};
enum decoded_status decode_entry(const unsigned char *, size_t, struct decoded *);
#endif
