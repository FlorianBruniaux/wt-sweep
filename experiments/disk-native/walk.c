/* Isolated native-SDK experiment, not a production backend. */
#include "decoder.h"
#include <sys/stat.h>
#include <sys/vnode.h>
#include <dirent.h>
#include <fcntl.h>
#include <unistd.h>
#include <errno.h>
#include <stdbool.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <limits.h>
#define DIRFLAGS (O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC | O_NONBLOCK)
#define BUFFER_SIZE 65536
struct key { uint64_t dev, ino; bool occupied; };
struct set { struct key *slots; size_t cap, len; };
struct object { char *path; uint64_t dev, ino, logical, allocated; };
struct row {
    const char *path;
    struct stat discovered;
    bool discovered_ok, measured, complete;
    uint64_t logical, allocated;
    size_t checked, excluded, omitted;
    char gaps[8][100]; size_t gaps_len;
    struct set seen;
    struct object *objects; size_t objects_len, objects_cap;
};
struct options {
    bool native, manifest, pause_root, pause_end, paused;
    const char *pause_child;
    size_t max_entries, max_depth, per_root;
    uint64_t timeout_ms, per_root_ms;
};
struct collector {
    struct options opt; struct row *rows; size_t count, checked;
    struct set seen; uint64_t logical, allocated, start;
    size_t stat_calls, bulk_calls, readdir_calls, open_calls;
};
static void *alloc(size_t n) { void *p = calloc(1,n); if (!p) { perror("allocation"); exit(2); } return p; }
static uint64_t now_ns(void) {
    struct timespec t;
    if (clock_gettime(CLOCK_MONOTONIC,&t)) { perror("clock"); exit(2); }
    return (uint64_t)t.tv_sec * 1000000000 + (uint64_t)t.tv_nsec;
}
static uint64_t hash(uint64_t dev, uint64_t ino) {
    uint64_t x = ino ^ (dev * UINT64_C(0x9e3779b97f4a7c15));
    x ^= x >> 30; x *= UINT64_C(0xbf58476d1ce4e5b9); x ^= x >> 27;
    return x;
}
static bool insert(struct set *s, uint64_t dev, uint64_t ino) {
    if (!s->cap || s->len >= s->cap/2) {
        size_t cap = s->cap ? s->cap*2 : 64;
        struct key *old = s->slots; size_t oldcap = s->cap;
        s->slots = alloc(cap*sizeof(struct key)); s->cap=cap; s->len=0;
        for (size_t i=0;i<oldcap;i++) if (old[i].occupied) insert(s,old[i].dev,old[i].ino);
        free(old);
    }
    size_t at=hash(dev,ino)&(s->cap-1);
    while (s->slots[at].occupied) {
        if (s->slots[at].dev==dev && s->slots[at].ino==ino) return false;
        at=(at+1)&(s->cap-1);
    }
    s->slots[at]=(struct key){dev,ino,true}; s->len++; return true;
}
static void gap(struct row *r, const char *reason) {
    r->complete=false;
    if (r->gaps_len<8) snprintf(r->gaps[r->gaps_len++],100,"%s",reason);
    else r->omitted++;
}
static bool unchanged(const struct stat *a, const struct stat *b) {
    return a->st_dev==b->st_dev && a->st_ino==b->st_ino && a->st_mode==b->st_mode &&
        a->st_size==b->st_size && a->st_mtimespec.tv_sec==b->st_mtimespec.tv_sec &&
        a->st_mtimespec.tv_nsec==b->st_mtimespec.tv_nsec &&
        a->st_ctimespec.tv_sec==b->st_ctimespec.tv_sec && a->st_ctimespec.tv_nsec==b->st_ctimespec.tv_nsec;
}
static bool check(struct collector *c, struct row *r, size_t allowance, uint64_t start, uint64_t ms) {
    uint64_t time=now_ns();
    if ((time-c->start)/1000000>=c->opt.timeout_ms || (time-start)/1000000>=ms) {
        gap(r,"time budget"); return false;
    }
    if (c->checked>=c->opt.max_entries || r->checked>=allowance) { gap(r,"entry budget"); return false; }
    return true;
}
static char *join(const char *path, const char *name) {
    size_t a=strlen(path),b=strlen(name);
    if (a+b+2>1024*1024) { fputs("path budget\n",stderr); exit(2); }
    char *p=alloc(a+b+2); memcpy(p,path,a); p[a]='/'; memcpy(p+a+1,name,b+1); return p;
}
static bool add(uint64_t *to, uint64_t n) { if (*to>UINT64_MAX-n) return false; *to+=n; return true; }
static bool account(struct collector *c,struct row *r,const char *path,uint64_t dev,uint64_t ino,uint64_t logical,uint64_t allocated) {
    if (dev!=(uint64_t)r->discovered.st_dev) { gap(r,"filesystem boundary"); return false; }
    if (!insert(&r->seen,dev,ino)) return false;
    if (!add(&r->logical,logical) || !add(&r->allocated,allocated)) { gap(r,"byte overflow"); return false; }
    r->measured=true;
    if (insert(&c->seen,dev,ino))
        if (!add(&c->logical,logical) || !add(&c->allocated,allocated)) { gap(r,"byte overflow"); return false; }
    if (c->opt.manifest) {
        if (r->objects_len==r->objects_cap) {
            size_t cap=r->objects_cap?r->objects_cap*2:64;
            struct object *p=realloc(r->objects,cap*sizeof(*p)); if(!p) exit(2);
            r->objects=p; r->objects_cap=cap;
        }
        struct object *o=&r->objects[r->objects_len++];
        *o=(struct object){strdup(path),dev,ino,logical,allocated}; if(!o->path) exit(2);
    }
    return true;
}
static bool stat_account(struct collector *c,struct row *r,const char *path,const struct stat *st) {
    if(st->st_size<0 || st->st_blocks<0 || (uint64_t)st->st_blocks>UINT64_MAX/512) {
        gap(r,"invalid scalar sizes"); return false;
    }
    return account(c,r,path,st->st_dev,st->st_ino,
        S_ISREG(st->st_mode)||S_ISLNK(st->st_mode)?(uint64_t)st->st_size:0,(uint64_t)st->st_blocks*512);
}
static void pause_here(struct collector *c) {
    if(c->opt.paused) return;
    c->opt.paused=true; fputs("paused\n",stderr); fflush(stderr);
    char line[40]; if(!fgets(line,sizeof(line),stdin)) exit(2);
}
static bool registered(struct collector *c,const char *path,const struct stat *st) {
    for(size_t i=0;i<c->count;i++) {
        struct row *r=&c->rows[i];
        if(!strcmp(r->path,path) || (r->discovered_ok && st->st_dev==r->discovered.st_dev && st->st_ino==r->discovered.st_ino)) return true;
    }
    return false;
}
static void directory(struct collector *,struct row *,int,const char *,size_t,const struct stat *,size_t,uint64_t,uint64_t);
static void child(struct collector *c,struct row *r,int fd,const char *parent,const char *name,
                  const struct decoded *native,enum decoded_status status,size_t depth,size_t allowance,uint64_t start,uint64_t ms) {
    if(!strcmp(name,".") || !strcmp(name,"..")) return;
    char *path=join(parent,name); c->checked++; r->checked++;
    if(status==DECODE_ENTRY_ERROR) { gap(r,"native per-entry error"); free(path); return; }
    /* Re-read every directory/special/unknown entry. The bulk object may be a
       mountpoint or firmlink stub, not the anchored target that statat sees. */
    if(native && status==DECODE_OK && (native->type==VREG || native->type==VLNK)) {
        account(c,r,path,(uint64_t)native->device,native->inode,(uint64_t)native->logical,(uint64_t)native->allocated);
        free(path); return;
    }
    struct stat st; c->stat_calls++;
    if(fstatat(fd,name,&st,AT_SYMLINK_NOFOLLOW)) { gap(r,"statat failed"); free(path); return; }
    bool isdir=S_ISDIR(st.st_mode);
    if(isdir && registered(c,path,&st)) { r->excluded++; free(path); return; }
    bool fresh=stat_account(c,r,path,&st);
    if(!isdir || !fresh) { free(path); return; }
    if(st.st_flags & SF_DATALESS) { gap(r,"dataless directory"); free(path); return; }
    if(depth>=c->opt.max_depth) { gap(r,"depth budget"); free(path); return; }
    if(c->opt.pause_child && !strcmp(name,c->opt.pause_child)) pause_here(c);
    c->open_calls++; int next=openat(fd,name,DIRFLAGS);
    if(next<0) { gap(r,"directory open failed"); free(path); return; }
    struct stat after; c->stat_calls++;
    if(fstat(next,&after) || !unchanged(&st,&after)) gap(r,"directory changed before open");
    else directory(c,r,next,path,depth+1,&st,allowance,start,ms);
    close(next); free(path);
}
static void directory(struct collector *c,struct row *r,int fd,const char *path,size_t depth,const struct stat *before,
                      size_t allowance,uint64_t start,uint64_t ms) {
    if(c->opt.native) {
        unsigned char *buffer=alloc(BUFFER_SIZE);
        struct attrlist attrs={.bitmapcount=ATTR_BIT_MAP_COUNT,
            .commonattr=ATTR_CMN_RETURNED_ATTRS|ATTR_CMN_ERROR|ATTR_CMN_NAME|ATTR_CMN_DEVID|ATTR_CMN_OBJTYPE|ATTR_CMN_FLAGS|ATTR_CMN_FILEID,
            .fileattr=ATTR_FILE_ALLOCSIZE|ATTR_FILE_DATALENGTH};
        bool stop=false;
        while(!stop && check(c,r,allowance,start,ms)) {
            c->bulk_calls++;
            int count=getattrlistbulk(fd,&attrs,buffer,BUFFER_SIZE,FSOPT_RETURN_REALDEV);
            if(count<0) { gap(r,"getattrlistbulk failed"); break; }
            if(count==0) break;
            if((size_t)count>BUFFER_SIZE/24) { gap(r,"native count invalid"); break; }
            size_t offset=0;
            for(int i=0;i<count;i++) {
                if(!check(c,r,allowance,start,ms)) { stop=true; break; }
                struct decoded entry;
                enum decoded_status status=decode_entry(buffer+offset,BUFFER_SIZE-offset,&entry);
                if(status==DECODE_INVALID) { gap(r,"native record invalid"); stop=true; break; }
                offset+=entry.used;
                child(c,r,fd,path,(const char *)entry.name,&entry,status,depth,allowance,start,ms);
            }
        }
        free(buffer);
    } else {
        int copy=dup(fd); DIR *dir=copy<0?NULL:fdopendir(copy);
        if(!dir) { if(copy>=0) close(copy); gap(r,"readdir open failed"); }
        else {
            while(check(c,r,allowance,start,ms)) {
                errno=0; c->readdir_calls++; struct dirent *e=readdir(dir);
                if(!e) { if(errno) gap(r,"readdir failed"); break; }
                child(c,r,fd,path,e->d_name,NULL,DECODE_OK,depth,allowance,start,ms);
            }
            closedir(dir);
        }
    }
    if(c->opt.pause_end && depth==0) pause_here(c);
    struct stat after; c->stat_calls++;
    if(fstat(fd,&after) || !unchanged(before,&after)) gap(r,"directory changed during measurement");
}
static void hex(const char *p) {
    putchar('"'); for(const unsigned char *s=(const unsigned char *)p;*s;s++) printf("%02x",*s); putchar('"');
}
static uint64_t number(const char *p) {
    char *end; errno=0; unsigned long long n=strtoull(p,&end,10);
    if(errno || !*p || *end || *p=='-') { fputs("invalid limit\n",stderr); exit(2); } return n;
}
int main(int argc,char **argv) {
    struct collector c={.opt={.max_entries=2000000,.per_root=500000,.max_depth=128,.timeout_ms=600000,.per_root_ms=30000}};
    int at=1;
    for(;at<argc && strcmp(argv[at],"--");at++) {
        const char *a=argv[at];
        if(!strcmp(a,"--manifest")) c.opt.manifest=true;
        else if(!strcmp(a,"--pause-root")) c.opt.pause_root=true;
        else if(!strcmp(a,"--pause-end")) c.opt.pause_end=true;
        else if(at+1<argc && !strcmp(a,"--pause-child")) c.opt.pause_child=argv[++at];
        else if(at+1<argc && !strcmp(a,"--mode")) {
            const char *mode=argv[++at]; if(strcmp(mode,"scalar") && strcmp(mode,"native")) return 2;
            c.opt.native=!strcmp(mode,"native");
        } else if(at+1<argc && !strcmp(a,"--max-entries")) c.opt.max_entries=number(argv[++at]);
        else if(at+1<argc && !strcmp(a,"--max-depth")) c.opt.max_depth=number(argv[++at]);
        else if(at+1<argc && !strcmp(a,"--timeout-ms")) c.opt.timeout_ms=number(argv[++at]);
        else return 2;
    }
    at++; if(at>=argc || c.opt.max_entries>2000000 || c.opt.max_depth>128 || c.opt.timeout_ms>600000) return 2;
    c.count=(size_t)(argc-at); c.rows=alloc(c.count*sizeof(struct row));
    for(size_t i=0;i<c.count;i++) {
        c.rows[i].path=argv[at+(int)i]; c.rows[i].complete=true;
        c.rows[i].discovered_ok=lstat(c.rows[i].path,&c.rows[i].discovered)==0;
    }
    c.start=now_ns();
    for(size_t i=0;i<c.count;i++) {
        struct row *r=&c.rows[i]; uint64_t start=now_ns();
        size_t allowance=(c.opt.max_entries-c.checked)/(c.count-i);
        if(allowance>c.opt.per_root) allowance=c.opt.per_root;
        uint64_t elapsed=(start-c.start)/1000000;
        uint64_t ms=(elapsed>=c.opt.timeout_ms?0:c.opt.timeout_ms-elapsed)/(c.count-i);
        if(ms>c.opt.per_root_ms) ms=c.opt.per_root_ms;
        if(!check(&c,r,allowance,start,ms)) continue;
        if(!r->discovered_ok) { gap(r,"root identity unavailable"); continue; }
        c.open_calls++; int fd=open(r->path,DIRFLAGS);
        if(fd<0) { gap(r,"root open failed"); continue; }
        struct stat st; c.stat_calls++;
        if(fstat(fd,&st) || st.st_dev!=r->discovered.st_dev || st.st_ino!=r->discovered.st_ino) gap(r,"root identity changed");
        else if(st.st_flags & SF_DATALESS) gap(r,"dataless root");
        else {
            if(c.opt.pause_root) pause_here(&c);
            c.checked++; r->checked++; stat_account(&c,r,r->path,&st);
            directory(&c,r,fd,r->path,0,&st,allowance,start,ms);
            c.stat_calls++; struct stat after;
            if(lstat(r->path,&after) || !unchanged(&st,&after)) gap(r,"root changed during measurement");
        }
        close(fd);
    }
    uint64_t elapsed=now_ns()-c.start; bool complete=true;
    for(size_t i=0;i<c.count;i++) if(!c.rows[i].complete) complete=false;
    printf("{\"complete\":%s,\"elapsed_ns\":%"PRIu64",\"entries_checked\":%zu,\"unique_bytes\":[%"PRIu64",%"PRIu64"],",complete?"true":"false",elapsed,c.checked,c.logical,c.allocated);
    printf("\"calls\":{\"stat\":%zu,\"bulk\":%zu,\"readdir\":%zu,\"open\":%zu},\"roots\":[",c.stat_calls,c.bulk_calls,c.readdir_calls,c.open_calls);
    for(size_t i=0;i<c.count;i++) {
        struct row *r=&c.rows[i]; if(i) putchar(','); fputs("{\"path_hex\":",stdout); hex(r->path);
        printf(",\"complete\":%s,\"entries_checked\":%zu,\"excluded_registered_roots\":%zu,\"bytes\":",r->complete?"true":"false",r->checked,r->excluded);
        if(r->measured) printf("[%"PRIu64",%"PRIu64"]",r->logical,r->allocated); else fputs("null",stdout);
        fputs(",\"gaps\":[",stdout); for(size_t j=0;j<r->gaps_len;j++) { if(j) putchar(','); printf("\"%s\"",r->gaps[j]); }
        printf("],\"gaps_omitted\":%zu,\"objects\":[",r->omitted);
        for(size_t j=0;j<r->objects_len;j++) {
            struct object *o=&r->objects[j]; if(j) putchar(','); putchar('['); hex(o->path);
            printf(",%"PRIu64",%"PRIu64",%"PRIu64",%"PRIu64"]",o->dev,o->ino,o->logical,o->allocated); free(o->path);
        }
        fputs("]}",stdout); free(r->objects); free(r->seen.slots);
    }
    puts("]}"); free(c.seen.slots); free(c.rows); return 0;
}
