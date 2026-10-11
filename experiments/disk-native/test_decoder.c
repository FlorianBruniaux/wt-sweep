#include "decoder.h"
#include <sys/vnode.h>
#include <assert.h>
#include <string.h>
#include <stdio.h>
/* Hand-built SDK ABI record, not encoded by the decoder under test. */
static size_t record(unsigned char *p) {
    uint32_t len = 72, err = 0, flags = 0;
    attribute_set_t attrs = { .commonattr = ATTR_CMN_RETURNED_ATTRS | ATTR_CMN_ERROR |
        ATTR_CMN_NAME | ATTR_CMN_DEVID | ATTR_CMN_OBJTYPE | ATTR_CMN_FLAGS | ATTR_CMN_FILEID,
        .fileattr = ATTR_FILE_ALLOCSIZE | ATTR_FILE_DATALENGTH };
    attrreference_t ref = { .attr_dataoffset = 40, .attr_length = 3 };
    dev_t device = 11; fsobj_type_t type = VREG; uint64_t ino = 42;
    off_t allocated = 4096, logical = 7;
    memset(p, 0, 128);
    memcpy(p, &len, 4); memcpy(p+4, &attrs, 20); memcpy(p+24, &err, 4);
    memcpy(p+28, &ref, 8); memcpy(p+36, &device, 4); memcpy(p+40, &type, 4);
    memcpy(p+44, &flags, 4); memcpy(p+48, &ino, 8);
    memcpy(p+56, &allocated, 8); memcpy(p+64, &logical, 8);
    /* Variable names must start after all fixed fields, adjust length/reference. */
    len = 80; ref.attr_dataoffset = 44;
    memcpy(p, &len, 4); memcpy(p+28, &ref, 8);
    p[72] = 'x'; p[73] = 0xff; p[74] = 0;
    return len;
}
int main(void) {
    unsigned char p[128]; struct decoded d;
    size_t n = record(p);
    assert(decode_entry(p, n, &d) == DECODE_OK);
    assert(d.used == 80 && d.name_len == 2 && d.name[1] == 0xff);
    assert(d.device == 11 && d.inode == 42 && d.logical == 7 && d.allocated == 4096);
    /* Darwin omits ATTR_CMN_ERROR on successful entries. Its absence is not
       missing metadata; the following fields shift left by uint32_t. */
    attribute_set_t success; memcpy(&success,p+4,sizeof(success));
    success.commonattr &= ~ATTR_CMN_ERROR;
    memmove(p+24,p+28,n-28); memcpy(p+4,&success,sizeof(success));
    assert(decode_entry(p,n,&d) == DECODE_OK && d.error == 0);
    assert(d.device == 11 && d.inode == 42 && d.logical == 7 && d.allocated == 4096);
    record(p);
    for (size_t cut = 0; cut < n; ++cut) assert(decode_entry(p, cut, &d) == DECODE_INVALID);
    uint32_t value = 0; memcpy(p, &value, 4);
    assert(decode_entry(p, n, &d) == DECODE_INVALID);
    value = 81; memcpy(p, &value, 4);
    assert(decode_entry(p, sizeof(p), &d) == DECODE_INVALID);
    record(p); value = UINT32_MAX; memcpy(p, &value, 4);
    assert(decode_entry(p, sizeof(p), &d) == DECODE_INVALID);
    record(p); p[74] = 'a'; assert(decode_entry(p, n, &d) == DECODE_INVALID);
    record(p); p[73] = '/'; assert(decode_entry(p, n, &d) == DECODE_INVALID);
    record(p); p[72] = 0; assert(decode_entry(p, n, &d) == DECODE_INVALID);
    record(p); int32_t off = INT32_MIN; memcpy(p+28, &off, 4);
    assert(decode_entry(p, n, &d) == DECODE_INVALID);
    record(p); off = 0; memcpy(p+28, &off, 4);
    assert(decode_entry(p, n, &d) == DECODE_INVALID);
    record(p); value = UINT32_MAX; memcpy(p+32, &value, 4);
    assert(decode_entry(p, n, &d) == DECODE_INVALID);
    /* Missing metadata is explicit; invalid packed zeros are never accepted. */
    record(p); attribute_set_t attrs; memcpy(&attrs,p+4,20);
    attrs.fileattr = 0; memcpy(p+4,&attrs,20);
    assert(decode_entry(p,n,&d) == DECODE_MISSING);
    record(p); value = 13; memcpy(p+24,&value,4);
    assert(decode_entry(p,n,&d) == DECODE_ENTRY_ERROR && d.error == 13);
    record(p); attrs.commonattr &= ~ATTR_CMN_NAME; memcpy(p+4,&attrs,20);
    assert(decode_entry(p,n,&d) == DECODE_INVALID);
    puts("decoder: valid exact bytes, truncations, invalid lengths/names/offsets, missing attrs and entry errors passed");
}
