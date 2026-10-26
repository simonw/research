package main

import (
	"os"
	"syscall"
	"unsafe"
)

// Copied from webframe's cmd/webframe/silence_linux.go: stop JavaScriptCore
// warning about SIGUSR1 and GTK/EGL warning about the a11y bus and DRI3.
func init() {
	var dfl [4]uint64
	_, _, _ = syscall.RawSyscall6(syscall.SYS_RT_SIGACTION, uintptr(syscall.SIGUSR1),
		uintptr(unsafe.Pointer(&dfl)), 0, 8, 0, 0)
	for k, v := range map[string]string{"NO_AT_BRIDGE": "1", "EGL_LOG_LEVEL": "fatal", "LIBGL_DEBUG": "quiet"} {
		if _, ok := syscall.Getenv(k); !ok {
			_ = syscall.Setenv(k, v)
		}
	}
}

// quietNative points fd 2 at /dev/null so warnings native libraries print
// there don't mix with ours, and returns a file for the real stderr.
func quietNative() *os.File {
	fd, err := syscall.Dup(2)
	if err != nil {
		return os.Stderr
	}
	null, err := os.OpenFile(os.DevNull, os.O_WRONLY, 0)
	if err != nil {
		return os.Stderr
	}
	_ = syscall.Dup3(int(null.Fd()), 2, 0)
	return os.NewFile(uintptr(fd), "stderr")
}
