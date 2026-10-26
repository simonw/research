package main

import (
	"bufio"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"strings"
	"syscall"
)

// ensureDisplay starts a private Xvfb server when there is no display, as
// when run headless. With DISPLAY or WAYLAND_DISPLAY already set (including
// under xvfb-run) it does nothing.
func ensureDisplay() error {
	if os.Getenv("DISPLAY") != "" || os.Getenv("WAYLAND_DISPLAY") != "" {
		return nil
	}
	xvfb, err := exec.LookPath("Xvfb")
	if err != nil {
		return errors.New("no display: set DISPLAY or install Xvfb")
	}
	r, w, err := os.Pipe()
	if err != nil {
		return err
	}
	defer r.Close()
	// -displayfd has Xvfb pick a free display and write its number to fd 3
	// once it is ready.
	cmd := exec.Command(xvfb, "-displayfd", "3", "-screen", "0", "1280x1024x24", "-nolisten", "tcp")
	cmd.ExtraFiles = []*os.File{w}
	// Xvfb exits with us. The parent death signal is tied to the creating
	// thread, which is the main thread here (locked in init).
	cmd.SysProcAttr = &syscall.SysProcAttr{Pdeathsig: syscall.SIGTERM, Setpgid: true}
	if err := cmd.Start(); err != nil {
		w.Close()
		return fmt.Errorf("starting Xvfb: %w", err)
	}
	w.Close()
	line, err := bufio.NewReader(r).ReadString('\n')
	if err != nil {
		_ = cmd.Process.Kill()
		return errors.New("Xvfb failed to start")
	}
	return os.Setenv("DISPLAY", ":"+strings.TrimSpace(line))
}
