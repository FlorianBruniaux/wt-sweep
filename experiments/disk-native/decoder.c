#include "decoder.h"
#include <sys/vnode.h>
#include <string.h>
#include <stdbool.h>
static bool take(const unsigned char *p, size_t n, size_t *at, void *to, size_t size) {
    if (*at > n || size > n - *at) return false;
    memcpy(to, p + *at, size); *at += size; return true;
}
enum decoded_status decode_entry(const unsigned char *p, size_t n, struct decoded *out) {
    memset(out, 0, sizeof(*out));
    size_t at = 0; uint32_t length; attribute_set_t attrs;
    if (!take(p,n,&at,&length,sizeof(length)) || length < 24 || length > n || length % 8)
        return DECODE_INVALID;
    out->used = length;
    if (!take(p,length,&at,&attrs,sizeof(attrs))) return DECODE_INVALID;
    attrgroup_t common = ATTR_CMN_RETURNED_ATTRS | ATTR_CMN_ERROR | ATTR_CMN_NAME |
        ATTR_CMN_DEVID | ATTR_CMN_OBJTYPE | ATTR_CMN_FLAGS | ATTR_CMN_FILEID;
    attrgroup_t file = ATTR_FILE_ALLOCSIZE | ATTR_FILE_DATALENGTH;
    if (attrs.commonattr & ~common || attrs.fileattr & ~file || attrs.volattr ||
        attrs.dirattr || attrs.forkattr || !(attrs.commonattr & ATTR_CMN_RETURNED_ATTRS) ||
        !(attrs.commonattr & ATTR_CMN_NAME)) return DECODE_INVALID;
    if (attrs.commonattr & ATTR_CMN_ERROR)
        if (!take(p,length,&at,&out->error,4)) return DECODE_INVALID;
    attrreference_t name; size_t name_at = at;
    if (!take(p,length,&at,&name,sizeof(name))) return DECODE_INVALID;
    int64_t start = (int64_t)name_at + name.attr_dataoffset;
    if (start < 0 || (uint64_t)start > length || name.attr_length < 2 ||
        name.attr_length > length - (size_t)start) return DECODE_INVALID;
    out->name = p + start; out->name_len = name.attr_length - 1;
    if (out->name[out->name_len] != 0 || memchr(out->name,0,out->name_len) ||
        memchr(out->name,'/',out->name_len)) return DECODE_INVALID;
    /* On entry failure the rest of the metadata may be absent. */
    if (out->error) return DECODE_ENTRY_ERROR;
#define FIELD(bit, field) do { if (attrs.commonattr & (bit)) \
    if (!take(p,length,&at,&out->field,sizeof(out->field))) return DECODE_INVALID; } while(0)
    FIELD(ATTR_CMN_DEVID, device);
    FIELD(ATTR_CMN_OBJTYPE, type);
    FIELD(ATTR_CMN_FLAGS, flags);
    FIELD(ATTR_CMN_FILEID, inode);
#undef FIELD
    if (attrs.fileattr & ATTR_FILE_ALLOCSIZE)
        if (!take(p,length,&at,&out->allocated,sizeof(off_t))) return DECODE_INVALID;
    if (attrs.fileattr & ATTR_FILE_DATALENGTH)
        if (!take(p,length,&at,&out->logical,sizeof(off_t))) return DECODE_INVALID;
    if ((size_t)start < at) return DECODE_INVALID;
    /* Error is reported only for failing entries; success omits its bit/field. */
    attrgroup_t required = common & ~ATTR_CMN_ERROR;
    if ((attrs.commonattr & required) != required) return DECODE_MISSING;
    if ((out->type == VREG || out->type == VLNK) && attrs.fileattr != file)
        return DECODE_MISSING;
    if (out->logical < 0 || out->allocated < 0) return DECODE_INVALID;
    return DECODE_OK;
}
