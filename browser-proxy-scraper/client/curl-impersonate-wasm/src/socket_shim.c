/**
 * Socket shim for curl-impersonate WASM.
 *
 * Provides POSIX socket-like functions that route through our Wisp JS bridge.
 * curl calls getaddrinfo() → socket() → connect() → send()/recv() → close().
 * We intercept all of these:
 *
 * - getaddrinfo: stores hostname→fake-IP mapping, returns fake sockaddr
 * - socket: allocates a virtual fd via the JS bridge
 * - connect: looks up the hostname from the fake IP, connects via Wisp
 * - send/recv: pass through to the JS bridge
 * - close: closes the Wisp stream
 * - poll: checks if data is available in the JS receive buffer
 */

#include <emscripten.h>
#include <string.h>
#include <stdlib.h>
#include <stdint.h>
#include <errno.h>

/* Maximum tracked hostname mappings */
#define MAX_HOST_ENTRIES 256
#define FAKE_IP_BASE 0x7F000100  /* 127.0.1.0 */

/* Hostname → fake IP mapping */
static struct {
    char hostname[256];
    uint32_t fake_ip;
    int port;
    int in_use;
} host_table[MAX_HOST_ENTRIES];

static int host_table_count = 0;

/* Store a hostname and return a fake IP */
static uint32_t store_hostname(const char *hostname) {
    /* Check if already stored */
    for (int i = 0; i < host_table_count; i++) {
        if (host_table[i].in_use && strcmp(host_table[i].hostname, hostname) == 0) {
            return host_table[i].fake_ip;
        }
    }
    /* Add new entry */
    if (host_table_count >= MAX_HOST_ENTRIES) {
        return 0;
    }
    int idx = host_table_count++;
    strncpy(host_table[idx].hostname, hostname, 255);
    host_table[idx].hostname[255] = '\0';
    host_table[idx].fake_ip = FAKE_IP_BASE + idx;
    host_table[idx].in_use = 1;
    return host_table[idx].fake_ip;
}

/* Look up hostname from fake IP */
static const char *lookup_hostname(uint32_t fake_ip) {
    int idx = fake_ip - FAKE_IP_BASE;
    if (idx >= 0 && idx < host_table_count && host_table[idx].in_use) {
        return host_table[idx].hostname;
    }
    return NULL;
}

/* JS bridge functions (implemented in wisp-socket-bridge.js) */
extern int wispSocketCreate(void);
extern int wispSocketConnect(int fd, const char *host, int port);
extern int wispSocketSend(int fd, const void *buf, int len);
extern int wispSocketRecv(int fd, void *buf, int maxLen);
extern int wispSocketClose(int fd);

/* ---- POSIX Socket API replacements ---- */

/* Minimal sockaddr structures */
struct shim_sockaddr_in {
    uint16_t sin_family;   /* AF_INET = 2 */
    uint16_t sin_port;     /* Network byte order */
    uint32_t sin_addr;     /* Network byte order */
    char sin_zero[8];
};

struct shim_addrinfo {
    int ai_flags;
    int ai_family;
    int ai_socktype;
    int ai_protocol;
    uint32_t ai_addrlen;
    struct shim_sockaddr_in *ai_addr;
    char *ai_canonname;
    struct shim_addrinfo *ai_next;
};

/* getaddrinfo: resolve hostname to a fake IP, store the mapping */
int getaddrinfo(const char *node, const char *service,
                const struct shim_addrinfo *hints,
                struct shim_addrinfo **res) {
    if (!node || !res) return -1;

    uint32_t fake_ip = store_hostname(node);
    if (fake_ip == 0) return -1;

    /* Allocate result structure */
    struct shim_addrinfo *info = (struct shim_addrinfo *)calloc(1, sizeof(struct shim_addrinfo));
    struct shim_sockaddr_in *addr = (struct shim_sockaddr_in *)calloc(1, sizeof(struct shim_sockaddr_in));

    if (!info || !addr) {
        free(info);
        free(addr);
        return -1;
    }

    int port = 0;
    if (service) {
        port = atoi(service);
    }

    addr->sin_family = 2; /* AF_INET */
    addr->sin_port = ((port >> 8) & 0xFF) | ((port & 0xFF) << 8); /* htons */
    addr->sin_addr = fake_ip; /* Already in our format */

    info->ai_family = 2; /* AF_INET */
    info->ai_socktype = 1; /* SOCK_STREAM */
    info->ai_protocol = 6; /* IPPROTO_TCP */
    info->ai_addrlen = sizeof(struct shim_sockaddr_in);
    info->ai_addr = addr;
    info->ai_next = NULL;

    *res = info;
    return 0;
}

void freeaddrinfo(struct shim_addrinfo *res) {
    while (res) {
        struct shim_addrinfo *next = res->ai_next;
        free(res->ai_addr);
        free(res->ai_canonname);
        free(res);
        res = next;
    }
}

const char *gai_strerror(int errcode) {
    return "getaddrinfo error";
}

/* socket: create a virtual fd via the JS bridge */
int socket(int domain, int type, int protocol) {
    return wispSocketCreate();
}

/* connect: look up the hostname from the fake IP and connect via Wisp */
int connect(int fd, const void *addr, uint32_t addrlen) {
    const struct shim_sockaddr_in *sin = (const struct shim_sockaddr_in *)addr;
    uint32_t fake_ip = sin->sin_addr;
    uint16_t port_net = sin->sin_port;
    int port = ((port_net >> 8) & 0xFF) | ((port_net & 0xFF) << 8); /* ntohs */

    const char *hostname = lookup_hostname(fake_ip);
    if (!hostname) {
        errno = 22; /* EINVAL */
        return -1;
    }

    return wispSocketConnect(fd, hostname, port);
}

/* send: push bytes to the Wisp stream */
int send(int fd, const void *buf, int len, int flags) {
    return wispSocketSend(fd, buf, len);
}

/* recv: pull bytes from the Wisp stream */
int recv(int fd, void *buf, int len, int flags) {
    return wispSocketRecv(fd, buf, len);
}

/* write: same as send for sockets */
int __wrap_write(int fd, const void *buf, int len) {
    if (fd >= 100) { /* Our virtual fds start at 100 */
        return wispSocketSend(fd, buf, len);
    }
    /* Fall through to real write for stdout/stderr */
    return len; /* Stub for non-socket fds */
}

/* read: same as recv for sockets */
int __wrap_read(int fd, void *buf, int len) {
    if (fd >= 100) {
        return wispSocketRecv(fd, buf, len);
    }
    return 0;
}

/* close: close the Wisp stream */
int __wrap_close(int fd) {
    if (fd >= 100) {
        return wispSocketClose(fd);
    }
    return 0;
}

/* poll: simplified - always report readable/writable for our fds */
struct shim_pollfd {
    int fd;
    short events;
    short revents;
};

int poll(struct shim_pollfd *fds, unsigned int nfds, int timeout) {
    if (!fds) return -1;
    int ready = 0;
    for (unsigned int i = 0; i < nfds; i++) {
        if (fds[i].fd >= 100) {
            fds[i].revents = fds[i].events; /* Report all requested events as ready */
            ready++;
        } else {
            fds[i].revents = 0;
        }
    }
    return ready > 0 ? ready : 1;
}

/* select: simplified stub */
int select(int nfds, void *readfds, void *writefds, void *exceptfds, void *timeout) {
    return 1; /* Always ready */
}

/* setsockopt: no-op */
int setsockopt(int fd, int level, int optname, const void *optval, uint32_t optlen) {
    return 0;
}

/* getsockopt: no-op */
int getsockopt(int fd, int level, int optname, void *optval, uint32_t *optlen) {
    return 0;
}

/* getsockname / getpeername: return fake addr */
int getsockname(int fd, void *addr, uint32_t *addrlen) {
    if (addr && addrlen && *addrlen >= sizeof(struct shim_sockaddr_in)) {
        struct shim_sockaddr_in *sin = (struct shim_sockaddr_in *)addr;
        memset(sin, 0, sizeof(*sin));
        sin->sin_family = 2;
        *addrlen = sizeof(struct shim_sockaddr_in);
    }
    return 0;
}

int getpeername(int fd, void *addr, uint32_t *addrlen) {
    return getsockname(fd, addr, addrlen);
}

/* fcntl: no-op for non-blocking etc */
int __wrap_fcntl(int fd, int cmd, ...) {
    return 0;
}

/* ioctl: no-op */
int __wrap_ioctl(int fd, unsigned long request, ...) {
    return 0;
}
