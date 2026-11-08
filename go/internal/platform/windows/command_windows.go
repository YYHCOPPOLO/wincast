//go:build windows && amd64

package windows

import (
	"context"
	"errors"
	"fmt"
	"os"
	"strings"
	"syscall"
	"unsafe"
)

var (
	waitForSingleObject = kernel32.NewProc("WaitForSingleObject")
	getExitCodeProcess  = kernel32.NewProc("GetExitCodeProcess")
	terminateProcess    = kernel32.NewProc("TerminateProcess")
)

const (
	createNoWindow   = 0x08000000
	waitObject0      = 0x00000000
	waitObjectFailed = 0xFFFFFFFF
)

// ErrEmptyCommand reports a blank command line before any process is created.
var ErrEmptyCommand = errors.New("enter a command to run")

// RunCommandLine runs a user-authored command exactly as CreateProcessW sees
// it: no shell wrapper, PATH lookup for a bare executable, the user profile as
// the working directory and no console window. Cancellation terminates the
// child before reporting.
func RunCommandLine(ctx context.Context, line string) error {
	line = strings.TrimSpace(line)
	if line == "" {
		return ErrEmptyCommand
	}
	commandLine, err := syscall.UTF16PtrFromString(formatCommandLine(line))
	if err != nil {
		return err
	}
	var workingDirectory *uint16
	if home := os.Getenv("USERPROFILE"); home != "" {
		workingDirectory, _ = syscall.UTF16PtrFromString(home)
	}
	var startup syscall.StartupInfo
	startup.Cb = uint32(unsafe.Sizeof(startup))
	var process syscall.ProcessInformation
	if err := syscall.CreateProcess(nil, commandLine, nil, nil, false, createNoWindow, nil, workingDirectory, &startup, &process); err != nil {
		return fmt.Errorf("start command: %w", err)
	}
	defer syscall.CloseHandle(process.Thread)
	finished := make(chan struct{})
	go func() {
		select {
		case <-ctx.Done():
			terminateProcess.Call(uintptr(process.Process), 1)
		case <-finished:
		}
	}()
	result, _, callErr := waitForSingleObject.Call(uintptr(process.Process), 0xFFFFFFFF) // INFINITE
	close(finished)
	var exitCode uint32
	getExitCodeProcess.Call(uintptr(process.Process), uintptr(unsafe.Pointer(&exitCode)))
	syscall.CloseHandle(process.Process)
	if ctx.Err() != nil {
		return ctx.Err()
	}
	if uint32(result) == waitObjectFailed {
		return fmt.Errorf("wait for command: %v", callErr)
	}
	if uint32(result) != waitObject0 {
		return fmt.Errorf("wait for command: status 0x%x", uint32(result))
	}
	if exitCode != 0 {
		return fmt.Errorf("command exited with status %d", exitCode)
	}
	return nil
}

// formatCommandLine quotes an executable path that contains spaces, matching
// the Windows oracle. A bare executable name is left for PATH lookup.
func formatCommandLine(command string) string {
	command = strings.TrimSpace(command)
	if command == "" || strings.HasPrefix(command, `"`) {
		return command
	}
	space := strings.IndexFunc(command, func(r rune) bool {
		return r == ' ' || r == '\t' || r == '\r' || r == '\n'
	})
	if space < 0 {
		return command
	}
	argv0 := command[:space]
	rest := strings.TrimLeft(command[space:], " \t\r\n")
	if !strings.ContainsAny(argv0, `\/`) || !strings.Contains(argv0, " ") {
		return command
	}
	if rest == "" {
		return `"` + argv0 + `"`
	}
	return `"` + argv0 + `" ` + rest
}
