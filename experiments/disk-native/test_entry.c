/* Error/metadata injection exercises the actual entry handler. Real disposable
   paths prove scalar fallback, but synthetic flags do not prove live mounts. */
#define main walk_program_main
#include "walk.c"
#undef main
#include <assert.h>
int main(int argc, char **argv) {
    assert(argc == 2);
    int fd = open(argv[1], DIRFLAGS); assert(fd >= 0);
    struct row row = {.path = argv[1], .complete = true, .discovered_ok = true};
    assert(fstat(fd, &row.discovered) == 0);
    struct collector c = {.rows = &row, .count = 1,
        .opt = {.native = true, .max_entries = 100, .max_depth = 10, .timeout_ms = 10000}};
    c.start = now_ns();
    struct decoded entry = {.type = VREG, .error = 13, .device = row.discovered.st_dev,
        .inode = 12345, .logical = 123, .allocated = 4096};
    child(&c,&row,fd,argv[1],"file",&entry,DECODE_ENTRY_ERROR,0,100,c.start,10000);
    assert(!row.complete && !row.measured && row.gaps_len == 1 && c.stat_calls == 0);
    assert(row.logical == 0 && c.logical == 0 && row.checked == 1);
    row.complete = true;
    child(&c,&row,fd,argv[1],"file",&entry,DECODE_MISSING,0,100,c.start,10000);
    assert(row.complete && row.measured && row.logical == 3 && c.stat_calls == 1);
    /* A directory's bulk metadata can describe an underlying mount/firmlink
       stub. Its device/identity/flags must not override target statat. */
    entry = (struct decoded){.type = VDIR, .device = -1, .inode = 12345, .flags = SF_FIRMLINK};
    child(&c,&row,fd,argv[1],"d",&entry,DECODE_OK,0,100,c.start,10000);
    assert(row.complete && row.logical == 7 && c.stat_calls >= 4);
    free(row.seen.slots); free(c.seen.slots); close(fd);
    puts("entry handler: explicit error gap, positive missing-metadata scalar fallback, directory target re-read passed");
}
