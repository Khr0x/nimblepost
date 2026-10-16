// Diagnostic preload only: glibc counters, no payloads, allocation hooks or trim.
#define _GNU_SOURCE
#include <fcntl.h>
#include <limits.h>
#include <malloc.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

static void *sample(void *unused) {
    (void)unused;
    const char *directory = getenv("NP013_ALLOCATOR_DIAGNOSTIC_DIR");
    char path[PATH_MAX], temporary[PATH_MAX];
    int size = snprintf(path, sizeof(path), "%s/allocator-%ld.xml", directory, (long)getpid());
    if (size < 0 || (size_t)size >= sizeof(path)) return NULL;
    size = snprintf(temporary, sizeof(temporary), "%s.tmp", path);
    if (size < 0 || (size_t)size >= sizeof(temporary)) return NULL;
    for (int i = 0; i < 600; ++i) {
        int fd = open(temporary, O_WRONLY | O_CREAT | O_TRUNC | O_NOFOLLOW, 0600);
        if (fd < 0) return NULL;
        FILE *out = fdopen(fd, "w");
        if (!out) { close(fd); return NULL; }
        struct timespec now;
        int ok = clock_gettime(CLOCK_MONOTONIC, &now) == 0;
        if (ok) {
            fprintf(out, "<np013 pid=\"%ld\" monotonicNs=\"%lld\"/>\n", (long)getpid(),
                    (long long)now.tv_sec * 1000000000LL + now.tv_nsec);
            ok = malloc_info(0, out) == 0;
        }
        if (fclose(out) != 0) ok = 0;
        if (!ok || rename(temporary, path) != 0) return NULL;
        sleep(1);
    }
    return NULL;
}

__attribute__((constructor)) static void start(void) {
    if (!getenv("NP013_ALLOCATOR_DIAGNOSTIC_DIR")) return;
    char executable[PATH_MAX];
    ssize_t size = readlink("/proc/self/exe", executable, sizeof(executable) - 1);
    if (size < 0) return;
    executable[size] = '\0';
    const char *name = strrchr(executable, '/');
    if (!name || (strcmp(name + 1, "NimblePost") != 0 && strncmp(name + 1, "WebKit", 6) != 0)) return;
    pthread_t thread;
    if (pthread_create(&thread, NULL, sample, NULL) == 0) pthread_detach(thread);
}
