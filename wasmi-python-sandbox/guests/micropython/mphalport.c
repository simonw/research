// Route MicroPython's stdout to a host-provided function instead of libc.
#include <stddef.h>
#include "py/mphal.h"

__attribute__((import_module("env"), import_name("host_write")))
void host_write(int fd, const char *ptr, size_t len);

void mp_hal_stdout_tx_strn_cooked(const char *str, size_t len) {
    host_write(1, str, len);
}
