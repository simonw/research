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
#include <stdio.h>
#include <arpa/inet.h>

/* Non-blocking flag for fcntl */
#define O_NONBLOCK 0x800
static int fd_flags[512]; /* per-fd flags */

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
extern int wispSocketGetState(int fd);
extern int wispSocketHasData(int fd);
extern int wispSocketWaitForData(int fd, int timeoutMs);
extern int wispSocketDebugEnabled(void);

#define SHIM_LOG(fmt, ...) do { \
    if (wispSocketDebugEnabled()) { \
        emscripten_log(EM_LOG_CONSOLE, fmt, ##__VA_ARGS__); \
    } \
} while (0)

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

/* getaddrinfo: resolve hostname to a fake IP, store the mapping.
 * Must use --wrap=getaddrinfo so this overrides Emscripten's SOCKFS version. */
int __wrap_getaddrinfo(const char *node, const char *service,
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
    addr->sin_port = htons(port);
    addr->sin_addr = htonl(fake_ip); /* Store in network byte order */

    info->ai_family = 2; /* AF_INET */
    info->ai_socktype = 1; /* SOCK_STREAM */
    info->ai_protocol = 6; /* IPPROTO_TCP */
    info->ai_addrlen = sizeof(struct shim_sockaddr_in);
    info->ai_addr = addr;
    info->ai_next = NULL;

    *res = info;
    return 0;
}

void __wrap_freeaddrinfo(struct shim_addrinfo *res) {
    while (res) {
        struct shim_addrinfo *next = res->ai_next;
        free(res->ai_addr);
        free(res->ai_canonname);
        free(res);
        res = next;
    }
}

const char *__wrap_gai_strerror(int errcode) {
    return "getaddrinfo error";
}

/* socket: create a virtual fd via the JS bridge */
int __wrap_socket(int domain, int type, int protocol) {
    int fd = wispSocketCreate();
    SHIM_LOG("[shim] socket(domain=%d type=%d proto=%d) = fd=%d", domain, type, protocol, fd);
    return fd;
}

/* connect: look up the hostname from the fake IP and send a Wisp CONNECT.
 * For non-blocking sockets we still report EINPROGRESS so curl continues via
 * poll()/getsockopt(SO_ERROR), but the later readiness/error state is driven
 * by the bridge rather than a kernel TCP handshake. */
int __wrap_connect(int fd, const void *addr, uint32_t addrlen) {
    const struct shim_sockaddr_in *sin = (const struct shim_sockaddr_in *)addr;
    SHIM_LOG("[shim] connect fd=%d addrlen=%d family=%d", fd, addrlen, sin->sin_family);
    uint32_t fake_ip = ntohl(sin->sin_addr); /* Network → host byte order */
    uint16_t port_net = sin->sin_port;
    int port = ntohs(port_net);

    const char *hostname = lookup_hostname(fake_ip);
    if (!hostname) {
        SHIM_LOG("[shim] connect fd=%d NO HOSTNAME for ip=0x%x", fd, fake_ip);
        errno = EINVAL;
        return -1;
    }

    SHIM_LOG("[shim] connect fd=%d host=%s port=%d", fd, hostname, port);

    /* Send Wisp CONNECT packet synchronously */
    int result = wispSocketConnect(fd, hostname, port);
    if (result < 0) {
        SHIM_LOG("[shim] connect failed (ws not ready)");
        errno = ECONNREFUSED;
        return -1;
    }

    if (fd < 512 && (fd_flags[fd] & O_NONBLOCK)) {
        errno = EINPROGRESS;
        SHIM_LOG("[shim] connect returning EINPROGRESS for non-blocking fd=%d", fd);
        return -1;
    }

    SHIM_LOG("[shim] connect returning 0 (CONNECT sent)");
    return 0;
}

/* send: push bytes to the Wisp stream */
int __wrap_send(int fd, const void *buf, int len, int flags) {
    SHIM_LOG("[shim] send fd=%d len=%d", fd, len);
    if (fd >= 100) {
        return wispSocketSend(fd, buf, len);
    }
    return len;
}

/* recv: pull bytes from the Wisp stream.
 * Virtual sockets stay non-blocking from curl's perspective. poll()/select()
 * is the point where Asyncify may suspend, so recv()/read() should return
 * EAGAIN when no bytes are queued yet. */
int __wrap_recv(int fd, void *buf, int len, int flags) {
    if (fd >= 100) {
        int nonblock = (fd < 512) ? (fd_flags[fd] & O_NONBLOCK) : 0;
        int hasData = wispSocketHasData(fd);
        SHIM_LOG("[shim] recv fd=%d len=%d nonblock=%d hasData=%d", fd, len, nonblock, hasData);
        if (nonblock && !hasData) {
            errno = EAGAIN;
            SHIM_LOG("[shim] recv fd=%d -> EAGAIN", fd);
            return -1;
        }
        int result = wispSocketRecv(fd, buf, len);
        if (result < 0) {
            errno = (wispSocketGetState(fd) == 2) ? ECONNRESET : EAGAIN;
        }
        SHIM_LOG("[shim] recv fd=%d result=%d", fd, result);
        return result;
    }
    return 0;
}

/* write: same as send for sockets */
int __wrap_write(int fd, const void *buf, int len) {
    if (fd >= 100) { /* Our virtual fds start at 100 */
        SHIM_LOG("[shim] write(socket) fd=%d len=%d", fd, len);
        return wispSocketSend(fd, buf, len);
    }
    /* Fall through to real write for stdout/stderr */
    extern int __real_write(int, const void *, int);
    return __real_write(fd, buf, len);
}

/* read: same as recv for sockets */
int __wrap_read(int fd, void *buf, int len) {
    if (fd >= 100) {
        int nonblock = (fd < 512) ? (fd_flags[fd] & O_NONBLOCK) : 0;
        int hasData = wispSocketHasData(fd);
        SHIM_LOG("[shim] read fd=%d len=%d nonblock=%d hasData=%d", fd, len, nonblock, hasData);
        if (nonblock && !hasData) {
            errno = EAGAIN;
            SHIM_LOG("[shim] read fd=%d -> EAGAIN", fd);
            return -1;
        }
        int result = wispSocketRecv(fd, buf, len);
        if (result < 0) {
            errno = (wispSocketGetState(fd) == 2) ? ECONNRESET : EAGAIN;
        }
        SHIM_LOG("[shim] read fd=%d result=%d", fd, result);
        return result;
    }
    return 0;
}

/* close: close the Wisp stream */
int __wrap_close(int fd) {
    SHIM_LOG("[shim] close fd=%d", fd);
    if (fd >= 100) {
        return wispSocketClose(fd);
    }
    return 0;
}

/* poll: implemented entirely in JS (wisp-socket-bridge.js) as an Asyncify import.
 * Removed --Wl,--wrap=poll; instead, poll is provided directly by --js-library.
 * This ensures Asyncify properly instruments all callers (Curl_poll, etc.). */

/* select: simplified stub */
int __wrap_select(int nfds, void *readfds, void *writefds, void *exceptfds, void *timeout) {
    SHIM_LOG("[shim] select nfds=%d", nfds);
    return 1; /* Always ready */
}

/* setsockopt: no-op */
int __wrap_setsockopt(int fd, int level, int optname, const void *optval, uint32_t optlen) {
    SHIM_LOG("[shim] setsockopt fd=%d level=%d optname=%d", fd, level, optname);
    return 0;
}

/* getsockopt: query connection state for SO_ERROR */
#define SOL_SOCKET 1
#define SO_ERROR   4

int __wrap_getsockopt(int fd, int level, int optname, void *optval, uint32_t *optlen) {
    SHIM_LOG("[shim] getsockopt fd=%d level=%d optname=%d", fd, level, optname);
    if (optval && optlen && *optlen >= 4) {
        if (level == SOL_SOCKET && optname == SO_ERROR) {
            int state = wispSocketGetState(fd);
            int err = 0;
            if (state == 2) err = ECONNREFUSED;
            *(int *)optval = err;
            *optlen = 4;
            SHIM_LOG("[shim] getsockopt SO_ERROR fd=%d state=%d err=%d", fd, state, err);
        } else {
            /* Zero-fill for other options */
            memset(optval, 0, *optlen > 256 ? 256 : *optlen);
        }
    }
    return 0;
}

/* getsockname: return fake addr */
int __wrap_getsockname(int fd, void *addr, uint32_t *addrlen) {
    SHIM_LOG("[shim] getsockname fd=%d", fd);
    if (addr && addrlen && *addrlen >= sizeof(struct shim_sockaddr_in)) {
        struct shim_sockaddr_in *sin = (struct shim_sockaddr_in *)addr;
        memset(sin, 0, sizeof(*sin));
        sin->sin_family = 2;
        *addrlen = sizeof(struct shim_sockaddr_in);
    }
    return 0;
}

/* getpeername: return fake addr in network byte order */
int __wrap_getpeername(int fd, void *addr, uint32_t *addrlen) {
    SHIM_LOG("[shim] getpeername fd=%d addr=%p addrlen=%p", fd, addr, addrlen);
    if (addr && addrlen && *addrlen >= sizeof(struct shim_sockaddr_in)) {
        struct shim_sockaddr_in *sin = (struct shim_sockaddr_in *)addr;
        memset(sin, 0, sizeof(*sin));
        sin->sin_family = 2; /* AF_INET */
        sin->sin_addr = htonl(FAKE_IP_BASE); /* Network byte order */
        sin->sin_port = htons(443);
        *addrlen = sizeof(struct shim_sockaddr_in);
    }
    return 0;
}

/* fcntl: track non-blocking flag per fd */
#include <stdarg.h>
#define F_GETFL 3
#define F_SETFL 4
int __wrap_fcntl(int fd, int cmd, ...) {
    va_list ap;
    va_start(ap, cmd);
    int result = 0;
    if (cmd == F_GETFL) {
        result = (fd < 512) ? fd_flags[fd] : 0;
    } else if (cmd == F_SETFL) {
        int flags = va_arg(ap, int);
        if (fd < 512) fd_flags[fd] = flags;
        result = 0;
    }
    va_end(ap);
    SHIM_LOG("[shim] fcntl fd=%d cmd=%d result=%d", fd, cmd, result);
    return result;
}

/* ioctl: no-op */
int __wrap_ioctl(int fd, unsigned long request, ...) {
    return 0;
}
