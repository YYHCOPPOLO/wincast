//go:build windows

package windows

import (
	"context"
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"net"
	"os"
	"path/filepath"
	"sync"
	"syscall"
	"time"
	"unsafe"
)

var kernel32 = syscall.NewLazyDLL("kernel32.dll")
var createMutex = kernel32.NewProc("CreateMutexW")

type Activation byte

const (
	ActivatePanel Activation = iota + 1
	ActivateDiagnostics
	ActivateQuit
)

const activationVersion = 1

// Instance holds the writer identity until all application workers finish.
// Only its first owner may publish an activation endpoint, after UI readiness.
type Instance struct {
	mu           sync.Mutex
	handle       syscall.Handle
	first        bool
	path         string
	listener     *net.UnixListener
	active       net.Conn
	socket       os.FileInfo
	stopped      bool
	holdIdentity bool
}

func AcquireInstance(identifier, profile string) (*Instance, bool, error) {
	name, err := syscall.UTF16PtrFromString(`Local\` + identifier)
	if err != nil {
		return nil, false, err
	}
	handle, _, callErr := createMutex.Call(0, 0, uintptr(unsafe.Pointer(name)))
	if handle == 0 {
		return nil, false, fmt.Errorf("create preview instance lock: %w", callErr)
	}
	first := !errors.Is(callErr, syscall.ERROR_ALREADY_EXISTS)
	key := sha256.Sum256([]byte(identifier + "\x00" + profile))
	path := filepath.Join(os.TempDir(), "tinycast-"+hex.EncodeToString(key[:12])+".sock")
	// Windows AF_UNIX has a 108-byte sun_path, including the terminator.
	// Keep a path error visible instead of silently weakening single-instance.
	if len([]byte(path)) >= 108 {
		_ = syscall.CloseHandle(syscall.Handle(handle))
		return nil, false, errors.New("preview activation socket path is too long")
	}
	instance := &Instance{handle: syscall.Handle(handle), first: first, path: path}
	return instance, first, nil
}

// Listen publishes readiness only after the UI has installed its handlers.
// A later instance never calls Listen or removes an existing endpoint.
func (i *Instance) Listen(ctx context.Context, dispatch func(Activation)) error {
	i.mu.Lock()
	defer i.mu.Unlock()
	if !i.first || i.stopped || i.listener != nil {
		return errors.New("preview cannot own the activation endpoint")
	}
	// The kernel object proves that no other writer owns this endpoint.
	// Refuse to remove an unrelated file or a substituted link at that path.
	if previous, err := os.Lstat(i.path); err == nil {
		if previous.Mode()&os.ModeSocket == 0 {
			return errors.New("preview activation endpoint is not a socket")
		}
		if err := os.Remove(i.path); err != nil {
			return fmt.Errorf("remove stale preview endpoint: %w", err)
		}
	} else if !errors.Is(err, os.ErrNotExist) {
		return fmt.Errorf("read preview activation endpoint: %w", err)
	}
	listener, err := net.ListenUnix("unix", &net.UnixAddr{Name: i.path, Net: "unix"})
	if err != nil {
		return fmt.Errorf("listen for preview activation: %w", err)
	}
	listener.SetUnlinkOnClose(false)
	socket, err := os.Stat(i.path)
	if err != nil {
		listener.Close()
		return fmt.Errorf("identify preview activation endpoint: %w", err)
	}
	i.listener, i.socket = listener, socket
	go i.serve(ctx, listener, dispatch)
	return nil
}

func (i *Instance) serve(ctx context.Context, listener *net.UnixListener, dispatch func(Activation)) {
	for {
		conn, err := listener.Accept()
		if err != nil {
			return
		}
		i.mu.Lock()
		if i.stopped {
			i.mu.Unlock()
			conn.Close()
			return
		}
		i.active = conn
		i.mu.Unlock()
		i.receive(ctx, conn, dispatch)
		conn.Close()
		i.mu.Lock()
		i.active = nil
		i.mu.Unlock()
	}
}

func (i *Instance) receive(ctx context.Context, conn net.Conn, dispatch func(Activation)) {
	_ = conn.SetDeadline(time.Now().Add(2 * time.Second))
	var greeting [5]byte
	greeting[0] = activationVersion
	binary.LittleEndian.PutUint32(greeting[1:], uint32(os.Getpid()))
	if _, err := conn.Write(greeting[:]); err != nil {
		return
	}
	var packet [2]byte
	if _, err := io.ReadFull(conn, packet[:]); err != nil || packet[0] != activationVersion || ctx.Err() != nil {
		return
	}
	command := Activation(packet[1])
	if command < ActivatePanel || command > ActivateQuit {
		return
	}
	// A quit can close activation I/O as soon as the UI takes it. Acknowledge
	// acceptance first so that normal shutdown cannot look like failed IPC.
	if _, err := conn.Write([]byte{activationVersion}); err == nil {
		dispatch(command) // enqueue on the UI thread; acceptance is not execution
	}
}

// Forward waits for the first owner's readiness without ever acquiring IPC
// ownership. Once a command was sent, an uncertain result is never replayed.
func (i *Instance) Forward(ctx context.Context, command Activation) error {
	ticker := time.NewTicker(25 * time.Millisecond)
	defer ticker.Stop()
	for {
		conn, err := (&net.Dialer{Timeout: 200 * time.Millisecond}).DialContext(ctx, "unix", i.path)
		if err != nil {
			select {
			case <-ctx.Done():
				return fmt.Errorf("preview activation did not become ready: %w", ctx.Err())
			case <-ticker.C:
				continue
			}
		}
		defer conn.Close()
		deadline := time.Now().Add(2 * time.Second)
		if limit, ok := ctx.Deadline(); ok && limit.Before(deadline) {
			deadline = limit
		}
		_ = conn.SetDeadline(deadline)
		var greeting [5]byte
		if _, err := io.ReadFull(conn, greeting[:]); err != nil {
			return fmt.Errorf("read preview activation readiness: %w", err)
		}
		pid := binary.LittleEndian.Uint32(greeting[1:])
		if greeting[0] != activationVersion || pid == 0 || pid == uint32(os.Getpid()) {
			return errors.New("invalid preview activation readiness")
		}
		// Transfer foreground permission only to this exact peer, never ANY.
		user32.NewProc("AllowSetForegroundWindow").Call(uintptr(pid))
		if _, err := conn.Write([]byte{activationVersion, byte(command)}); err != nil {
			return fmt.Errorf("send preview activation: %w", err)
		}
		var accepted [1]byte
		if _, err := io.ReadFull(conn, accepted[:]); err != nil {
			return fmt.Errorf("preview activation acknowledgement is unavailable: %w", err)
		}
		if accepted[0] != activationVersion {
			return errors.New("invalid preview activation acknowledgement")
		}
		return nil
	}
}

// StopServing releases activation I/O on quit but keeps the writer identity
// until Close, after the UI loop and database workers have stopped.
func (i *Instance) StopServing() {
	if i == nil {
		return
	}
	i.mu.Lock()
	defer i.mu.Unlock()
	if i.stopped {
		return
	}
	i.stopped = true
	if i.listener != nil {
		i.listener.Close()
	}
	if i.active != nil {
		i.active.Close()
	}
	if i.socket != nil {
		if current, err := os.Lstat(i.path); err == nil && current.Mode()&os.ModeSocket != 0 && os.SameFile(current, i.socket) {
			_ = os.Remove(i.path)
		}
	}
}

func (i *Instance) Close() {
	if i != nil {
		i.StopServing()
		i.mu.Lock()
		defer i.mu.Unlock()
		if i.handle != 0 && !i.holdIdentity {
			_ = syscall.CloseHandle(i.handle)
			i.handle = 0
		}
	}
}

// HoldIdentityUntilExit keeps the kernel reference if a canceled writer did
// not finish by shutdown's deadline. Process termination releases it; another
// preview must not start writing while that worker's completion is uncertain.
func (i *Instance) HoldIdentityUntilExit() {
	i.mu.Lock()
	i.holdIdentity = true
	i.mu.Unlock()
}
